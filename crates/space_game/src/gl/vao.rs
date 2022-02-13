use js_sys::{Uint16Array, Uint8Array};
use nalgebra::{Vector2, Vector3};
use thiserror::Error;
use web_sys::{WebGl2RenderingContext, WebGlBuffer, WebGlVertexArrayObject};

use crate::gl::{webgl_scalar_count, webgl_scalar_type, Context};
use crate::mesh::{Attribute, AttributeVec, Mesh, MeshError, PrimitiveType};

pub struct Vao {
    gl: WebGl2RenderingContext,
    vao: WebGlVertexArrayObject,
    vert_buffer: WebGlBuffer,
    index_buffer: Option<WebGlBuffer>,
    mode: u32,
    index_count: i32,
}

#[derive(Error, Debug)]
pub enum VaoError {
    #[error("Failed to create_vertex_array")]
    CreateVertexArrayFailed,
    #[error("Failed to create_buffer")]
    CreateBufferFailed,
    #[error(transparent)]
    MeshError(#[from] MeshError),
}

impl Vao {
    pub fn build(
        context: &Context,
        attributes: &[Attribute],
        mesh: &Mesh,
    ) -> Result<Self, VaoError> {
        let vert_count = mesh.vert_count()?;

        let gl = &context.gl;
        let vao = gl
            .create_vertex_array()
            .ok_or(VaoError::CreateVertexArrayFailed)?;
        gl.bind_vertex_array(Some(&vao));

        let active_attributes = attributes
            .iter()
            .enumerate()
            .filter(|&(_, a)| mesh.attributes.contains_key(&a.name))
            .collect::<Vec<_>>();
        let stride: usize = active_attributes
            .iter()
            .map(|&(_, a)| a.type_.byte_count())
            .sum();

        let vert_buffer = gl.create_buffer().ok_or(VaoError::CreateBufferFailed)?;
        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&vert_buffer));

        let mut vert_buffer_data = vec![0u8; stride * vert_count];
        let mut offset = 0;
        for &(i, attr) in &active_attributes {
            gl.enable_vertex_attrib_array(i as u32);
            gl.vertex_attrib_pointer_with_i32(
                i as u32,
                webgl_scalar_count(attr.type_),
                webgl_scalar_type(attr.type_),
                false,
                stride as i32,
                offset as i32,
            );
            pack_attribute_vec(
                stride,
                offset,
                &mesh.attributes[&attr.name],
                &mut vert_buffer_data,
            );
            offset += attr.type_.byte_count();
        }

        gl.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ARRAY_BUFFER,
            &Uint8Array::from(vert_buffer_data.as_slice()),
            WebGl2RenderingContext::STATIC_DRAW,
        );

        let index_buffer = if let Some(indices) = &mesh.indices {
            let buf = gl.create_buffer().ok_or(VaoError::CreateBufferFailed)?;
            gl.bind_buffer(WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER, Some(&buf));
            gl.buffer_data_with_array_buffer_view(
                WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER,
                &Uint16Array::from(indices.as_slice()),
                WebGl2RenderingContext::STATIC_DRAW,
            );
            Some(buf)
        } else {
            None
        };

        let mode = match mesh.primitive_type {
            PrimitiveType::TRIANGLES => WebGl2RenderingContext::TRIANGLES,
            PrimitiveType::LINES => WebGl2RenderingContext::LINES,
        };

        let index_count = mesh.index_count()? as i32;

        Ok(Vao {
            gl: gl.clone(),
            vao,
            vert_buffer,
            index_buffer,
            mode,
            index_count,
        })
    }

    pub fn draw(&self) {
        self.gl.bind_vertex_array(Some(&self.vao));
        if self.index_buffer.is_some() {
            self.gl.draw_elements_with_i32(
                self.mode,
                self.index_count,
                WebGl2RenderingContext::UNSIGNED_SHORT,
                0,
            );
        } else {
            self.gl.draw_arrays(self.mode, 0, self.index_count);
        }
    }
}

fn pack_attribute_vec(stride: usize, offset: usize, attr_vec: &AttributeVec, out: &mut [u8]) {
    match attr_vec {
        AttributeVec::Vec2(vecs) => {
            for (i, vec) in vecs.iter().enumerate() {
                let pos = (i * stride) + offset;
                out[pos..pos + 8].copy_from_slice(bytemuck::cast_ref::<Vector2<f32>, [u8; 8]>(vec));
            }
        }
        AttributeVec::Vec3(vecs) => {
            for (i, vec) in vecs.iter().enumerate() {
                let pos = (i * stride) + offset;
                out[pos..pos + 12].copy_from_slice(bytemuck::cast_ref::<Vector3<f32>, [u8; 12]>(vec));
            }
        }
    }
}
