use std::{collections::{HashMap, HashSet}, marker::PhantomData};

use js_sys::{Promise, Function, Uint8Array, Uint16Array};
use wasm_bindgen::{JsValue, JsCast};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    WebGl2RenderingContext, WebGlBuffer, WebGlProgram, WebGlShader, WebGlVertexArrayObject, WebGlTexture, Window, Document, WebGlUniformLocation,
};

mod mesh;
mod shader;

pub use mesh::{AttributeFormat, Mesh, MeshBuilder};
pub use shader::{Shader, ShaderFormat, UniformFormat, Uniform, Sampler2D};


#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RenderType {
    Float,
    Vec2,
    Vec3,
    Mat3x3,
    Int,
    Sampler2D,
}

impl ShaderFormat {
    pub fn new(attributes: Vec<AttributeFormat>, uniforms: Vec<UniformFormat>) -> Self {
        let attribute_map = attributes
            .iter()
            .enumerate()
            .map(|(i, attr)| (attr.name.clone(), i))
            .collect();
        let uniform_map = uniforms
            .into_iter()
            .map(|uniform| (uniform.name.clone(), uniform))
            .collect();
        ShaderFormat { attributes, attribute_map, uniform_map }
    }

    pub fn vertex_bytes(&self) -> usize {
        self.attributes
            .iter()
            .map(|attr| attr.type_.num_bytes())
            .sum()
    }
}

impl RenderType {
    pub fn num_components(self) -> u32 {
        match self {
            Self::Float => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Mat3x3 => 9,
            Self::Int => 1,
            Self::Sampler2D => todo!(),
        }
    }

    pub fn num_bytes(self) -> usize {
        match self {
            Self::Float => 4,
            Self::Vec2 => 8,
            Self::Vec3 => 12,
            Self::Mat3x3 => 36,
            Self::Int => 4,
            Self::Sampler2D => todo!(),
        }
    }

    pub fn webgl_scalar_type(self) -> u32 {
        match self {
            Self::Float |
            Self::Vec2 |
            Self::Vec3 |
            Self::Mat3x3 => WebGl2RenderingContext::FLOAT,
            Self::Int => WebGl2RenderingContext::INT,
            Self::Sampler2D => todo!(),
        }
    }

    pub fn webgl_type(self) -> u32 {
        match self {
            Self::Float => WebGl2RenderingContext::FLOAT,
            Self::Vec2 => WebGl2RenderingContext::FLOAT_VEC2,
            Self::Vec3 => WebGl2RenderingContext::FLOAT_VEC3,
            Self::Mat3x3 => WebGl2RenderingContext::FLOAT_MAT3,
            Self::Int => WebGl2RenderingContext::INT,
            Self::Sampler2D => WebGl2RenderingContext::SAMPLER_2D,
        }
    }
}

#[derive(Clone)]
pub struct Texture(WebGlTexture);

impl Texture {
    pub async fn load(context: &Context, src: &str) -> Result<Texture, String> {
        let context = &context.0;
        let image = web_sys::HtmlImageElement::new()
            .expect("Failed to create HtmlImageElement");
        image.set_src(src);
        future_from_callback(|cb| {
            image.add_event_listener_with_callback("load", &cb)
                .expect("Failed to register for image load event");
            image.add_event_listener_with_callback("error", &cb)
                .expect("Failed to register for image error event"); 
        }).await;
        
        if !image.complete() || image.natural_height() == 0 {
            return Err("Failed to load image".to_string());
        }
    
        let texture = context.create_texture()
            .ok_or_else(|| "Failed to `create_texture`".to_string())?;
        context.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
        context.tex_image_2d_with_u32_and_u32_and_html_image_element(
            WebGl2RenderingContext::TEXTURE_2D,
            0,
            WebGl2RenderingContext::RGBA as i32,
            WebGl2RenderingContext::RGBA,
            WebGl2RenderingContext::UNSIGNED_BYTE,
            &image,
        ).map_err(|_| "Failed to `tex_image_2d`".to_string())?;
        context.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D, 
            WebGl2RenderingContext::TEXTURE_MIN_FILTER, 
            WebGl2RenderingContext::NEAREST as i32);
        context.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D, 
            WebGl2RenderingContext::TEXTURE_MAG_FILTER, 
            WebGl2RenderingContext::NEAREST as i32);
            
        Ok(Texture(texture))
    }
}

pub struct Context(WebGl2RenderingContext);

impl Context {
    pub fn from_canvas(element_id: &str) -> Result<Self, String> {
        Ok(Context(expect_document()
            .get_element_by_id(element_id)
            .ok_or_else(|| format!("get_element_by_id failed for `{}`", element_id))?
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .map_err(|_| format!("`{}` is not a canvas", element_id))?
            .get_context("webgl2")
            .ok()
            .flatten()
            .and_then(|o| o.dyn_into::<WebGl2RenderingContext>().ok())
            .ok_or_else(|| "Failed to get webgl2 context".to_string())?
        ))
    }

    pub fn begin(&self, clear_color: &glam::Vec4) {
        self.0.clear_color(0.0, 0.0, 0.0, 1.0);
        self.0.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
        let (width, height) = self.size();
        self.0.viewport(0, 0, width as i32, height as i32);
    }

    pub fn size(&self) -> (u32, u32) {
        let canvas = self.0
            .canvas()
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();
        (canvas.width(), canvas.height())
    }

    pub fn draw(&self, shader: &Shader, mesh: &Mesh, textures: &[Option<&Texture>]) {
        shader.use_(&self.0);

        for (i, texture) in textures.iter().enumerate() {
            if let Some(texture) = texture {
                self.0.active_texture(WebGl2RenderingContext::TEXTURE0 + (i as u32));
                self.0.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture.0));
            }
        }

        mesh.draw(&self.0);
    }
}

pub async fn dom_content_loaded() {
    if expect_document().ready_state() != "loading" {
        return;
    }

    future_from_callback(|resolve| {
        expect_window()
            .add_event_listener_with_callback("DOMContentLoaded", &resolve)
            .expect("Failed to add DOMContentLoaded event handler");
    }).await;
}

pub async fn animation_frame() -> f64 {
    future_from_callback(|cb| {
        expect_window()
            .request_animation_frame(&cb)
            .expect("Failed to `request_animation_frame`");
    }).await
        .as_f64()
        .expect("request_animation_frame did not provide a float")
        / 1e3
}

async fn future_from_callback(mut setup: impl FnMut(Function)) -> JsValue {
    JsFuture::from(Promise::new(&mut |resolve, _reject| setup(resolve)))
        .await
        .expect("Promise did not resolve")
}

fn expect_window() -> Window {
    web_sys::window().expect("Global `window` does not exist")
}

fn expect_document() -> Document {
    expect_window().document().expect("Global `document` does not exist")
}