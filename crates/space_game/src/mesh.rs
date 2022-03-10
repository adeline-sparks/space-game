use std::borrow::Cow;
use std::collections::HashMap;

use nalgebra::{Vector2, Vector3};
use thiserror::Error;

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
    Vec2(Vec<Vector2<f32>>),
    Vec3(Vec<Vector3<f32>>),
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

#[derive(Error, Debug, PartialEq, Eq)]
pub enum MeshError {
    #[error("Mesh ends with incomplete primitive ({0} indices is not a multiple of {1})")]
    IncompletePrimitive(usize, usize),
    #[error("Too many indices for GPU upload")]
    TooManyIndices(usize),
    #[error("Index {0} out of bounds: {1} > {2}")]
    IndexOutOfBounds(usize, u16, u16),
    #[error("Two or more attributes have different lengths: `{first_name}` ({first_len}) and `{second_name}` ({second_len})")]
    AttributeLengthMismatch {
        first_name: AttributeName,
        first_len: usize,
        second_name: AttributeName,
        second_len: usize,
    },
}

impl Mesh {
    pub fn new(primitive_type: PrimitiveType) -> Self {
        Mesh {
            attributes: HashMap::new(),
            indices: None,
            primitive_type,
        }
    }

    pub fn vert_count(&self) -> Result<usize, MeshError> {
        let (first_name, first_vec) = match self.attributes.iter().next() {
            None => return Ok(0),
            Some(v) => v,
        };

        if let Some((name, vec)) = self
            .attributes
            .iter()
            .find(|(_, vec)| vec.len() != first_vec.len())
        {
            return Err(MeshError::AttributeLengthMismatch {
                first_name: first_name.clone(),
                first_len: first_vec.len(),
                second_name: name.clone(),
                second_len: vec.len(),
            });
        }

        Ok(first_vec.len())
    }

    pub fn index_count(&self) -> Result<usize, MeshError> {
        match &self.indices {
            Some(indices) => Ok(indices.len()),
            None => self.vert_count(),
        }
    }

    pub fn make_wireframe(&mut self) -> Result<(), MeshError> {
        match (self.primitive_type, &mut self.indices) {
            (PrimitiveType::LINES, _) => {
                return Ok(());
            }
            (PrimitiveType::TRIANGLES, Some(indices)) => {
                let chunks = indices.chunks_exact(3);
                if !chunks.remainder().is_empty() {
                    return Err(MeshError::IncompletePrimitive(indices.len(), 3));
                }

                *indices = chunks
                    .flat_map(|c| [c[0], c[1], c[1], c[2], c[2], c[0]])
                    .collect();
            }
            (PrimitiveType::TRIANGLES, None) => {
                let num_verts = self.vert_count()?;
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

impl Mesh {
    pub fn validate(&self) -> Result<(), MeshError> {
        let index_count = self.index_count().expect("Attribute lengths are validated");
        let prim_verts = self.primitive_type.num_verts();
        if (index_count % prim_verts) != 0 {
            return Err(MeshError::IncompletePrimitive(index_count, prim_verts));
        }

        if let Some(indices) = &self.indices {
            let max: u16 = (index_count - 1)
                .try_into()
                .map_err(|_| MeshError::TooManyIndices(index_count))?;
            if let Some((i, val)) = indices
                .iter()
                .cloned()
                .enumerate()
                .find(|&(_, val)| val > max)
            {
                return Err(MeshError::IndexOutOfBounds(i, val, max));
            }
        }

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
