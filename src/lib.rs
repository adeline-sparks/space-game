use glam::{Mat3, Vec2};
use js_sys::Float32Array;
use render::{make_program, make_vao, VertexAttribute, VertexAttributeType, VertexFormat, animation_frame, dom_content_loaded, load_texture};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::spawn_local;
use web_sys::WebGl2RenderingContext;

mod render;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    console_log::init().unwrap();
    spawn_local(async {
        dom_content_loaded().await;
        let context = get_context();
        let draw_quad = make_draw_quad(&context);
        let _tex = load_texture(&context, "floors.png").await.unwrap();

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

fn make_draw_quad(context: &WebGl2RenderingContext) -> impl Fn(f64, &Mat3) {
    let vert_format = VertexFormat {
        attributes: vec![
            VertexAttribute {
                name: String::from("vert_pos"),
                type_: VertexAttributeType::Vec2,
            },
            VertexAttribute {
                name: String::from("vert_uv"),
                type_: VertexAttributeType::Vec2,
            }
        ],
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
        in vec2 frag_uv;
        out vec4 outColor;
        
        void main() {
            outColor.rg = frag_uv;
            outColor.b = 0.0;
            outColor.a = 1.0;
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
        .expect("failed to get uniform location");

    let context = context.clone();
    move |time: f64, projection: &Mat3| {
        context.use_program(Some(&program));
        let model_view = Mat3::from_angle(time as f32) * Mat3::from_scale(Vec2::new(200.0, 200.0));
        context.uniform_matrix3fv_with_f32_array(
            Some(&model_view_projection_loc),
            false,
            (*projection * model_view).as_ref(),
        );
        context.bind_vertex_array(Some(&vao));
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
