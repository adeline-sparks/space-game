use glam::{Mat3, Vec2};
use js_sys::Float32Array;
use render::{make_program, make_vao, VertexAttribute, ShaderType, ShaderFormat, animation_frame, dom_content_loaded, load_texture, Uniform};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::spawn_local;
use web_sys::{WebGl2RenderingContext, WebGlTexture};

mod render;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    console_log::init().unwrap();
    spawn_local(async {
        dom_content_loaded().await;
        let context = get_context();
        let texture = load_texture(&context, "floors.png").await.unwrap();
        let draw_quad = make_draw_quad(&context, &texture);

        loop {
            let time = animation_frame().await;
            context.clear_color(0.0, 0.0, 0.0, 1.0);
            context.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
            let canvas = context
                .canvas()
                .unwrap()
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .unwrap();
            context.viewport(0, 0, canvas.width() as i32, canvas.height() as i32);

            let projection =
                Mat3::from_scale(1.0f32 / Vec2::new(canvas.width() as f32, canvas.height() as f32));
            draw_quad(time, &projection);
        }
    });
}

fn make_draw_quad(context: &WebGl2RenderingContext, texture: &WebGlTexture) -> impl Fn(f64, &Mat3) {
    let vert_format = ShaderFormat {
        attributes: vec![
            VertexAttribute {
                name: "vert_pos".to_string(),
                type_: ShaderType::Vec2,
            },
            VertexAttribute {
                name: "vert_uv".to_string(),
                type_: ShaderType::Vec2,
            },
        ],
        uniforms: vec![
            Uniform {
                name: "model_view_projection".to_string(),
                type_: ShaderType::Mat3x3,
            },
            Uniform {
                name: "sampler".to_string(),
                type_: ShaderType::Sampler2D,
            }
        ]
    };

    let program = make_program(
        &context,
        &vert_format,
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

    let vertices: &[f32] = &[
        -0.5, 0.5, 
        0.0, 1.0,
        -0.5, -0.5, 
        0.0, 0.0,
        0.5, 0.5, 
        1.0, 1.0,
        0.5, -0.5,
        1.0, 0.0,
    ];
    let buffer = context.create_buffer().expect("failed to create buffer");
    context.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&buffer));
    context.buffer_data_with_array_buffer_view(
        WebGl2RenderingContext::ARRAY_BUFFER,
        &Float32Array::from(vertices.as_ref()),
        WebGl2RenderingContext::STATIC_DRAW,
    );

    let vao = make_vao(&context, &vert_format, &buffer).expect("failedc to create vao");

    let model_view_projection_loc = context
        .get_uniform_location(&program, "model_view_projection")
        .expect("failed to get uniform location of model_view_projection");
    let sampler_loc = context
        .get_uniform_location(&program, "sampler")
        .expect("failed to get uniform location of sampler");

    let context = context.clone();
    let texture = texture.clone();
    move |time: f64, projection: &Mat3| {
        context.use_program(Some(&program));
        let model_view = Mat3::from_angle(time as f32) * Mat3::from_scale(Vec2::new(64.0, 64.0));
        context.uniform_matrix3fv_with_f32_array(
            Some(&model_view_projection_loc),
            false,
            (*projection * model_view).as_ref(),
        );
        context.bind_vertex_array(Some(&vao));

        context.active_texture(WebGl2RenderingContext::TEXTURE0);
        context.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
        context.uniform1i(Some(&sampler_loc), 0);

        let vert_count = (vertices.len() / 4) as i32;
        context.draw_arrays(WebGl2RenderingContext::TRIANGLE_STRIP, 0, vert_count);
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
