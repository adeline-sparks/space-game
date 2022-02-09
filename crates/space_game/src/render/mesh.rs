use js_sys::{Uint16Array, Uint8Array};
use wasm_bindgen::JsValue;
use web_sys::{WebGl2RenderingContext, WebGlVertexArrayObject};

use super::{Context, DataType};

pub struct Mesh {
    vao: WebGlVertexArrayObject,
    mode: u32,
    vert_count: i32,
}

pub struct MeshBuilder<'a> {
    attributes: &'a [Attribute],
    mode: MeshBuilderMode,
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MeshBuilderMode {
    SOLID,
    WIREFRAME,
}

impl Mesh {
    pub(super) fn draw(&self, gl: &WebGl2RenderingContext) {
        gl.bind_vertex_array(Some(&self.vao));
        gl.draw_elements_with_i32(
            self.mode,
            self.vert_count,
            WebGl2RenderingContext::UNSIGNED_SHORT,
            0,
        );
    }
}

impl<'a> MeshBuilder<'a> {
    pub fn new(attributes: &'a [Attribute], mode: MeshBuilderMode) -> Self {
        MeshBuilder {
            attributes,
            mode,
            bytes: Vec::new(),
            indices: Vec::new(),
            attribute_num: 0,
            vertex_num: 0,
        }
    }

    pub fn write_attribute<V: AttributeValue>(&mut self, val: V) {
        assert!(self.attributes[self.attribute_num].type_ == V::RENDER_TYPE);
        self.attribute_num += 1;
        val.push(&mut self.bytes);
    }

    pub fn finish_vert(&mut self) -> u16 {
        assert!(self.attribute_num == self.attributes.len());
        let result: u16 = self.vertex_num.try_into().unwrap();
        self.vertex_num += 1;
        self.attribute_num = 0;
        result
    }

    pub fn write_triangle(&mut self, id1: u16, id2: u16, id3: u16) {
        match self.mode {
            MeshBuilderMode::SOLID => {
                self.indices.extend_from_slice(&[id1, id2, id3]);
            }
            MeshBuilderMode::WIREFRAME => {
                self.indices
                    .extend_from_slice(&[id1, id2, id2, id3, id3, id1]);
            }
        }
    }

    pub fn build(&self, context: &Context) -> Result<Mesh, JsValue> {
        let gl = &context.gl;
        assert!(self.attribute_num == 0);

        let vao = gl
            .create_vertex_array()
            .ok_or_else(|| JsValue::from("Failed to create_vertex_array"))?;
        gl.bind_vertex_array(Some(&vao));

        let vert_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from("failed to create vertex buffer"))?;
        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&vert_buffer));
        gl.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ARRAY_BUFFER,
            &Uint8Array::from(self.bytes.as_slice()),
            WebGl2RenderingContext::STATIC_DRAW,
        );

        let stride: usize = self.attributes.iter().map(|a| a.type_.num_bytes()).sum();
        let mut offset = 0;
        for (i, attr) in self.attributes.iter().enumerate() {
            gl.enable_vertex_attrib_array(i as u32);
            gl.vertex_attrib_pointer_with_i32(
                i as u32,
                attr.type_.num_components() as i32,
                attr.type_.webgl_scalar_type(),
                false,
                stride as i32,
                offset as i32,
            );
            offset += attr.type_.num_bytes();
        }

        let index_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from("failed to create index buffer"))?;
        gl.bind_buffer(
            WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER,
            Some(&index_buffer),
        );
        gl.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER,
            &Uint16Array::from(self.indices.as_slice()),
            WebGl2RenderingContext::STATIC_DRAW,
        );

        let mode = match self.mode {
            MeshBuilderMode::SOLID => WebGl2RenderingContext::TRIANGLES,
            MeshBuilderMode::WIREFRAME => WebGl2RenderingContext::LINES,
        };

        Ok(Mesh {
            vao,
            mode,
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

impl AttributeValue for glam::Vec3 {
    const RENDER_TYPE: DataType = DataType::Vec3;

    fn push(&self, bytes: &mut Vec<u8>) {
        self.x.push(bytes);
        self.y.push(bytes);
        self.z.push(bytes);
    }
}

impl AttributeValue for glam::Vec4 {
    const RENDER_TYPE: DataType = DataType::Vec4;

    fn push(&self, bytes: &mut Vec<u8>) {
        self.x.push(bytes);
        self.y.push(bytes);
        self.z.push(bytes);
        self.w.push(bytes);
    }
}
