use thiserror::Error;
use wasm_bindgen::JsCast;
use web_sys::{WebGl2RenderingContext, WebGlVertexArrayObject, WebGlProgram, HtmlCanvasElement, WebGlTexture};

use crate::gl::{webgl_scalar_count, webgl_scalar_type, Context};
use crate::mesh::{PrimitiveType, AttributeName};

use super::shader::ShaderError;
use super::{Shader, Texture, Sampler2D};
use super::buffer::PrimitiveBuffer;

pub struct DrawPrimitives {
    pub(super) gl: WebGl2RenderingContext,
    pub(super) vao: WebGlVertexArrayObject,
    pub(super) program: WebGlProgram,
    pub(super) primitive_type: PrimitiveType,
    pub(super) textures: Vec<WebGlTexture>,
    pub(super) index_count: usize,
    pub(super) indexed: bool,
}

#[derive(Error, Debug)]
pub enum DrawError {
    #[error("Failed to create_vertex_array")]
    CreateVertexArrayFailed,
    #[error("Shader expects unknown attribute `{0}`")]
    UnknownAttribute(AttributeName),
    #[error(transparent)]
    ShaderError(#[from] ShaderError),
}

impl DrawPrimitives {
    pub fn build(
        context: &Context,
        shader: &Shader,
        vbo: &PrimitiveBuffer,
        textures: &[(&str, &Texture)],
    ) -> Result<Self, DrawError> {
        let gl = &context.gl;
        let vao = gl
            .create_vertex_array()
            .ok_or(DrawError::CreateVertexArrayFailed)?;
        gl.bind_vertex_array(Some(&vao));
        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo.vert_buffer));
        if let Some(index_buffer) = &vbo.index_buffer {
            gl.bind_buffer(WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER, Some(index_buffer));
        }

        let num_attribs = gl.get_program_parameter(&shader.program, WebGl2RenderingContext::ACTIVE_ATTRIBUTES)
            .unchecked_into_f64()
            as u32;
        for i in 0..num_attribs {
            let attrib = gl.get_active_attrib(&shader.program, i).unwrap();
            let name = AttributeName::from(attrib.name());

            // TODO type check

            let loc = gl.get_attrib_location(&shader.program, name.as_ref()).try_into().unwrap();
            let &(attr_type, offset) = vbo.layout.types_offsets
                .get(&name)
                .ok_or(DrawError::UnknownAttribute(name))?;
            
            gl.enable_vertex_attrib_array(loc);
            gl.vertex_attrib_pointer_with_i32(
                loc,
                webgl_scalar_count(attr_type),
                webgl_scalar_type(attr_type),
                false,
                vbo.layout.stride as i32,
                offset as i32,
            );
        }

        for (i, &(name, _)) in textures.iter().enumerate() {
            shader.uniform(name)?.set(&Sampler2D(i as u32));
        }
        let textures = textures.iter().map(|&(_, t)| t.texture.clone()).collect();

        Ok(DrawPrimitives {
            gl: gl.clone(),
            vao,
            program: shader.program.clone(),
            primitive_type: vbo.primitive_type,
            textures,
            index_count: vbo.index_count,
            indexed: vbo.index_buffer.is_some(),
        })
    }

    pub fn draw(&self) {
        let canvas: HtmlCanvasElement = self.gl.canvas().unwrap().dyn_into().unwrap();
        self.gl.enable(WebGl2RenderingContext::DEPTH_TEST);
        self.gl.enable(WebGl2RenderingContext::CULL_FACE);
        self.gl.viewport(
            0,
            0,
            canvas.width() as i32,
            canvas.height() as i32,
        );

        self.gl.use_program(Some(&self.program));
        self.gl.bind_vertex_array(Some(&self.vao));

        for (i, texture) in self.textures.iter().enumerate() {
            self.gl.active_texture(WebGl2RenderingContext::TEXTURE0 + (i as u32));
            self.gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(texture));
        }

        let mode = match self.primitive_type {
            PrimitiveType::LINES => WebGl2RenderingContext::LINES,
            PrimitiveType::TRIANGLES => WebGl2RenderingContext::TRIANGLES,
        };
        let count = self.index_count as i32;

        if self.indexed {
            self.gl.draw_elements_with_i32(
                mode,
                count,
                WebGl2RenderingContext::UNSIGNED_SHORT,
                0,
            );
        } else {
            self.gl.draw_arrays(
                mode,
                0,
                count,
            );
        }
    }
}

impl Drop for DrawPrimitives {
    fn drop(&mut self) {
        self.gl.delete_vertex_array(Some(&self.vao));
    }
}
