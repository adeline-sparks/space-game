use std::borrow::Cow;
use std::collections::HashMap;

use glam::{Vec2, Vec3};

#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub attributes: HashMap<AttributeName, AttributeVec>,
    pub indices: Option<Vec<u16>>,
    pub primitive_type: PrimitiveType,
}

pub type AttributeName = Cow<'static, str>;
pub const POSITION: AttributeName = Cow::Borrowed("vert_pos");
pub const NORMAL: AttributeName = Cow::Borrowed("vert_normal");

#[derive(Debug, Clone, PartialEq)]
pub enum AttributeVec {
    Vec2(Vec<Vec2>),
    Vec3(Vec<Vec3>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: AttributeName,
    pub type_: AttributeType,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AttributeType {
    Vec2,
    Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    LINES,
    TRIANGLES,
}

impl Mesh {
    pub fn new(primitive_type: PrimitiveType) -> Self {
        Mesh {
            attributes: HashMap::new(),
            indices: None,
            primitive_type,
        }
    }

    pub fn vert_count(&self) -> Option<usize> {
        let vert_count = self.attributes.values().next()?.len();
        if self.attributes.values().any(|a| a.len() != vert_count) {
            return None;
        }

        Some(vert_count)
    }

    pub fn index_count(&self) -> Option<usize> {
        match &self.indices {
            Some(vec) => Some(vec.len()),
            None => self.vert_count()
        }
    }

    pub fn validate(&self) -> Result<(), ()> {
        let index_count = self.index_count().ok_or(())?;
        if (index_count % self.primitive_type.num_verts()) != 0 {
            return Err(());
        }

        if let Some(indices) = &self.indices {
            let max: u16 = (index_count - 1).try_into().map_err(|_| ())?;
            if indices.iter().any(|&i| i > max) {
                return Err(());
            }
        }

        return Ok(());
    }

    pub fn make_wireframe(&mut self) -> Result<(), ()> {
        match (self.primitive_type, &mut self.indices) {
            (PrimitiveType::LINES, _) => {
                return Ok(());
            }
            (PrimitiveType::TRIANGLES, Some(indices)) => {
                let chunks = indices.chunks_exact(3);
                if !chunks.remainder().is_empty() {
                    return Err(());
                }

                *indices = chunks
                    .map(|c| [c[0], c[1], c[1], c[2], c[2], c[0]])
                    .flatten()
                    .collect();
            }
            (PrimitiveType::TRIANGLES, None) => {
                let num_verts = self.vert_count().ok_or(())?;
                let mut indices = Vec::with_capacity(num_verts * 2);
                for i in 0..(num_verts / 3) {
                    let v0 = (3 * i) as u16;
                    let v1 = v0 + 1;
                    let v2 = v0 + 2;
                    indices.extend_from_slice(&[v0, v1, v1, v2, v2, v0]);
                }

                self.indices = Some(indices);
            }
        }

        self.primitive_type = PrimitiveType::LINES;
        Ok(())
    }
}

impl AttributeType {
    pub fn byte_count(&self) -> usize {
        match self {
            AttributeType::Vec2 => 2 * 4,
            AttributeType::Vec3 => 3 * 4,
        }
    }
}

impl AttributeVec {
    pub fn len(&self) -> usize {
        match self {
            AttributeVec::Vec2(v) => v.len(),
            AttributeVec::Vec3(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            AttributeVec::Vec2(v) => v.is_empty(),
            AttributeVec::Vec3(v) => v.is_empty(),
        }
    }

    pub fn type_(&self) -> AttributeType {
        match self {
            AttributeVec::Vec2(_) => AttributeType::Vec2,
            AttributeVec::Vec3(_) => AttributeType::Vec3,
        }
    }
}

impl PrimitiveType {
    pub fn num_verts(self) -> usize {
        match self {
            PrimitiveType::LINES => 2,
            PrimitiveType::TRIANGLES => 3,
        }
    }
}
