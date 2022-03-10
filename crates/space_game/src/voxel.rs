use crate::mesh::{AttributeVec, Mesh, PrimitiveType, NORMAL, POSITION};
use nalgebra::Vector3;
use once_cell::sync::Lazy;

mod consts;
use consts::{CASE_TABLE, CORNER_OFFSETS, EDGE_CORNERS, NUM_CORNERS, NUM_EDGES};

pub trait SignedDistanceFunction {
    fn value(&self, pos: Vector3<f64>) -> f64;
    fn grad(&self, pos: Vector3<f64>) -> Vector3<f64>;
}

pub fn marching_cubes(
    sdf: &impl SignedDistanceFunction,
    sample_volume: (Vector3<f64>, Vector3<f64>),
    sample_count: Vector3<i32>,
) -> Mesh {
    let cell_size = (sample_volume.1 - sample_volume.0).component_div(&sample_count.cast());
    let ipos_to_pos = |ipos: Vector3<i32>| sample_volume.0 + ipos.cast().component_mul(&cell_size);

    let mut pos_vec = Vec::new();
    let mut index_vec = Vec::<u16>::new();
    for x in 0..sample_count.x {
        for y in 0..sample_count.y {
            for z in 0..sample_count.z {
                let ipos = Vector3::new(x, y, z);
                let mut case = 0;
                for corner in 0..NUM_CORNERS {
                    let corner_pos = ipos_to_pos(ipos + corner_offset(corner));
                    if sdf.value(corner_pos) < 0.0 {
                        case |= 1 << corner;
                    }
                }

                let case = &CASES[case as usize];
                let base = pos_vec.len();
                pos_vec.extend(case.edges.iter().map(|&(d1, d2)| {
                    let pos1 = ipos_to_pos(ipos + d1);
                    let pos2 = ipos_to_pos(ipos + d2);

                    let val1 = sdf.value(pos1);
                    let val2 = sdf.value(pos2);
                    let scale = (val1 / (val1 - val2)).clamp(0.0, 1.0);

                    pos1.lerp(&pos2, scale)
                }));

                for &[i1, i2, i3] in case.tris.iter() {
                    index_vec.push((base + i1) as u16);
                    index_vec.push((base + i2) as u16);
                    index_vec.push((base + i3) as u16);
                }
            }
        }
    }

    let normal_vec: Vec<Vector3<f32>> = pos_vec
        .iter()
        .map(|&pos| sdf.grad(pos).normalize().cast())
        .collect();

    let pos_vec: Vec<Vector3<f32>> = pos_vec.into_iter().map(|v| v.cast()).collect();

    let mut mesh = Mesh::new(PrimitiveType::TRIANGLES);
    mesh.indices = Some(index_vec);
    mesh.attributes.insert(POSITION, AttributeVec::Vec3(pos_vec));
    mesh.attributes.insert(NORMAL, AttributeVec::Vec3(normal_vec));
    assert_eq!(mesh.validate(), Ok(()));
    mesh
}

#[derive(Default)]
struct Case {
    edges: Box<[(Vector3<i32>, Vector3<i32>)]>,
    tris: Box<[[usize; 3]]>,
}

static CASES: Lazy<Box<[Case]>> = Lazy::new(|| {
    (0..256)
        .into_iter()
        .map(|i| Case::from_raw(CASE_TABLE[i]))
        .collect::<Vec<_>>()
        .into_boxed_slice()
});

impl Case {
    fn from_raw(raw_tris: &[[u8; 3]]) -> Self {
        let mut edges = Vec::new();
        let mut edge_map = [None; NUM_EDGES as usize];

        let tris = raw_tris
            .iter()
            .map(|&raw_tri| {
                let mut tri = [0; 3];

                for i in 0..3 {
                    tri[i] = *edge_map[raw_tri[i] as usize].get_or_insert_with(|| {
                        let pos = edges.len();
                        let [c1, c2] = EDGE_CORNERS[raw_tri[i] as usize];
                        edges.push((corner_offset(c1), corner_offset(c2)));
                        pos
                    });
                }

                tri
            })
            .collect::<Vec<_>>();

        Self {
            edges: edges.into_boxed_slice(),
            tris: tris.into_boxed_slice(),
        }
    }
}

fn corner_offset(corner: u8) -> Vector3<i32> {
    let [x, y, z] = CORNER_OFFSETS[corner as usize];
    Vector3::new(x as i32, y as i32, z as i32)
}
