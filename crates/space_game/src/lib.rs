use std::f64::consts::PI;

use dom::{open_websocket, spawn, InputEventListener, Key};
use glam::{Mat3, Vec2, Vec4, DVec2, Vec3, Mat4, DVec3, DMat4};
use log::info;
use render::{Attribute, Context, DataType, MeshBuilder, Sampler2D, Shader, Texture};
use wasm_bindgen::prelude::*;

mod dom;
mod render;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    console_log::init().unwrap();
    spawn(main_render());
    spawn(main_net());
}

pub async fn main_render() -> Result<(), JsValue> {
    dom::content_loaded().await?;
    let input = InputEventListener::from_canvas("space_game")?;
    let context = Context::from_canvas("space_game")?;

    let texture = Texture::load(&context, "floors.png").await?;

    let attributes = &[
        Attribute {
            name: "vert_uv".to_string(),
            type_: DataType::Vec2,
        },
        Attribute {
            name: "vert_pos".to_string(),
            type_: DataType::Vec3,
        },
    ];

    let shader = Shader::compile(
        &context,
        attributes,
        r##"#version 300 es
        uniform mat4x4 model_view_projection;
        
        in vec3 vert_pos;
        in vec2 vert_uv;
        
        out vec2 frag_uv;

        void main() { 
            gl_Position = model_view_projection * vec4(vert_pos.x, vert_pos.y, vert_pos.z, 1.0);
            frag_uv = vert_uv;
        }
        "##,
        r##"#version 300 es
    
        precision highp float;

        uniform sampler2D sampler;
        in vec2 frag_uv;
        out vec4 outColor;
        
        void main() {
            outColor = texture(sampler, frag_uv);
        }
        "##,
    )?;

    let model_view_projection_loc =
        shader.uniform_location::<glam::Mat4>("model_view_projection")?;
    let sampler_loc = shader.uniform_location::<Sampler2D>("sampler")?;
    shader.set_uniform(&sampler_loc, Sampler2D(0));

    let mut builder = MeshBuilder::new(attributes);
    builder.push(Vec2::new(0.0, 1.0));
    builder.push(Vec3::new(-0.5, 0.5, 0.0));
    builder.end_vert();
    builder.push(Vec2::new(0.0, 0.0));
    builder.push(Vec3::new(-0.5, -0.5, 0.0));
    let v1 = builder.end_vert();
    builder.push(Vec2::new(1.0, 1.0));
    builder.push(Vec3::new(0.5, 0.5, 0.0));
    let v2 = builder.end_vert();
    builder.dup_vert(v1);
    builder.dup_vert(v2);
    builder.push(Vec2::new(1.0, 0.0));
    builder.push(Vec3::new(0.5, -0.5, 0.0));
    builder.end_vert();
    let mesh = builder.build(&context)?;

    let canvas = context.canvas();
    let aspect_ratio = (canvas.width() as f32) / (canvas.height() as f32);
    let projection = Mat4::perspective_rh_gl(
        (75.0f32).to_radians(),
        aspect_ratio,
        1.0,
        1000.0,
    );

    let mut view = DMat4::look_at_rh(DVec3::new(0.0, 0.0, 10.0), DVec3::ZERO, DVec3::Y);
    let mut prev_time = animation_frame_seconds().await?;
    loop {
        let time = animation_frame_seconds().await?;
        let dt = time - prev_time;
        prev_time = time;

        let speed = 50.0;
        if input.is_key_down(Key::ArrowUp) {
            view = DMat4::from_translation(DVec3::new(0.0, 0.0, speed * dt)) * view;
        } else if input.is_key_down(Key::ArrowDown) {
            view = DMat4::from_translation(DVec3::new(0.0, 0.0, -speed * dt)) * view;
        }

        let turn_speed = PI;
        if input.is_key_down(Key::ArrowLeft) {
            view = DMat4::from_rotation_y(-turn_speed * dt) * view;
        } else if input.is_key_down(Key::ArrowRight) {
            view = DMat4::from_rotation_y(turn_speed * dt) * view;
        }

        let model = Mat4::from_scale(Vec3::new(64.0, 64.0, 64.0));
        shader.set_uniform(&model_view_projection_loc, projection * view.as_mat4() * model);
        context.draw(&shader, &[Some(&texture)], &mesh);
    }
}

async fn animation_frame_seconds() -> Result<f64, JsValue> {
    Ok(dom::animation_frame().await? / 1e3)
}

pub async fn main_net() -> Result<(), JsValue> {
    info!("Creating websocket");
    let ws = open_websocket("ws://localhost:8000/ws/v1").await?;
    info!("Websocket connected");
    ws.send_with_str("Hello World")?;
    Ok(())
}
