use bytemuck::cast_slice;
use indexmap::IndexMap;
use js_sys::Uint8Array;
use thiserror::Error;
use web_sys::{WebGlBuffer, WebGl2RenderingContext};

use crate::mesh::{AttributeName, Mesh, MeshError, AttributeVec, AttributeLayout, PrimitiveType};

use super::{Context};

pub struct PrimitiveBuffer {
    pub(super) gl: WebGl2RenderingContext,
    pub(super) vert_buffer: WebGlBuffer,
    pub(super) index_buffer: Option<WebGlBuffer>,
    pub(super) index_count: usize,
    pub(super) layout: AttributeLayout,
    pub(super) primitive_type: PrimitiveType,
}

#[derive(Error, Debug)]
pub enum BufferError {
    #[error("Failed to create_buffer")]
    CreateFailed,
    #[error(transparent)]
    MeshError(#[from] MeshError),
}

impl PrimitiveBuffer {
    pub fn build(context: &Context, mesh: &Mesh) -> Result<Self, BufferError> {
        // Compute an AttributeLayout.
        let layout = compute_layout(&mesh.attributes);

        // Copy each attribute into a contiguous buffer.
        let vert_count = mesh.vert_count()?;
        let mut vert_data = vec![0u8; vert_count * layout.stride];
        for (i, (_, attr)) in mesh.attributes.iter().enumerate() {
            let layout_offset = layout.types_offsets[i].1;
            for v in 0..vert_count {
                let pos = v * layout.stride + layout_offset;
                let bytes = attr.get_bytes(v);
                vert_data[pos..][..bytes.len()].copy_from_slice(bytes);
            }
        }

        // Build the vertex buffer.
        let gl = &context.gl;
        let vert_buffer = create_buffer(gl, WebGl2RenderingContext::ARRAY_BUFFER, &vert_data)?;

        // Build the index buffer
        let index_buffer;
        let index_count; 
        
        if let Some(indices) = &mesh.indices {
            index_buffer = Some(create_buffer(gl, WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER, cast_slice(indices.as_slice()))?);
            index_count = indices.len();
        } else {
            index_buffer = None;
            index_count = vert_count;
        }

        Ok(PrimitiveBuffer { 
            gl: gl.clone(), 
            vert_buffer,
            index_buffer,
            index_count,
            layout, 
            primitive_type: mesh.primitive_type,
        })
    }
}

impl Drop for PrimitiveBuffer {
    fn drop(&mut self) {
        self.gl.delete_buffer(Some(&self.vert_buffer));

        if let Some(index_buffer) = &self.index_buffer {
            self.gl.delete_buffer(Some(index_buffer));
        }
    }
}

fn compute_layout(attributes: &IndexMap<AttributeName, AttributeVec>) -> AttributeLayout {
    // Compute the memory order by sorting attributes from largest to smallest.
    let mut order = (0..attributes.len()).collect::<Vec<_>>();
    order.sort_unstable_by_key(|&i| -(attributes[i].type_().byte_count() as isize));

    // Assign offsets in memory order.
    let mut types_offsets = attributes
        .iter()
        .map(|(n, attr) | (n.clone(), (attr.type_(), 0)))
        .collect::<IndexMap<_, _>>();
    let mut len = 0;
    for &i in &order {
        let (attr_type, offset) = &mut types_offsets[i];
        *offset = len;
        len += attr_type.byte_count();
    }

    // Round the vertex length up to an alignment to compute the stride.
    const ALIGN: usize = 16;
    let stride = ((len / ALIGN) + 1) * ALIGN;

    AttributeLayout { types_offsets, stride }
}

fn create_buffer(gl: &WebGl2RenderingContext, target: u32, data: &[u8]) -> Result<WebGlBuffer, BufferError> {
    let buf = gl.create_buffer().ok_or(BufferError::CreateFailed)?;
    gl.bind_buffer(target, Some(&buf));
    gl.buffer_data_with_array_buffer_view(
        target,
        &Uint8Array::from(data),
        WebGl2RenderingContext::STATIC_DRAW,
    );

    Ok(buf)
}