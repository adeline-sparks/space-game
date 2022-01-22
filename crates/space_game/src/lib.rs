use dom::open_websocket;
use glam::{Mat3, Vec2, Vec4};
use log::info;
use render::{Attribute, Context, DataType, MeshBuilder, Sampler2D, Shader, Texture};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

mod dom;
mod render;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    console_log::init().unwrap();
    spawn_local(main_render());
    spawn_local(main_net());
}

pub async fn main_render() {
    dom::content_loaded().await;
    let context = Context::from_canvas("space_game").unwrap();

    let texture = Texture::load(&context, "floors.png").await.unwrap();

    let attributes = &[
        Attribute {
            name: "vert_uv".to_string(),
            type_: DataType::Vec2,
        },
        Attribute {
            name: "vert_pos".to_string(),
            type_: DataType::Vec2,
        },
        Attribute {
            name: "vert_extra".to_string(),
            type_: DataType::Float,
        },
    ];

    let shader = Shader::compile(
        &context,
        attributes,
        r##"#version 300 es
        uniform mat3x3 model_view_projection;
        
        in vec2 vert_pos;
        in vec2 vert_uv;
        
        out vec2 frag_uv;

        void main() { 
            gl_Position.xyw = model_view_projection * vec3(vert_pos.x, vert_pos.y, 1.0);
            gl_Position.z = 0.0;
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
    )
    .expect("failed to compile program");

    let model_view_projection_loc = shader
        .uniform_location::<glam::Mat3>("model_view_projection")
        .expect("failed to get uniform location of model_view_projection");
    let sampler_loc = shader
        .uniform_location::<Sampler2D>("sampler")
        .expect("failed to get uniform location of sampler");
    shader.set_uniform(&sampler_loc, Sampler2D(0));

    let mut builder = MeshBuilder::new(attributes);
    builder.push(Vec2::new(0.0, 1.0));
    builder.push(Vec2::new(-0.5, 0.5));
    builder.push(42.0);
    builder.end_vert();
    builder.push(Vec2::new(0.0, 0.0));
    builder.push(Vec2::new(-0.5, -0.5));
    builder.push(42.0);
    let v1 = builder.end_vert();
    builder.push(Vec2::new(1.0, 1.0));
    builder.push(Vec2::new(0.5, 0.5));
    builder.push(42.0);
    let v2 = builder.end_vert();
    builder.dup_vert(v1);
    builder.dup_vert(v2);
    builder.push(Vec2::new(1.0, 0.0));
    builder.push(Vec2::new(0.5, -0.5));
    builder.push(42.0);
    builder.end_vert();
    let mesh = builder.build(&context).expect("failed to build Mesh");

    let canvas = context.canvas();
    let projection =
        Mat3::from_scale(1.0f32 / Vec2::new(canvas.width() as f32, canvas.height() as f32));

    loop {
        let time = dom::animation_frame().await;
        context.clear(&Vec4::new(0.0, 0.0, 0.0, 1.0));

        let model_view = Mat3::from_angle(time as f32) * Mat3::from_scale(Vec2::new(64.0, 64.0));
        shader.set_uniform(&model_view_projection_loc, projection * model_view);
        context.draw(&shader, &[Some(&texture)], &mesh);
    }
}

pub async fn main_net() {
    info!("Creating websocket");
    let ws = open_websocket("ws://localhost:3030/ws/v1").await.unwrap();
    info!("Websocket connected");
    ws.send_with_str("Hello World").unwrap();
    ws.close().unwrap();
}
