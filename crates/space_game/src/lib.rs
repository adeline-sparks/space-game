#![allow(dead_code)]
use std::f64::consts::PI;

use dom::{key_consts, open_websocket, spawn, InputEventListener};
use glam::{DMat4, DQuat, DVec3, IVec3, Mat4, Vec3, Vec4};
use log::info;
use render::{Attribute, Context, DataType, MeshBuilder, MeshBuilderMode, Shader, Texture};
use wasm_bindgen::prelude::*;

mod dom;
mod render;
mod voxel;
use voxel::{marching_cubes, SignedDistanceFunction};

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    console_log::init().unwrap();
    spawn(main_render());
    spawn(main_net());
}

struct Sphere(f32);

impl SignedDistanceFunction for Sphere {
    fn value(&self, pos: Vec3) -> f32 {
        self.0 - pos.length()
    }

    fn grad(&self, pos: Vec3) -> Vec3 {
        -2.0 * pos
    }
}

async fn main_render() -> Result<(), JsValue> {
    dom::content_loaded().await?;
    let input = InputEventListener::from_canvas("space_game")?;
    let context = Context::from_canvas("space_game")?;

    let texture = Texture::load(&context, "floors.png").await?;

    let attributes = &[
        Attribute {
            name: "vert_pos".to_string(),
            type_: DataType::Vec3,
        },
        Attribute {
            name: "vert_normal".to_string(),
            type_: DataType::Vec3,
        },
    ];

    let mut builder = MeshBuilder::new(attributes, MeshBuilderMode::SOLID);
    marching_cubes(
        &Sphere(32.0),
        (
            Vec3::new(-128.0, -128.0, -128.0),
            Vec3::new(128.0, 128.0, 128.0),
        ),
        IVec3::new(32, 32, 32),
        &mut |v1, v2, v3, n1, n2, n3| {
            builder.write_attribute(v1);
            builder.write_attribute(n1);
            let i1 = builder.finish_vert();
            builder.write_attribute(v2);
            builder.write_attribute(n2);
            let i2 = builder.finish_vert();
            builder.write_attribute(v3);
            builder.write_attribute(n3);
            let i3 = builder.finish_vert();
            builder.write_triangle(i1, i2, i3);
        },
    );
    let mesh = builder.build(&context)?;

    let shader = Shader::compile(
        &context,
        attributes,
        r##"#version 300 es
        uniform mat4x4 model_view_projection;
        uniform mat4x4 normal_matrix;
        
        in vec3 vert_pos;
        in vec3 vert_normal;
        out vec3 frag_normal;

        void main() { 
            gl_Position = model_view_projection * vec4(vert_pos.x, vert_pos.y, vert_pos.z, 1.0);
            frag_normal = (normal_matrix * vec4(vert_normal.x, vert_normal.y, vert_normal.z, 0.0)).xyz;
        }
        "##,
        r##"#version 300 es
    
        precision highp float;

        in vec3 frag_normal;
        out vec4 outColor;
        
        void main() {
            outColor.rgb = frag_normal / 2.0 + vec3(0.5, 0.5, 0.5);
            outColor.a = 1.0;
        }
        "##,
    )?;

    let model_view_projection_loc =
        shader.uniform_location::<glam::Mat4>("model_view_projection")?;
    let normal_matrix_loc = shader.uniform_location::<glam::Mat4>("normal_matrix")?;
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

        context.clear(Vec4::new(0.0, 0.0, 0.0, 1.0));
        let model = Mat4::IDENTITY;
        let model_view = view.as_mat4() * model;
        let model_view_projection = projection * model_view;
        shader.set_uniform(&model_view_projection_loc, model_view_projection);
        shader.set_uniform(&normal_matrix_loc, model_view.inverse().transpose());
        context.draw(&shader, &[Some(&texture)], &mesh);
    }
}

async fn animation_frame_seconds() -> Result<f64, JsValue> {
    Ok(dom::animation_frame().await? / 1e3)
}

async fn main_net() -> Result<(), JsValue> {
    info!("Creating websocket");
    let ws = open_websocket("ws://localhost:8000/ws/v1").await?;
    info!("Websocket connected");
    ws.send_with_str("Hello World")?;
    Ok(())
}
