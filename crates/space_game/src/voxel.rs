use glam::{Vec3, IVec3};
use log::info;

pub trait SignedDistanceFunction {
    fn value(&self, pos: Vec3) -> f32;
}

pub fn marching_cubes(sdf: &impl SignedDistanceFunction, sample_volume: (Vec3, Vec3), sample_count: IVec3, output_tri: &mut impl FnMut(Vec3, Vec3, Vec3)) {
    let cell_size = (sample_volume.1 - sample_volume.0) / sample_count.as_vec3();
    info!("cell_size: {cell_size}");
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
        
                let class = CASE_TO_CLASS[case] as usize;
                if class == 0 {
                    continue;
                }

                info!("ipos {ipos} case {case:#02X}");
        
                let vert_poses: Vec<_> = CASE_TO_VERT_CUBE_CORNERS[case].iter().map(|&[corner1, corner2]| {
                    let (corner1, corner2) = (corner1 as usize, corner2 as usize);
        
                    let pos1 = ipos_to_pos(ipos + corner_offset(corner1));
                    let pos2 = ipos_to_pos(ipos + corner_offset(corner2));
                    let val1 = sdf.value(pos1);
                    let val2 = sdf.value(pos2);
                    assert!((val1 > 0.0) ^ (val2 > 0.0));
        
                    let scale = (val1 / (val1 - val2)).clamp(0.0, 1.0);
                    let pos = pos1.lerp(pos2, scale);
                    let val = sdf.value(pos);
                    info!("edge {pos1} ({val1}) -> {pos2} ({val2}) generated {pos} ({val})");

                    pos
                }).collect();
        
                for &[v1, v2, v3] in CLASS_TO_TRIS[class] {
                    let (v1, v2, v3) = (v1 as usize, v2 as usize, v3 as usize);
                    output_tri(vert_poses[v1], vert_poses[v2], vert_poses[v3]);
                }
            }
        }
    }
}

const NUM_CORNERS: usize = 8;

const CORNER_OFFSETS: [[u8; 3]; NUM_CORNERS] = [
    [0, 0, 0],
    [1, 0, 0],
    [1, 1, 0],
    [0, 1, 0],
    [0, 0, 1],
    [1, 0, 1],
    [1, 1, 1],
    [0, 1, 1],
];

fn corner_offset(corner: usize) -> IVec3 {
    let offsets = CORNER_OFFSETS[corner];
    IVec3::new(offsets[0] as i32, offsets[1] as i32, offsets[2] as i32)
}

static CASE_TO_CLASS: [u8; 256] = [
	0x00, 0x01, 0x01, 0x03, 0x01, 0x03, 0x02, 0x04, 0x01, 0x02, 0x03, 0x04, 0x03, 0x04, 0x04, 0x03,
	0x01, 0x03, 0x02, 0x04, 0x02, 0x04, 0x06, 0x0C, 0x02, 0x05, 0x05, 0x0B, 0x05, 0x0A, 0x07, 0x04,
	0x01, 0x02, 0x03, 0x04, 0x02, 0x05, 0x05, 0x0A, 0x02, 0x06, 0x04, 0x0C, 0x05, 0x07, 0x0B, 0x04,
	0x03, 0x04, 0x04, 0x03, 0x05, 0x0B, 0x07, 0x04, 0x05, 0x07, 0x0A, 0x04, 0x08, 0x0E, 0x0E, 0x03,
	0x01, 0x02, 0x02, 0x05, 0x03, 0x04, 0x05, 0x0B, 0x02, 0x06, 0x05, 0x07, 0x04, 0x0C, 0x0A, 0x04,
	0x03, 0x04, 0x05, 0x0A, 0x04, 0x03, 0x07, 0x04, 0x05, 0x07, 0x08, 0x0E, 0x0B, 0x04, 0x0E, 0x03,
	0x02, 0x06, 0x05, 0x07, 0x05, 0x07, 0x08, 0x0E, 0x06, 0x09, 0x07, 0x0F, 0x07, 0x0F, 0x0E, 0x0D,
	0x04, 0x0C, 0x0B, 0x04, 0x0A, 0x04, 0x0E, 0x03, 0x07, 0x0F, 0x0E, 0x0D, 0x0E, 0x0D, 0x02, 0x01,
	0x01, 0x02, 0x02, 0x05, 0x02, 0x05, 0x06, 0x07, 0x03, 0x05, 0x04, 0x0A, 0x04, 0x0B, 0x0C, 0x04,
	0x02, 0x05, 0x06, 0x07, 0x06, 0x07, 0x09, 0x0F, 0x05, 0x08, 0x07, 0x0E, 0x07, 0x0E, 0x0F, 0x0D,
	0x03, 0x05, 0x04, 0x0B, 0x05, 0x08, 0x07, 0x0E, 0x04, 0x07, 0x03, 0x04, 0x0A, 0x0E, 0x04, 0x03,
	0x04, 0x0A, 0x0C, 0x04, 0x07, 0x0E, 0x0F, 0x0D, 0x0B, 0x0E, 0x04, 0x03, 0x0E, 0x02, 0x0D, 0x01,
	0x03, 0x05, 0x05, 0x08, 0x04, 0x0A, 0x07, 0x0E, 0x04, 0x07, 0x0B, 0x0E, 0x03, 0x04, 0x04, 0x03,
	0x04, 0x0B, 0x07, 0x0E, 0x0C, 0x04, 0x0F, 0x0D, 0x0A, 0x0E, 0x0E, 0x02, 0x04, 0x03, 0x0D, 0x01,
	0x04, 0x07, 0x0A, 0x0E, 0x0B, 0x0E, 0x0E, 0x02, 0x0C, 0x0F, 0x04, 0x0D, 0x04, 0x0D, 0x03, 0x01,
	0x03, 0x04, 0x04, 0x03, 0x04, 0x03, 0x0D, 0x01, 0x04, 0x0D, 0x03, 0x01, 0x03, 0x01, 0x01, 0x00
];

static CLASS_TO_TRIS: [&[[u8; 3]]; 16] = [
    &[],
    &[[0,1,2]],
    &[[0,1,2], [3,4,5]],
    &[[0,1,2], [0,2,3]],
    &[[0,1,4], [1,3,4], [1,2,3]],
    &[[0,1,2], [0,2,3], [4,5,6]],
    &[[0,1,2], [3,4,5], [6,7,8]],
    &[[0,1,4], [1,3,4], [1,2,3], [5,6,7]],
    &[[0,1,2], [0,2,3], [4,5,6], [4,6,7]],
    &[[0,1,2], [3,4,5], [6,7,8], [9,10,11]],
    &[[0,4,5], [0,1,4], [1,3,4], [1,2,3]],
    &[[0,5,4], [0,4,1], [1,4,3], [1,3,2]],
    &[[0,4,5], [0,3,4], [0,1,3], [1,2,3]],
    &[[0,1,2], [0,2,3], [0,3,4], [0,4,5]],
    &[[0,1,2], [0,2,3], [0,3,4], [0,4,5], [0,5,6]],
    &[[0,4,5], [0,3,4], [0,1,3], [1,2,3], [6,7,8]],
];

static CASE_TO_VERT_CUBE_CORNERS: [&[[u8; 2]]; 256] = [
    &[],
    &[[0,1],[0,2],[0,4]],
    &[[0,1],[1,5],[1,3]],
    &[[0,2],[0,4],[1,5],[1,3]],
    &[[0,2],[2,3],[2,6]],
    &[[0,4],[0,1],[2,3],[2,6]],
    &[[0,1],[1,5],[1,3],[0,2],[2,3],[2,6]],
    &[[2,3],[2,6],[0,4],[1,5],[1,3]],
    &[[1,3],[3,7],[2,3]],
    &[[0,1],[0,2],[0,4],[2,3],[1,3],[3,7]],
    &[[0,1],[1,5],[3,7],[2,3]],
    &[[0,2],[0,4],[1,5],[3,7],[2,3]],
    &[[0,2],[1,3],[3,7],[2,6]],
    &[[1,3],[3,7],[2,6],[0,4],[0,1]],
    &[[0,1],[1,5],[3,7],[2,6],[0,2]],
    &[[0,4],[1,5],[3,7],[2,6]],
    &[[0,4],[4,6],[4,5]],
    &[[0,1],[0,2],[4,6],[4,5]],
    &[[0,1],[1,5],[1,3],[0,4],[4,6],[4,5]],
    &[[1,5],[1,3],[0,2],[4,6],[4,5]],
    &[[0,2],[2,3],[2,6],[0,4],[4,6],[4,5]],
    &[[4,6],[4,5],[0,1],[2,3],[2,6]],
    &[[0,4],[4,6],[4,5],[0,1],[1,5],[1,3],[0,2],[2,3],[2,6]],
    &[[2,3],[2,6],[4,6],[4,5],[1,5],[1,3]],
    &[[2,3],[1,3],[3,7],[0,4],[4,6],[4,5]],
    &[[0,1],[0,2],[4,6],[4,5],[2,3],[1,3],[3,7]],
    &[[2,3],[0,1],[1,5],[3,7],[0,4],[4,6],[4,5]],
    &[[2,3],[3,7],[1,5],[4,5],[4,6],[0,2]],
    &[[0,2],[1,3],[3,7],[2,6],[0,4],[4,6],[4,5]],
    &[[1,3],[3,7],[2,6],[4,6],[4,5],[0,1]],
    &[[0,1],[1,5],[3,7],[2,6],[0,2],[0,4],[4,6],[4,5]],
    &[[4,5],[1,5],[3,7],[2,6],[4,6]],
    &[[1,5],[4,5],[5,7]],
    &[[0,1],[0,2],[0,4],[1,5],[4,5],[5,7]],
    &[[1,3],[0,1],[4,5],[5,7]],
    &[[4,5],[5,7],[1,3],[0,2],[0,4]],
    &[[0,2],[2,3],[2,6],[1,5],[4,5],[5,7]],
    &[[0,1],[2,3],[2,6],[0,4],[1,5],[4,5],[5,7]],
    &[[0,1],[4,5],[5,7],[1,3],[0,2],[2,3],[2,6]],
    &[[2,3],[2,6],[0,4],[4,5],[5,7],[1,3]],
    &[[2,3],[1,3],[3,7],[1,5],[4,5],[5,7]],
    &[[0,1],[0,2],[0,4],[2,3],[1,3],[3,7],[1,5],[4,5],[5,7]],
    &[[3,7],[2,3],[0,1],[4,5],[5,7]],
    &[[0,2],[0,4],[4,5],[5,7],[3,7],[2,3]],
    &[[0,2],[1,3],[3,7],[2,6],[1,5],[4,5],[5,7]],
    &[[1,3],[3,7],[2,6],[0,4],[0,1],[1,5],[4,5],[5,7]],
    &[[0,2],[2,6],[3,7],[5,7],[4,5],[0,1]],
    &[[5,7],[3,7],[2,6],[0,4],[4,5]],
    &[[1,5],[0,4],[4,6],[5,7]],
    &[[0,1],[0,2],[4,6],[5,7],[1,5]],
    &[[0,4],[4,6],[5,7],[1,3],[0,1]],
    &[[1,3],[0,2],[4,6],[5,7]],
    &[[1,5],[0,4],[4,6],[5,7],[0,2],[2,3],[2,6]],
    &[[2,6],[2,3],[0,1],[1,5],[5,7],[4,6]],
    &[[0,4],[4,6],[5,7],[1,3],[0,1],[0,2],[2,3],[2,6]],
    &[[2,6],[4,6],[5,7],[1,3],[2,3]],
    &[[1,5],[0,4],[4,6],[5,7],[2,3],[1,3],[3,7]],
    &[[0,1],[0,2],[4,6],[5,7],[1,5],[2,3],[1,3],[3,7]],
    &[[0,4],[4,6],[5,7],[3,7],[2,3],[0,1]],
    &[[2,3],[0,2],[4,6],[5,7],[3,7]],
    &[[1,5],[0,4],[4,6],[5,7],[0,2],[1,3],[3,7],[2,6]],
    &[[0,1],[1,3],[3,7],[2,6],[4,6],[5,7],[1,5]],
    &[[0,1],[0,4],[4,6],[5,7],[3,7],[2,6],[0,2]],
    &[[2,6],[4,6],[5,7],[3,7]],
    &[[2,6],[6,7],[4,6]],
    &[[0,1],[0,2],[0,4],[2,6],[6,7],[4,6]],
    &[[0,1],[1,5],[1,3],[2,6],[6,7],[4,6]],
    &[[0,2],[0,4],[1,5],[1,3],[2,6],[6,7],[4,6]],
    &[[0,2],[2,3],[6,7],[4,6]],
    &[[0,4],[0,1],[2,3],[6,7],[4,6]],
    &[[0,2],[2,3],[6,7],[4,6],[0,1],[1,5],[1,3]],
    &[[4,6],[6,7],[2,3],[1,3],[1,5],[0,4]],
    &[[1,3],[3,7],[2,3],[2,6],[6,7],[4,6]],
    &[[0,1],[0,2],[0,4],[2,3],[1,3],[3,7],[2,6],[6,7],[4,6]],
    &[[0,1],[1,5],[3,7],[2,3],[2,6],[6,7],[4,6]],
    &[[0,2],[0,4],[1,5],[3,7],[2,3],[2,6],[6,7],[4,6]],
    &[[6,7],[4,6],[0,2],[1,3],[3,7]],
    &[[0,1],[1,3],[3,7],[6,7],[4,6],[0,4]],
    &[[0,1],[1,5],[3,7],[6,7],[4,6],[0,2]],
    &[[4,6],[0,4],[1,5],[3,7],[6,7]],
    &[[0,4],[2,6],[6,7],[4,5]],
    &[[2,6],[6,7],[4,5],[0,1],[0,2]],
    &[[0,4],[2,6],[6,7],[4,5],[0,1],[1,5],[1,3]],
    &[[2,6],[6,7],[4,5],[1,5],[1,3],[0,2]],
    &[[0,2],[2,3],[6,7],[4,5],[0,4]],
    &[[0,1],[2,3],[6,7],[4,5]],
    &[[0,2],[2,3],[6,7],[4,5],[0,4],[0,1],[1,5],[1,3]],
    &[[1,3],[2,3],[6,7],[4,5],[1,5]],
    &[[0,4],[2,6],[6,7],[4,5],[2,3],[1,3],[3,7]],
    &[[2,6],[6,7],[4,5],[0,1],[0,2],[2,3],[1,3],[3,7]],
    &[[0,4],[2,6],[6,7],[4,5],[2,3],[0,1],[1,5],[3,7]],
    &[[0,2],[2,6],[6,7],[4,5],[1,5],[3,7],[2,3]],
    &[[0,4],[4,5],[6,7],[3,7],[1,3],[0,2]],
    &[[3,7],[6,7],[4,5],[0,1],[1,3]],
    &[[0,2],[0,1],[1,5],[3,7],[6,7],[4,5],[0,4]],
    &[[1,5],[3,7],[6,7],[4,5]],
    &[[1,5],[4,5],[5,7],[2,6],[6,7],[4,6]],
    &[[0,1],[0,2],[0,4],[1,5],[4,5],[5,7],[2,6],[6,7],[4,6]],
    &[[0,1],[4,5],[5,7],[1,3],[2,6],[6,7],[4,6]],
    &[[4,5],[5,7],[1,3],[0,2],[0,4],[2,6],[6,7],[4,6]],
    &[[2,3],[6,7],[4,6],[0,2],[1,5],[4,5],[5,7]],
    &[[0,4],[0,1],[2,3],[6,7],[4,6],[1,5],[4,5],[5,7]],
    &[[2,3],[6,7],[4,6],[0,2],[0,1],[4,5],[5,7],[1,3]],
    &[[0,4],[4,5],[5,7],[1,3],[2,3],[6,7],[4,6]],
    &[[2,3],[1,3],[3,7],[1,5],[4,5],[5,7],[2,6],[6,7],[4,6]],
    &[[0,1],[0,2],[0,4],[2,3],[1,3],[3,7],[1,5],[4,5],[5,7],[2,6],[6,7],[4,6]],
    &[[3,7],[2,3],[0,1],[4,5],[5,7],[2,6],[6,7],[4,6]],
    &[[2,3],[0,2],[0,4],[4,5],[5,7],[3,7],[2,6],[6,7],[4,6]],
    &[[6,7],[4,6],[0,2],[1,3],[3,7],[1,5],[4,5],[5,7]],
    &[[0,1],[1,3],[3,7],[6,7],[4,6],[0,4],[1,5],[4,5],[5,7]],
    &[[3,7],[6,7],[4,6],[0,2],[0,1],[4,5],[5,7]],
    &[[0,4],[4,5],[5,7],[3,7],[6,7],[4,6]],
    &[[5,7],[1,5],[0,4],[2,6],[6,7]],
    &[[6,7],[5,7],[1,5],[0,1],[0,2],[2,6]],
    &[[6,7],[2,6],[0,4],[0,1],[1,3],[5,7]],
    &[[6,7],[5,7],[1,3],[0,2],[2,6]],
    &[[0,2],[2,3],[6,7],[5,7],[1,5],[0,4]],
    &[[1,5],[0,1],[2,3],[6,7],[5,7]],
    &[[0,4],[0,2],[2,3],[6,7],[5,7],[1,3],[0,1]],
    &[[1,3],[2,3],[6,7],[5,7]],
    &[[5,7],[1,5],[0,4],[2,6],[6,7],[2,3],[1,3],[3,7]],
    &[[5,7],[1,5],[0,1],[0,2],[2,6],[6,7],[2,3],[1,3],[3,7]],
    &[[5,7],[3,7],[2,3],[0,1],[0,4],[2,6],[6,7]],
    &[[0,2],[2,6],[6,7],[5,7],[3,7],[2,3]],
    &[[6,7],[5,7],[1,5],[0,4],[0,2],[1,3],[3,7]],
    &[[0,1],[1,3],[3,7],[6,7],[5,7],[1,5]],
    &[[0,1],[0,4],[0,2],[3,7],[6,7],[5,7]],
    &[[3,7],[6,7],[5,7]],
    &[[3,7],[5,7],[6,7]],
    &[[0,1],[0,2],[0,4],[3,7],[5,7],[6,7]],
    &[[0,1],[1,5],[1,3],[3,7],[5,7],[6,7]],
    &[[0,2],[0,4],[1,5],[1,3],[3,7],[5,7],[6,7]],
    &[[0,2],[2,3],[2,6],[3,7],[5,7],[6,7]],
    &[[0,1],[2,3],[2,6],[0,4],[3,7],[5,7],[6,7]],
    &[[0,1],[1,5],[1,3],[0,2],[2,3],[2,6],[3,7],[5,7],[6,7]],
    &[[2,3],[2,6],[0,4],[1,5],[1,3],[3,7],[5,7],[6,7]],
    &[[1,3],[5,7],[6,7],[2,3]],
    &[[2,3],[1,3],[5,7],[6,7],[0,1],[0,2],[0,4]],
    &[[5,7],[6,7],[2,3],[0,1],[1,5]],
    &[[0,4],[1,5],[5,7],[6,7],[2,3],[0,2]],
    &[[2,6],[0,2],[1,3],[5,7],[6,7]],
    &[[5,7],[1,3],[0,1],[0,4],[2,6],[6,7]],
    &[[2,6],[0,2],[0,1],[1,5],[5,7],[6,7]],
    &[[6,7],[2,6],[0,4],[1,5],[5,7]],
    &[[0,4],[4,6],[4,5],[3,7],[5,7],[6,7]],
    &[[0,1],[0,2],[4,6],[4,5],[3,7],[5,7],[6,7]],
    &[[0,1],[1,5],[1,3],[0,4],[4,6],[4,5],[3,7],[5,7],[6,7]],
    &[[1,5],[1,3],[0,2],[4,6],[4,5],[3,7],[5,7],[6,7]],
    &[[0,2],[2,3],[2,6],[0,4],[4,6],[4,5],[3,7],[5,7],[6,7]],
    &[[4,6],[4,5],[0,1],[2,3],[2,6],[3,7],[5,7],[6,7]],
    &[[0,1],[1,5],[1,3],[0,2],[2,3],[2,6],[0,4],[4,6],[4,5],[3,7],[5,7],[6,7]],
    &[[1,3],[2,3],[2,6],[4,6],[4,5],[1,5],[3,7],[5,7],[6,7]],
    &[[2,3],[1,3],[5,7],[6,7],[0,4],[4,6],[4,5]],
    &[[0,1],[0,2],[4,6],[4,5],[2,3],[1,3],[5,7],[6,7]],
    &[[5,7],[6,7],[2,3],[0,1],[1,5],[0,4],[4,6],[4,5]],
    &[[1,5],[5,7],[6,7],[2,3],[0,2],[4,6],[4,5]],
    &[[2,6],[0,2],[1,3],[5,7],[6,7],[0,4],[4,6],[4,5]],
    &[[2,6],[4,6],[4,5],[0,1],[1,3],[5,7],[6,7]],
    &[[0,2],[0,1],[1,5],[5,7],[6,7],[2,6],[0,4],[4,6],[4,5]],
    &[[2,6],[4,6],[4,5],[1,5],[5,7],[6,7]],
    &[[1,5],[4,5],[6,7],[3,7]],
    &[[1,5],[4,5],[6,7],[3,7],[0,1],[0,2],[0,4]],
    &[[1,3],[0,1],[4,5],[6,7],[3,7]],
    &[[0,2],[1,3],[3,7],[6,7],[4,5],[0,4]],
    &[[1,5],[4,5],[6,7],[3,7],[0,2],[2,3],[2,6]],
    &[[0,1],[2,3],[2,6],[0,4],[3,7],[1,5],[4,5],[6,7]],
    &[[1,3],[0,1],[4,5],[6,7],[3,7],[0,2],[2,3],[2,6]],
    &[[1,3],[2,3],[2,6],[0,4],[4,5],[6,7],[3,7]],
    &[[1,5],[4,5],[6,7],[2,3],[1,3]],
    &[[1,5],[4,5],[6,7],[2,3],[1,3],[0,1],[0,2],[0,4]],
    &[[0,1],[4,5],[6,7],[2,3]],
    &[[0,4],[4,5],[6,7],[2,3],[0,2]],
    &[[0,2],[1,3],[1,5],[4,5],[6,7],[2,6]],
    &[[1,3],[1,5],[4,5],[6,7],[2,6],[0,4],[0,1]],
    &[[0,2],[0,1],[4,5],[6,7],[2,6]],
    &[[0,4],[4,5],[6,7],[2,6]],
    &[[6,7],[3,7],[1,5],[0,4],[4,6]],
    &[[0,2],[4,6],[6,7],[3,7],[1,5],[0,1]],
    &[[0,4],[4,6],[6,7],[3,7],[1,3],[0,1]],
    &[[3,7],[1,3],[0,2],[4,6],[6,7]],
    &[[6,7],[3,7],[1,5],[0,4],[4,6],[0,2],[2,3],[2,6]],
    &[[4,6],[6,7],[3,7],[1,5],[0,1],[2,3],[2,6]],
    &[[6,7],[3,7],[1,3],[0,1],[0,4],[4,6],[0,2],[2,3],[2,6]],
    &[[1,3],[2,3],[2,6],[4,6],[6,7],[3,7]],
    &[[0,4],[1,5],[1,3],[2,3],[6,7],[4,6]],
    &[[1,5],[0,1],[0,2],[4,6],[6,7],[2,3],[1,3]],
    &[[4,6],[6,7],[2,3],[0,1],[0,4]],
    &[[0,2],[4,6],[6,7],[2,3]],
    &[[6,7],[2,6],[0,2],[1,3],[1,5],[0,4],[4,6]],
    &[[0,1],[1,3],[1,5],[2,6],[4,6],[6,7]],
    &[[0,1],[0,4],[4,6],[6,7],[2,6],[0,2]],
    &[[2,6],[4,6],[6,7]],
    &[[2,6],[3,7],[5,7],[4,6]],
    &[[3,7],[5,7],[4,6],[2,6],[0,1],[0,2],[0,4]],
    &[[3,7],[5,7],[4,6],[2,6],[0,1],[1,5],[1,3]],
    &[[1,3],[0,2],[0,4],[1,5],[2,6],[3,7],[5,7],[4,6]],
    &[[3,7],[5,7],[4,6],[0,2],[2,3]],
    &[[0,1],[2,3],[3,7],[5,7],[4,6],[0,4]],
    &[[3,7],[5,7],[4,6],[0,2],[2,3],[0,1],[1,5],[1,3]],
    &[[2,3],[3,7],[5,7],[4,6],[0,4],[1,5],[1,3]],
    &[[2,3],[1,3],[5,7],[4,6],[2,6]],
    &[[2,3],[1,3],[5,7],[4,6],[2,6],[0,1],[0,2],[0,4]],
    &[[4,6],[5,7],[1,5],[0,1],[2,3],[2,6]],
    &[[2,3],[0,2],[0,4],[1,5],[5,7],[4,6],[2,6]],
    &[[1,3],[5,7],[4,6],[0,2]],
    &[[0,1],[1,3],[5,7],[4,6],[0,4]],
    &[[1,5],[5,7],[4,6],[0,2],[0,1]],
    &[[1,5],[5,7],[4,6],[0,4]],
    &[[4,5],[0,4],[2,6],[3,7],[5,7]],
    &[[0,1],[4,5],[5,7],[3,7],[2,6],[0,2]],
    &[[4,5],[0,4],[2,6],[3,7],[5,7],[0,1],[1,5],[1,3]],
    &[[4,5],[1,5],[1,3],[0,2],[2,6],[3,7],[5,7]],
    &[[2,3],[3,7],[5,7],[4,5],[0,4],[0,2]],
    &[[5,7],[4,5],[0,1],[2,3],[3,7]],
    &[[4,5],[0,4],[0,2],[2,3],[3,7],[5,7],[1,3],[0,1],[1,5]],
    &[[2,3],[3,7],[5,7],[4,5],[1,5],[1,3]],
    &[[1,3],[5,7],[4,5],[0,4],[2,6],[2,3]],
    &[[2,6],[2,3],[1,3],[5,7],[4,5],[0,1],[0,2]],
    &[[5,7],[4,5],[0,4],[2,6],[2,3],[0,1],[1,5]],
    &[[0,2],[2,6],[2,3],[1,5],[5,7],[4,5]],
    &[[0,4],[0,2],[1,3],[5,7],[4,5]],
    &[[1,3],[5,7],[4,5],[0,1]],
    &[[0,2],[0,1],[1,5],[5,7],[4,5],[0,4]],
    &[[1,5],[5,7],[4,5]],
    &[[4,6],[2,6],[3,7],[1,5],[4,5]],
    &[[4,6],[2,6],[3,7],[1,5],[4,5],[0,1],[0,2],[0,4]],
    &[[0,1],[4,5],[4,6],[2,6],[3,7],[1,3]],
    &[[4,5],[4,6],[2,6],[3,7],[1,3],[0,2],[0,4]],
    &[[0,2],[4,6],[4,5],[1,5],[3,7],[2,3]],
    &[[4,6],[0,4],[0,1],[2,3],[3,7],[1,5],[4,5]],
    &[[3,7],[1,3],[0,1],[4,5],[4,6],[0,2],[2,3]],
    &[[2,3],[3,7],[1,3],[0,4],[4,5],[4,6]],
    &[[1,3],[1,5],[4,5],[4,6],[2,6],[2,3]],
    &[[4,6],[2,6],[2,3],[1,3],[1,5],[4,5],[0,1],[0,2],[0,4]],
    &[[2,6],[2,3],[0,1],[4,5],[4,6]],
    &[[2,3],[0,2],[0,4],[4,5],[4,6],[2,6]],
    &[[4,5],[4,6],[0,2],[1,3],[1,5]],
    &[[1,3],[1,5],[4,5],[4,6],[0,4],[0,1]],
    &[[0,1],[4,5],[4,6],[0,2]],
    &[[0,4],[4,5],[4,6]],
    &[[0,4],[2,6],[3,7],[1,5]],
    &[[0,2],[2,6],[3,7],[1,5],[0,1]],
    &[[0,1],[0,4],[2,6],[3,7],[1,3]],
    &[[0,2],[2,6],[3,7],[1,3]],
    &[[2,3],[3,7],[1,5],[0,4],[0,2]],
    &[[0,1],[2,3],[3,7],[1,5]],
    &[[0,4],[0,2],[2,3],[3,7],[1,3],[0,1]],
    &[[1,3],[2,3],[3,7]],
    &[[1,3],[1,5],[0,4],[2,6],[2,3]],
    &[[2,6],[2,3],[1,3],[1,5],[0,1],[0,2]],
    &[[0,4],[2,6],[2,3],[0,1]],
    &[[0,2],[2,6],[2,3]],
    &[[0,2],[1,3],[1,5],[0,4]],
    &[[0,1],[1,3],[1,5]],
    &[[0,1],[0,4],[0,2]],
    &[],
];