use wasm_bindgen::{JsCast, JsValue};

use web_sys::{HtmlCanvasElement, WebGl2RenderingContext};

use crate::dom;

mod mesh;
mod shader;
mod texture;

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

pub struct Context {
    gl: WebGl2RenderingContext,
    canvas: HtmlCanvasElement,
}

impl Context {
    pub fn from_canvas(element_id: &str) -> Result<Self, JsValue> {
        let canvas = dom::get_canvas(element_id)?;
        let gl = canvas
            .get_context("webgl2")?
            .ok_or_else(|| JsValue::from("Failed to get WebGl2RenderingContext"))?
            .dyn_into::<WebGl2RenderingContext>()?;
        Ok(Context { gl, canvas })
    }

    pub fn canvas(&self) -> &HtmlCanvasElement {
        &self.canvas
    }

    pub fn clear(&self, clear_color: &glam::Vec4) {
        self.gl
            .clear_color(clear_color.x, clear_color.y, clear_color.z, clear_color.w);
        self.gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
    }

    pub fn draw(&self, shader: &Shader, textures: &[Option<&Texture>], mesh: &Mesh) {
        self.gl.viewport(
            0,
            0,
            self.canvas.width() as i32,
            self.canvas.height() as i32,
        );
        shader.use_(&self.gl);
        Texture::bind(textures, &self.gl);
        mesh.draw(&self.gl);
    }
}
