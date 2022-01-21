use glam::{Mat3, Vec2};
use render::{VertexAttribute, ShaderType, ShaderFormat, animation_frame, dom_content_loaded, load_texture, Uniform, Shader, Sampler2D, MeshBuilder, Context};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::spawn_local;
use web_sys::{WebGl2RenderingContext, WebGlTexture};

#[allow(unused)]
mod render;

#[allow(unused)]
mod state;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    console_log::init().unwrap();
    spawn_local(async {
        dom_content_loaded().await;
        let context = Context::from_canvas("spacegame").unwrap();
        let texture = load_texture(&context.0, "floors.png").await.unwrap();
        let draw_quad = make_draw_quad(&context.0, &texture);

        loop {
            let time = animation_frame().await;
            context.0.clear_color(0.0, 0.0, 0.0, 1.0);
            context.0.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
            let canvas = context.0
                .canvas()
                .unwrap()
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .unwrap();
            context.0.viewport(0, 0, canvas.width() as i32, canvas.height() as i32);

            let projection =
                Mat3::from_scale(1.0f32 / Vec2::new(canvas.width() as f32, canvas.height() as f32));
            draw_quad(time, &projection);
        }
    });
}

fn make_draw_quad(context: &WebGl2RenderingContext, texture: &WebGlTexture) -> impl Fn(f64, &Mat3) {
    let format = ShaderFormat::new(
        vec![
            VertexAttribute {
                name: "vert_pos".to_string(),
                type_: ShaderType::Vec2,
            },
            VertexAttribute {
                name: "vert_uv".to_string(),
                type_: ShaderType::Vec2,
            },
        ],
        vec![
            Uniform {
                name: "model_view_projection".to_string(),
                type_: ShaderType::Mat3x3,
            },
            Uniform {
                name: "sampler".to_string(),
                type_: ShaderType::Sampler2D,
            }
        ],
    );

    let shader = Shader::compile(
        &context,
        format,
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

    let model_view_projection_loc = shader.uniform_location::<glam::Mat3>(context, "model_view_projection")
        .expect("failed to get uniform location of model_view_projection");
    let sampler_loc = shader.uniform_location::<Sampler2D>(context, "sampler")
        .expect("failed to get uniform location of sampler");
    shader.set_uniform(context, &sampler_loc, Sampler2D(0));

    let mut builder = MeshBuilder::new(&shader.format().attributes);
    builder.push(Vec2::new(-0.5, 0.5));
    builder.push(Vec2::new(0.0, 1.0));
    builder.push(Vec2::new(-0.5, -0.5));
    builder.push(Vec2::new(0.0, 0.0));
    builder.push(Vec2::new(0.5, 0.5));
    builder.push(Vec2::new(1.0, 1.0));
    builder.push(Vec2::new(0.5, -0.5));
    builder.push(Vec2::new(1.0, 0.0));
    let mesh = builder.build(&context).expect("failed to build Mesh");

    let context = context.clone();
    let texture = texture.clone();
    move |time: f64, projection: &Mat3| {
        let model_view = Mat3::from_angle(time as f32) * Mat3::from_scale(Vec2::new(64.0, 64.0));
        shader.set_uniform(&context, &model_view_projection_loc, *projection * model_view);
        shader.render(&context, &mesh, &[&texture]);
    }
}

fn get_context() -> WebGl2RenderingContext {
    web_sys::window()
        .expect("no global `window` exists")
        .document()
        .expect("no `document` exists")
        .get_element_by_id("space_game")
        .expect("no element named `space_game` exists")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("`space_game` element is not a canvas")
        .get_context("webgl2")
        .expect("failed to get webgl2 context")
        .expect("failed to get webgl2 context")
        .dyn_into::<WebGl2RenderingContext>()
        .unwrap()
}
