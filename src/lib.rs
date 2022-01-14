use std::{cell::RefCell, rc::Rc};

use glam::{Mat3, Vec2};
use js_sys::Float32Array;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{WebGl2RenderingContext, WebGlProgram, WebGlShader};

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();

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

        let projection = Mat3::from_scale(1.0f32 / Vec2::new(canvas.width() as f32, canvas.height() as f32));
        draw_quad(time, &projection);

        request_animation_frame(cb);
    });

    request_animation_frame(&cb);
}

fn make_draw_quad(context: &WebGl2RenderingContext) -> impl Fn(f64, &Mat3) {
    let vert_shader = compile_shader(
        &context,
        WebGl2RenderingContext::VERTEX_SHADER,
        r##"#version 300 es
    
        in vec2 position;
        uniform mat3x3 model_view_projection;

        void main() { 
            gl_Position.xyw = model_view_projection * vec3(position.x, position.y, 1.0);
            gl_Position.z = 0.0;
        }
        "##,
    )
    .expect("failed to compile vert_shader");

    let frag_shader = compile_shader(
        &context,
        WebGl2RenderingContext::FRAGMENT_SHADER,
        r##"#version 300 es
    
        precision highp float;
        out vec4 outColor;
        
        void main() {
            outColor = vec4(1.0, 1.0, 1.0, 1.0);
        }
        "##,
    )
    .expect("failed to compile frag_shader");

    let program =
        link_program(&context, &vert_shader, &frag_shader).expect("failed to link program");
    context.use_program(Some(&program));

    let buffer = context.create_buffer().expect("failed to create buffer");
    context.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&buffer));

    let vertices: [f32; 8] = [-0.5, 0.5, -0.5, -0.5, 0.5, 0.5, 0.5, -0.5];
    context.buffer_data_with_array_buffer_view(
        WebGl2RenderingContext::ARRAY_BUFFER,
        &Float32Array::from(vertices.as_ref()),
        WebGl2RenderingContext::STATIC_DRAW,
    );

    let position_attribute_location = context.get_attrib_location(&program, "position") as u32;
    context.vertex_attrib_pointer_with_i32(position_attribute_location, 2, WebGl2RenderingContext::FLOAT, false, 0, 0);
    context.enable_vertex_attrib_array(position_attribute_location);

    let model_view_projection_loc = context
        .get_uniform_location(&program, "model_view_projection")
        .expect("failed to get uniform location");

    let context = context.clone();
    move |time:f64, projection:&Mat3| {
        let model_view = 
            Mat3::from_angle(time as f32) * 
            Mat3::from_scale(Vec2::new(200.0, 200.0));
        context.uniform_matrix3fv_with_f32_array(
            Some(&model_view_projection_loc),
            false,
            (*projection * model_view).as_ref(),
        );
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

fn compile_shader(
    context: &WebGl2RenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader, String> {
    let shader = context
        .create_shader(shader_type)
        .ok_or_else(|| String::from("Unable to create shader object"))?;
    context.shader_source(&shader, source);
    context.compile_shader(&shader);

    if context
        .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(context
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| String::from("Unknown error creating shader")))
    }
}

fn link_program(
    context: &WebGl2RenderingContext,
    vert_shader: &WebGlShader,
    frag_shader: &WebGlShader,
) -> Result<WebGlProgram, String> {
    let program = context
        .create_program()
        .ok_or_else(|| String::from("Unable to create shader object"))?;

    context.attach_shader(&program, vert_shader);
    context.attach_shader(&program, frag_shader);
    context.link_program(&program);

    if context
        .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(context
            .get_program_info_log(&program)
            .unwrap_or_else(|| String::from("Unknown error creating program object")))
    }
}
