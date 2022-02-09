use glam::{Vec3, IVec3};
use log::info;

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
        
                if case == 0 || case == 0xff { continue; }

                for &tri in table::CASE_TRIS[case as usize] {
                    let mut verts = [Vec3::ZERO; 3];
                    for i in 0..3 {
                        let [corner1, corner2] = EDGE_CORNERS[tri[i] as usize];

                        let pos1 = ipos_to_pos(ipos + corner_offset(corner1));
                        let pos2 = ipos_to_pos(ipos + corner_offset(corner2));
                        let val1 = sdf.value(pos1);
                        let val2 = sdf.value(pos2);
                        assert!((val1 > 0.0) ^ (val2 > 0.0));
            
                        let scale = (val1 / (val1 - val2)).clamp(0.0, 1.0);
                        let pos = pos1.lerp(pos2, scale);
                        let val = sdf.value(pos);
                        info!("edge {pos1} ({val1}) -> {pos2} ({val2}) generated {pos} ({val})");

                        verts[i] = pos;
                    }

                    output_tri(
                        verts[0], 
                        verts[1], 
                        verts[2], 
                        sdf.grad(verts[0]).normalize(), 
                        sdf.grad(verts[1]).normalize(), 
                        sdf.grad(verts[2]).normalize(),
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
