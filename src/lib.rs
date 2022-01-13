use std::{cell::RefCell, rc::Rc};

use js_sys::Float32Array;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{WebGl2RenderingContext, WebGlProgram, WebGlShader};

#[wasm_bindgen(start)]
pub fn main() {
    let document = web_sys::window()
        .expect("no global `window` exists")
        .document()
        .expect("no `document` exists");
    let canvas = document.get_element_by_id("adel")
        .expect("no element named `adel` exists")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("`adel` element is not a canvas");
    
    let context = canvas
        .get_context("webgl2")
        .expect("failed to get webgl2 context")
        .expect("failed to get webgl2 context")
        .dyn_into::<WebGl2RenderingContext>()
        .unwrap();

    let vert_shader = compile_shader(
        &context,
        WebGl2RenderingContext::VERTEX_SHADER,
        r##"#version 300 es
    
        in vec2 position;
        uniform vec2 rot_vec;
        out vec2 clip_pos;

        void main() { 
            vec2 rot_position = (position.x * rot_vec) + (position.y * vec2(-rot_vec.y, rot_vec.x));
            gl_Position = vec4(rot_position, 0.0, 1.0);
            clip_pos = position;
        }
        "##,
    ).expect("failed to compile vert_shader");

    let frag_shader = compile_shader(
        &context,
        WebGl2RenderingContext::FRAGMENT_SHADER,
        r##"#version 300 es
    
        precision highp float;
        in vec2 clip_pos;
        out vec4 outColor;
        
        void main() {
            outColor = vec4(clip_pos.xy * 0.5 + 0.5, 0.0, 1);
        }
        "##,
    ).expect("failed to compile frag_shader");

    let program = link_program(&context, &vert_shader, &frag_shader)
        .expect("failed to link program");
    context.use_program(Some(&program));

    let buffer = context.create_buffer().expect("failed to create buffer");
    context.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&buffer));

    let vertices: [f32; 6] = [-0.7, -0.7, 0.7, -0.7, 0.0, 0.7];
    context.buffer_data_with_array_buffer_view(
        WebGl2RenderingContext::ARRAY_BUFFER,
        &Float32Array::from(vertices.as_ref()),
        WebGl2RenderingContext::STATIC_DRAW,
    );

    let position_attribute_location = context.get_attrib_location(&program, "position");
    context.vertex_attrib_pointer_with_i32(0, 2, WebGl2RenderingContext::FLOAT, false, 0, 0);
    context.enable_vertex_attrib_array(position_attribute_location as u32);

    let rot_uniform_location = context.get_uniform_location(&program, "rot_vec")
        .expect("failed to get uniform location");

    let cb = make_callback(move |cb, mut time| {
        time /= 1e3;
        context.clear_color(0.0, 0.0, 0.0, 1.0);
        context.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);

        let vert_count = (vertices.len() / 2) as i32;
        context.uniform2f(Some(&rot_uniform_location), time.cos() as f32, time.sin() as f32);
        context.draw_arrays(WebGl2RenderingContext::TRIANGLES, 0, vert_count);

        request_animation_frame(cb);
    });

    request_animation_frame(&cb);
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

