use thiserror::Error;
use web_sys::{WebGl2RenderingContext, WebGlVertexArrayObject, WebGlProgram};

use crate::gl::{webgl_scalar_count, webgl_scalar_type, Context};
use crate::mesh::{PrimitiveType, AttributeName};

use super::Shader;
use super::vbo::Vbo;

pub struct Vao {
    pub(super) gl: WebGl2RenderingContext,
    pub(super) vao: WebGlVertexArrayObject,
    pub(super) program: WebGlProgram,
    pub(super) primitive_type: PrimitiveType,
    pub(super) index_count: usize,
    pub(super) indexed: bool,
}

#[derive(Error, Debug)]
pub enum VaoError {
    #[error("Failed to create_vertex_array")]
    CreateVertexArrayFailed,
    #[error("Failed to create_buffer")]
    CreateBufferFailed,
    #[error("Type error for attribute `{0}` (Found {1:#04X} expected {2:#04X})")]
    AttributeTypeError(AttributeName, u32, u32),
    #[error("Shader expects unknown attribute `{0}`")]
    UnknownAttribute(AttributeName),
}

impl Vao {
    pub fn build(
        context: &Context,
        shader: &Shader,
        vbo: &Vbo,
    ) -> Result<Self, VaoError> {
        let gl = &context.gl;
        let program = &shader.program;
        let vert_buffer = &vbo.vert_buffer;
        let index_buffer = &vbo.index_buffer;
        let vao = gl
            .create_vertex_array()
            .ok_or(VaoError::CreateVertexArrayFailed)?;
        gl.bind_vertex_array(Some(&vao));
        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(vert_buffer));
        if let Some(index_buffer) = index_buffer {
            gl.bind_buffer(WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER, Some(index_buffer));
        }

        let num_attribs = gl.get_program_parameter(program, WebGl2RenderingContext::ACTIVE_ATTRIBUTES)
            .unchecked_into_f64()
            as u32;
        for i in 0..num_attribs {
            let attrib = gl.get_active_attrib(program, i).unwrap();
            let name = AttributeName::from(attrib.name());

            // TODO type check

            let loc = gl.get_attrib_location(program, name.as_ref()).try_into().unwrap();
            let &(attr_type, offset) = vbo.layout.types_offsets
                .get(&name)
                .ok_or(VaoError::UnknownAttribute(name))?;
            
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

        Ok(Vao {
            gl: gl.clone(),
            vao,
            program: program.clone(),
            primitive_type: vbo.primitive_type,
            index_count: vbo.index_count,
            indexed: vbo.index_buffer.is_some(),
        })
    }
}

impl Drop for Vao {
    fn drop(&mut self) {
        self.gl.delete_vertex_array(Some(&self.vao));
    }
}
