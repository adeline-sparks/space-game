use glam::{Vec3, IVec3};
use log::info;
use once_cell::sync::Lazy;

mod table;

pub trait SignedDistanceFunction {
    fn value(&self, pos: Vec3) -> f32;
    fn grad(&self, pos: Vec3) -> Vec3;
}

pub fn marching_cubes(sdf: &impl SignedDistanceFunction, sample_volume: (Vec3, Vec3), sample_count: IVec3, output_tri: &mut impl FnMut(Vec3, Vec3, Vec3, Vec3, Vec3, Vec3)) {
    let cell_size = (sample_volume.1 - sample_volume.0) / sample_count.as_vec3();
    let ipos_to_pos = |ipos: IVec3| -> Vec3 {
        sample_volume.0 + ipos.as_vec3() * cell_size
    };

    for x in 0..sample_count.x {
        for y in 0..sample_count.y {
            for z in 0..sample_count.z {
                let ipos = IVec3::new(x, y, z);
                let mut case = 0;
                for corner in 0..NUM_CORNERS {
                    let corner_pos = ipos_to_pos(ipos + corner_offset(corner));
                    if sdf.value(corner_pos) > 0.0 {
                        case |= 1 << corner;
                    }
                }
        
                let case = &ALL_CASES[case as usize];
                let verts = case.edges.iter().map(|&(d1, d2)| {
                    let pos1 = ipos_to_pos(ipos + d1);
                    let pos2 = ipos_to_pos(ipos + d2);

                    let val1 = sdf.value(pos1);
                    let val2 = sdf.value(pos2);
                    assert!((val1 > 0.0) ^ (val2 > 0.0));
                    let scale = (val1 / (val1 - val2)).clamp(0.0, 1.0);
                    
                    pos1.lerp(pos2, scale)
                }).collect::<Vec<_>>();
                let normals = verts.iter().map(|&p| {
                    sdf.grad(p).normalize_or_zero()
                }).collect::<Vec<_>>();

                for &[i1, i2, i3] in case.tris.iter() {
                    output_tri(
                        verts[i1], 
                        verts[i2], 
                        verts[i3], 
                        normals[i1],
                        normals[i2],
                        normals[i3],
                    );
                }
            }
        }
    }
}

const NUM_CORNERS: u8 = 8;
const CORNER_OFFSETS: [[u8; 3]; NUM_CORNERS as usize] = [
    [0, 0, 0],
    [1, 0, 0],
    [1, 1, 0],
    [0, 1, 0],
    [0, 0, 1],
    [1, 0, 1],
    [1, 1, 1],
    [0, 1, 1],
];

fn corner_offset(corner: u8) -> IVec3 {
    let [x, y, z] = CORNER_OFFSETS[corner as usize];
    IVec3::new(x as i32, y as i32, z as i32)
}

const NUM_EDGES: u8 = 12;
static EDGE_CORNERS: [[u8; 2]; NUM_EDGES as usize] = [
    [0, 1],
    [1, 2],
    [2, 3],
    [3, 0],
    [4, 5],
    [5, 6],
    [6, 7],
    [7, 4],
    [0, 4],
    [1, 5],
    [2, 6],
    [3, 7],
];

static ALL_CASES: Lazy<Box<[Case]>> = Lazy::new(|| 
    (0..256)
        .into_iter()
        .map(|i| Case::from_raw_tris(table::CASE_TRIS[i]))
        .collect::<Vec<_>>()
        .into_boxed_slice()
);

#[derive(Default)]
struct Case {
    edges: Box<[(IVec3, IVec3)]>,
    tris: Box<[[usize; 3]]>,
}

impl Case {
    fn from_raw_tris(raw_tris: &[[u8; 3]]) -> Self {
        let mut edges = Vec::new();
        let mut tris = Vec::new();
        let mut edge_map = [None; NUM_EDGES as usize];

        for &raw_tri in raw_tris {
            let mut tri = [0; 3];

            for i in 0..3 {
                tri[i] = *edge_map[raw_tri[i] as usize].get_or_insert_with(|| {
                    let pos = edges.len();
                    let [c1, c2] = EDGE_CORNERS[raw_tri[i] as usize];
                    edges.push((corner_offset(c1), corner_offset(c2)));
                    pos
                });
            }

            tris.push(tri);
        }

        Self {
             edges: edges.into_boxed_slice(), 
             tris: tris.into_boxed_slice(),
        }
    }
}
