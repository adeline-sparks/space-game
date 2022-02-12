#![allow(dead_code)]
use std::f64::consts::PI;

use dom::{open_websocket, spawn, InputEventListener, Key};
use futures::FutureExt;
use gl::{Context, Sampler2D, Shader, Texture, Vao};
use glam::{DMat4, DQuat, DVec3, IVec3, Mat4, Vec3};
use log::info;
use mesh::{Attribute, NORMAL, POSITION};
use wasm_bindgen::prelude::*;

pub mod dom;
pub mod gl;
pub mod mesh;
pub mod voxel;
use voxel::{marching_cubes, SignedDistanceFunction};

use crate::dom::DomError;
use crate::mesh::AttributeType;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    console_log::init().unwrap();

    use anyhow::Context;
    spawn(main_render().map(|r| r.context("main_render failed")));
    spawn(main_net().map(|r| r.context("main_net failed")));
}

struct Sphere(Vec3, f32);

impl SignedDistanceFunction for Sphere {
    fn value(&self, pos: Vec3) -> f32 {
        (pos - self.0).length() - self.1
    }

    fn grad(&self, pos: Vec3) -> Vec3 {
        2.0 * (pos - self.0)
    }
}

impl<A: SignedDistanceFunction, B: SignedDistanceFunction> SignedDistanceFunction for (A, B) {
    fn value(&self, pos: Vec3) -> f32 {
        let a = self.0.value(pos);
        let b = self.1.value(pos);
        if a < b {
            a
        } else {
            b
        }
    }

    fn grad(&self, pos: Vec3) -> Vec3 {
        let a = self.0.value(pos);
        let b = self.1.value(pos);
        if a < b {
            self.0.grad(pos)
        } else {
            self.1.grad(pos)
        }
    }
}

async fn main_render() -> anyhow::Result<()> {
    dom::content_loaded().await?;
    let input = InputEventListener::from_canvas("space_game")?;
    let context = Context::from_canvas("space_game")?;

    let color_texture = Texture::load(&context, "ground_0010_base_color_2k.jpg").await?;
    let normal_texture = Texture::load(&context, "ground_0010_normal_2k.jpg").await?;

    let attributes = &[
        Attribute {
            name: POSITION,
            type_: AttributeType::Vec3,
        },
        Attribute {
            name: NORMAL,
            type_: AttributeType::Vec3,
        },
    ];

    let mesh = marching_cubes(
        &(
            Sphere(Vec3::new(0.0, 0.0, 0.0), 50.0),
            Sphere(Vec3::new(50.0, 10.0, 0.0), 30.0),
        ),
        (
            Vec3::new(-128.0, -128.0, -128.0),
            Vec3::new(128.0, 128.0, 128.0),
        ),
        IVec3::new(32, 32, 32),
    );

    let vao = Vao::build(&context, attributes, &mesh)?;

    let shader = Shader::compile(
        &context,
        attributes,
        r##"#version 300 es
        uniform mat4x4 model_view_projection;
        uniform mat4x4 model_matrix;
        uniform mat4x4 normal_matrix;
        
        in vec3 vert_pos;
        in vec3 vert_normal;
        out vec3 frag_world_pos;
        out vec3 frag_world_normal;

        void main() { 
            vec4 pos;
            pos.xyz = vert_pos;
            pos.w = 1.0;

            gl_Position = model_view_projection * pos;
            frag_world_pos = (model_matrix * pos).xyz;

            vec4 normal;
            normal.xyz = vert_normal;
            normal.w = 0.0;
            frag_world_normal = (normal_matrix * normal).xyz;
        }
        "##,
        r##"#version 300 es
    
        precision highp float;

        uniform sampler2D tex_color;
        uniform sampler2D tex_normal;
        uniform float tex_scale;
        uniform float tex_blend_sharpness;
        uniform vec3 light_dir;

        in vec3 frag_world_pos;
        in vec3 frag_world_normal;
        out vec4 out_color;
        
        void main() {
            vec3 uv = frag_world_pos * tex_scale;
            vec3 weights = pow(abs(frag_world_normal), vec3(tex_blend_sharpness));
            weights /= (weights.x + weights.y + weights.z);

            mat3 colors = mat3(
                pow(texture(tex_color, uv.yz).rgb, vec3(2.2)),
                pow(texture(tex_color, uv.xz).rgb, vec3(2.2)),
                pow(texture(tex_color, uv.xy).rgb, vec3(2.2))
            );
            vec3 color = colors * weights;

            mat3 normals = mat3(
                texture(tex_normal, uv.yz).rgb,
                texture(tex_normal, uv.xz).rgb,
                texture(tex_normal, uv.xy).rgb
            );
            normals = 2.0 * normals - 1.0;
            normals[0].xy += frag_world_normal.zy;
            normals[1].xy += frag_world_normal.xz;
            normals[2].xy += frag_world_normal.xy;
            normals[0].z = abs(normals[0].z) * frag_world_normal.x;
            normals[1].z = abs(normals[1].z) * frag_world_normal.y;
            normals[2].z = abs(normals[2].z) * frag_world_normal.z;
            normals[0] = normals[0].zyx;
            normals[1] = normals[1].xzy;
            vec3 normal = normalize(normals * weights);

            out_color.rgb = .3 * dot(light_dir, normal) + color;
            //out_color.rgb = normal / 2.0 + 0.5;
            out_color.rgb = pow(out_color.rgb, vec3(1.0/2.2));
            out_color.a = 1.0;
        }
        "##,
    )?;

    let model_view_projection_loc =
        shader.uniform_location::<glam::Mat4>("model_view_projection")?;
    let model_matrix_loc = shader.uniform_location::<glam::Mat4>("model_matrix")?;
    let normal_matrix_loc = shader.uniform_location::<glam::Mat4>("normal_matrix")?;
    let tex_color = shader.uniform_location::<Sampler2D>("tex_color")?;
    let tex_normal = shader.uniform_location::<Sampler2D>("tex_normal")?;
    let tex_scale_loc = shader.uniform_location::<f32>("tex_scale")?;
    let tex_blend_sharpness_loc = shader.uniform_location::<f32>("tex_blend_sharpness")?;
    let light_dir_loc = shader.try_uniform_location::<glam::Vec3>("light_dir");

    shader.set_uniform(&tex_scale_loc, 0.1);
    shader.set_uniform(&tex_blend_sharpness_loc, 4.0);
    shader.set_uniform(&tex_color, Sampler2D(0));
    shader.set_uniform(&tex_normal, Sampler2D(1));

    let canvas = context.canvas();
    let aspect_ratio = (canvas.width() as f32) / (canvas.height() as f32);
    let projection = Mat4::perspective_rh_gl((75.0f32).to_radians(), aspect_ratio, 1.0, 1000.0);

    let mut view = DMat4::look_at_rh(DVec3::new(0.0, 0.0, 100.0), DVec3::ZERO, DVec3::Y);
    let mut prev_time = animation_frame_seconds().await?;
    let mut prev_mouse_pos = input.mouse_pos();
    loop {
        let time = animation_frame_seconds().await?;
        let dt = time - prev_time;
        prev_time = time;

        let mouse_pos = input.mouse_pos();
        let mouse_delta = (mouse_pos - prev_mouse_pos).as_dvec2() * dt;
        prev_mouse_pos = mouse_pos;

        let quat =
            DQuat::from_scaled_axis(DVec3::new(-mouse_delta.y / 20.0, mouse_delta.x / 20.0, 0.0));
        view = DMat4::from_quat(quat) * view;

        let speed = PI / 4.0;
        if input.is_key_down(&Key::ch('q')) {
            view = DMat4::from_rotation_z(speed * dt) * view;
        } else if input.is_key_down(&Key::ch('e')) {
            view = DMat4::from_rotation_z(-speed * dt) * view;
        }

        let speed = 50.0;
        if input.is_key_down(&Key::ch('w')) {
            view = DMat4::from_translation(DVec3::new(0.0, 0.0, speed * dt)) * view;
        } else if input.is_key_down(&Key::ch('s')) {
            view = DMat4::from_translation(DVec3::new(0.0, 0.0, -speed * dt)) * view;
        }

        if input.is_key_down(&Key::ch('a')) {
            view = DMat4::from_translation(DVec3::new(speed * dt, 0.0, 0.0)) * view;
        } else if input.is_key_down(&Key::ch('d')) {
            view = DMat4::from_translation(DVec3::new(-speed * dt, 0.0, 0.0)) * view;
        }

        let light_dir = DVec3::new((time / 2.0).cos(), 0.0, (time / 2.0).sin());

        context.clear();
        let model = Mat4::IDENTITY;
        let model_view = view.as_mat4() * model;
        let model_view_projection = projection * model_view;
        shader.set_uniform(&model_view_projection_loc, model_view_projection);
        shader.set_uniform(&model_matrix_loc, model);
        shader.set_uniform(&normal_matrix_loc, model.inverse().transpose());
        if let Some(loc) = &light_dir_loc {
            shader.set_uniform(loc, light_dir.as_vec3());
        }
        context.draw(
            &shader,
            &[Some(&color_texture), Some(&normal_texture)],
            &vao,
        );
    }
}

async fn animation_frame_seconds() -> Result<f64, DomError> {
    Ok(dom::animation_frame().await? / 1e3)
}

async fn main_net() -> anyhow::Result<()> {
    info!("Creating websocket");
    let ws = open_websocket("ws://localhost:8000/ws/v1").await?;
    info!("Websocket connected");
    ws.send_with_str("Hello World").map_err(DomError::from)?;
    Ok(())
}
