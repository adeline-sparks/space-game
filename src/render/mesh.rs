use js_sys::{Uint16Array, Uint8Array};
use web_sys::{WebGl2RenderingContext, WebGlVertexArrayObject};

use super::{Context, DataType};

pub struct Mesh {
    vao: WebGlVertexArrayObject,
    vert_count: i32,
}

pub struct MeshBuilder<'a> {
    attributes: &'a [Attribute],
    bytes: Vec<u8>,
    indices: Vec<u16>,
    attribute_num: usize,
    vertex_num: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub type_: DataType,
}

impl Mesh {
    pub(super) fn draw(&self, context: &WebGl2RenderingContext) {
        context.bind_vertex_array(Some(&self.vao));
        context.draw_elements_with_i32(
            WebGl2RenderingContext::TRIANGLES,
            self.vert_count,
            WebGl2RenderingContext::UNSIGNED_SHORT,
            0,
        );
    }
}

impl<'a> MeshBuilder<'a> {
    pub fn new(attributes: &'a [Attribute]) -> Self {
        MeshBuilder {
            attributes,
            bytes: Vec::new(),
            indices: Vec::new(),
            attribute_num: 0,
            vertex_num: 0,
        }
    }

    pub fn push<V: AttributeValue>(&mut self, val: V) {
        assert!(self.attributes[self.attribute_num].type_ == V::RENDER_TYPE);
        self.attribute_num += 1;
        val.push(&mut self.bytes);
    }

    pub fn end_vert(&mut self) -> u16 {
        assert!(self.attribute_num == self.attributes.len());
        let result: u16 = self.vertex_num.try_into().unwrap();
        self.vertex_num += 1;
        self.attribute_num = 0;
        self.indices.push(result);
        result
    }

    pub fn dup_vert(&mut self, id: u16) {
        self.indices.push(id);
    }

    pub fn build(&self, context: &Context) -> Result<Mesh, String> {
        let context = &context.0;
        assert!(self.attribute_num == 0);

        let vao = context
            .create_vertex_array()
            .ok_or_else(|| "Failed to create_vertex_array".to_string())?;
        context.bind_vertex_array(Some(&vao));

        let vert_buffer = context
            .create_buffer()
            .ok_or_else(|| "failed to create vertex buffer".to_string())?;
        context.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&vert_buffer));
        context.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ARRAY_BUFFER,
            &Uint8Array::from(self.bytes.as_slice()),
            WebGl2RenderingContext::STATIC_DRAW,
        );

        let stride: usize = self.attributes.iter().map(|a| a.type_.num_bytes()).sum();
        let mut offset = 0;
        for (i, attr) in self.attributes.iter().enumerate() {
            context.enable_vertex_attrib_array(i as u32);
            context.vertex_attrib_pointer_with_i32(
                i as u32,
                attr.type_.num_components() as i32,
                attr.type_.webgl_scalar_type(),
                false,
                stride as i32,
                offset as i32,
            );
            offset += attr.type_.num_bytes();
        }

        let index_buffer = context
            .create_buffer()
            .ok_or_else(|| "failed to create index buffer".to_string())?;
        context.bind_buffer(
            WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER,
            Some(&index_buffer),
        );
        context.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER,
            &Uint16Array::from(self.indices.as_slice()),
            WebGl2RenderingContext::STATIC_DRAW,
        );

        Ok(Mesh {
            vao,
            vert_count: self.indices.len() as i32,
        })
    }
}

pub trait AttributeValue {
    const RENDER_TYPE: DataType;

    fn push(&self, bytes: &mut Vec<u8>);
}

impl AttributeValue for f32 {
    const RENDER_TYPE: DataType = DataType::Float;

    fn push(&self, bytes: &mut Vec<u8>) {
        bytes.extend(self.to_ne_bytes().iter());
    }
}

impl AttributeValue for glam::Vec2 {
    const RENDER_TYPE: DataType = DataType::Vec2;

    fn push(&self, bytes: &mut Vec<u8>) {
        self.x.push(bytes);
        self.y.push(bytes);
    }
}
