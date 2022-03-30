use thiserror::Error;
use wasm_bindgen::JsCast;

use web_sys::{HtmlCanvasElement, WebGl2RenderingContext};

use crate::dom::{self, DomError};
use crate::mesh::{AttributeType};

mod shader;
mod texture;
mod draw;
mod buffer;

pub use shader::{Sampler2D, Shader, ShaderLoader, Uniform};
pub use texture::Texture;
pub use draw::DrawPrimitives;
pub use buffer::PrimitiveBuffer;

pub struct Context {
    gl: WebGl2RenderingContext,
    canvas: HtmlCanvasElement,
}

#[derive(Error, Debug)]
pub enum ContextError {
    #[error("Failed to get WebGl2RenderingContext")]
    GetContextFailed,
    #[error(transparent)]
    DomError(#[from] DomError),
}

impl Context {
    pub fn from_canvas(element_id: &str) -> Result<Self, ContextError> {
        let canvas = dom::get_canvas(element_id)?;
        let gl = canvas
            .get_context("webgl2")
            .map_err(DomError::from)?
            .ok_or(ContextError::GetContextFailed)?
            .unchecked_into::<WebGl2RenderingContext>();
        gl.front_face(WebGl2RenderingContext::CW);

        Ok(Context { gl, canvas })
    }

    pub fn canvas(&self) -> &HtmlCanvasElement {
        &self.canvas
    }

    pub fn clear(&self) {
        self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
        self.gl.clear(
            WebGl2RenderingContext::COLOR_BUFFER_BIT | WebGl2RenderingContext::DEPTH_BUFFER_BIT,
        );
    }
}

fn webgl_type(type_: AttributeType) -> u32 {
    match type_ {
        AttributeType::Vec2 => WebGl2RenderingContext::FLOAT_VEC2,
        AttributeType::Vec3 => WebGl2RenderingContext::FLOAT_VEC3,
    }
}

fn webgl_scalar_count(type_: AttributeType) -> i32 {
    match type_ {
        AttributeType::Vec2 => 2,
        AttributeType::Vec3 => 3,
    }
}

fn webgl_scalar_type(type_: AttributeType) -> u32 {
    match type_ {
        AttributeType::Vec2 => WebGl2RenderingContext::FLOAT,
        AttributeType::Vec3 => WebGl2RenderingContext::FLOAT,
    }
}
