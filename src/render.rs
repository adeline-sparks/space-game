use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

use js_sys::{Function, Promise, Uint16Array, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Document, WebGl2RenderingContext, WebGlBuffer, WebGlProgram, WebGlShader, WebGlUniformLocation,
    WebGlVertexArrayObject, Window,
};

mod dom;
mod mesh;
mod shader;
mod texture;

pub use dom::{animation_frame, dom_content_loaded};
pub use mesh::{Attribute, Mesh, MeshBuilder};
pub use shader::{Sampler2D, Shader, Uniform};
pub use texture::Texture;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DataType {
    Float,
    Vec2,
    Vec3,
    Mat3x3,
    Int,
    Sampler2D,
}

impl DataType {
    pub fn num_components(self) -> u32 {
        match self {
            Self::Float => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Mat3x3 => 9,
            Self::Int => 1,
            Self::Sampler2D => 1,
        }
    }

    pub fn num_bytes(self) -> usize {
        match self {
            Self::Float => 4,
            Self::Vec2 => 8,
            Self::Vec3 => 12,
            Self::Mat3x3 => 36,
            Self::Int => 4,
            Self::Sampler2D => 4,
        }
    }

    pub fn webgl_scalar_type(self) -> u32 {
        match self {
            Self::Float | Self::Vec2 | Self::Vec3 | Self::Mat3x3 => WebGl2RenderingContext::FLOAT,
            Self::Int => WebGl2RenderingContext::INT,
            Self::Sampler2D => WebGl2RenderingContext::SAMPLER_2D,
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

pub struct Context(WebGl2RenderingContext);

impl Context {
    pub fn from_canvas(element_id: &str) -> Result<Self, String> {
        Ok(Context(
            dom::get_canvas(element_id)?
                .get_context("webgl2")
                .ok()
                .flatten()
                .and_then(|o| o.dyn_into::<WebGl2RenderingContext>().ok())
                .ok_or_else(|| "Failed to get webgl2 context".to_string())?,
        ))
    }

    pub fn begin(&self, clear_color: &glam::Vec4) {
        self.0.clear_color(0.0, 0.0, 0.0, 1.0);
        self.0.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
        let (width, height) = self.size();
        self.0.viewport(0, 0, width as i32, height as i32);
    }

    pub fn size(&self) -> (u32, u32) {
        let canvas = self
            .0
            .canvas()
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();
        (canvas.width(), canvas.height())
    }

    pub fn draw(&self, shader: &Shader, mesh: &Mesh, textures: &[Option<&texture::Texture>]) {
        shader.use_(&self.0);
        Texture::bind(textures, &self.0);

        mesh.draw(&self.0);
    }
}
