#![allow(dead_code)]
use std::f64::consts::PI;

use dom::{key_consts, open_websocket, spawn, InputEventListener};
use futures::FutureExt;
use gl::{Context, Shader, Texture, Vao};
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
    spawn(main_render().map(|r| r.context("main_render")));
    spawn(main_net().map(|r| r.context("main_net")));
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

    let texture = Texture::load(&context, "ground_0010_base_color_2k.jpg").await?;

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
            Sphere(Vec3::new(50.0, 0.0, 0.0), 30.0),
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
        uniform float tex_scale;
        uniform float tex_blend_sharpness;

        in vec3 frag_world_pos;
        in vec3 frag_world_normal;
        out vec4 out_color;
        
        void main() {
            vec3 scaled_world_pos = frag_world_pos * tex_scale;
            mat3 sample_colors = mat3(
                texture(tex_color, scaled_world_pos.yz).rgb,
                texture(tex_color, scaled_world_pos.xz).rgb,
                texture(tex_color, scaled_world_pos.xy).rgb
            );
            vec3 sample_weights = pow(abs(frag_world_normal), vec3(tex_blend_sharpness));
            sample_weights /= (sample_weights.x + sample_weights.y + sample_weights.z);
            out_color.rgb = sample_colors * sample_weights;
            out_color.a = 1.0;
        }
        "##,
    )?;

    let model_view_projection_loc =
        shader.uniform_location::<glam::Mat4>("model_view_projection")?;
    let model_matrix_loc = shader.uniform_location::<glam::Mat4>("model_matrix")?;
    let normal_matrix_loc = shader.uniform_location::<glam::Mat4>("normal_matrix")?;
    let tex_scale_loc = shader.uniform_location::<f32>("tex_scale")?;
    let tex_blend_sharpness_loc = shader.uniform_location::<f32>("tex_blend_sharpness")?;

    shader.set_uniform(&tex_scale_loc, 0.1);
    shader.set_uniform(&tex_blend_sharpness_loc, 2.0);

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
        if input.is_key_down(&key_consts::ARROW_LEFT) {
            view = DMat4::from_rotation_z(speed * dt) * view;
        } else if input.is_key_down(&key_consts::ARROW_RIGHT) {
            view = DMat4::from_rotation_z(-speed * dt) * view;
        }

        let speed = 50.0;
        if input.is_key_down(&key_consts::ARROW_UP) {
            view = DMat4::from_translation(DVec3::new(0.0, 0.0, speed * dt)) * view;
        } else if input.is_key_down(&key_consts::ARROW_DOWN) {
            view = DMat4::from_translation(DVec3::new(0.0, 0.0, -speed * dt)) * view;
        }

        context.clear();
        let model = Mat4::IDENTITY;
        let model_view = view.as_mat4() * model;
        let model_view_projection = projection * model_view;
        shader.set_uniform(&model_view_projection_loc, model_view_projection);
        shader.set_uniform(&model_matrix_loc, model);
        shader.set_uniform(&normal_matrix_loc, model.inverse().transpose());
        context.draw(&shader, &[Some(&texture)], &vao);
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
