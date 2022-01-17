use std::{cell::RefCell, rc::Rc};

use glam::{Mat3, Vec2};
use js_sys::Float32Array;
use log::error;
use render::{make_program, make_vao, VertexAttribute, VertexAttributeType, VertexFormat};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::WebGl2RenderingContext;

mod render;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    console_log::init().unwrap();

    let context = get_context();
    let draw_quad = make_draw_quad(&context);

    let cb = make_callback(move |cb, time_ms| {
        let time = time_ms / 1e3;
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

        request_animation_frame(cb);
    });

    request_animation_frame(&cb);
}

fn make_draw_quad(context: &WebGl2RenderingContext) -> impl Fn(f64, &Mat3) {
    let vert_format = VertexFormat {
        attributes: vec![
            VertexAttribute {
                name: String::from("position"),
                type_: VertexAttributeType::Vec2,
            },
            VertexAttribute {
                name: String::from("color"),
                type_: VertexAttributeType::Vec3,
            }
        ],
    };

    let program = make_program(
        &context,
        &vert_format,
        r##"#version 300 es
    
        in vec2 position;
        in vec3 color_vert;
        uniform mat3x3 model_view_projection;
        out vec3 color;

        void main() { 
            gl_Position.xyw = model_view_projection * vec3(position.x, position.y, 1.0);
            gl_Position.z = 0.0;
            color = color_vert;
        }
        "##,
        r##"#version 300 es
    
        precision highp float;
        in vec3 color;
        out vec4 outColor;
        
        void main() {
            outColor.rgb = color;
            outColor.a = 1.0;
        }
        "##,
    )
    .expect("failed to compile program");

    let vertices: &[f32] = &[
        -0.5, 0.5, 
        1.0, 1.0, 1.0,
        -0.5, -0.5, 
        1.0, 0.0, 0.0,
        0.5, 0.5, 
        0.0, 1.0, 0.0,
        0.5, -0.5,
        0.0, 0.0, 1.0,
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
        let vert_count = (vertices.len() / 2) as i32;
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

type Callback = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

fn make_callback<F: FnMut(&Callback, f64) + 'static>(mut f: F) -> Callback {
    let cb = Rc::new(RefCell::new(None));
    let cb2 = cb.clone();
    let body = move |t: f64| f(&cb, t);
    *cb2.borrow_mut() = Some(Closure::wrap(Box::new(body) as Box<dyn FnMut(f64)>));
    cb2
}

fn request_animation_frame(f: &Callback) {
    web_sys::window()
        .expect("`window` failed")
        .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
        .expect("`requestAnimationFrame` failed");
}
