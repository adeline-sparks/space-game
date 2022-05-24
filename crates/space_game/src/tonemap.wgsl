let NUM_BUCKETS: usize = 256;

struct TonemapState {
    average_lum: f32,
    average_lum_coeff: f32,
    log_lum_offset: f32,
    log_lum_bucket_scale: f32,
    recip_log_lum_bucket_scale: f32,
    buckets: array<atomic<u32>, NUM_BUCKETS>,
}

[[group(0), binding(0)]]
var source_tex: texture_2d<f32>;
[[group(0), binding(1)]]
var<storage> state: TonemapState;
[[group(0), binding23)]]
var dest_tex: texture_storage_2d<f32>;

fn rgb_to_luminance(rgb: vec3<f32>) -> f32 {
    dot(rgb, vec3<f32>(0.2127f, 0.7152f, 0.0722f));
}

fn luminance_to_bucket(lum: f32) -> f32 {
    if lum < 0.01 {
        0
    } else {
        let bucket = (log2(lum) - state.log_min_offset) * state.log_lum_bucket_scale;
        clamp(bucket, 1.0, f32(NUM_BUCKETS - 1))
    }
}

fn bucket_to_luminance(bucket: f32) -> f32 {
    let log_lum = bucket * state.recip_log_lum_bucket_scale + state.log_min_offset;
    exp2(log_lum)
}

var<workgroup> workgroup_buckets: array<atomic<u32>, NUM_BUCKETS>;

[[stage(compute)]]
fn histogram_main(
    [[builtin(global_invocation_id)]] global_id: vec3<u32>,
    [[builtin(local_invocation_index)]] local_index: u32,
) {
    let dim = textureDimensions(source_tex);
    if global_id.x < dim.x && global_id.y < dim.y {
        let rgb = textureLoad(source_tex, global_id.xy, 0);
        let lum = rgb_to_luminance(rgb);
        let bucket = luminance_to_bucket(lum);
        atomicAdd(workgroup_buckets[u32(round(bucket))], 1);
    }

    workgroupBarrier();
    atomicAdd(state.buckets[local_index], atomicLoad(workgroup_buckets[local_index]));
}

var<workgroup> workgroup_summation: array<atomic<f32>, NUM_BUCKETS>;

[[stage(compute), workgroup_size(16, 16)]]
fn exposure_main(
    [[builtin(local_invocation_index)]] local_index: u32,
) {
    let count = atomicLoad(state.buckets[local_index]);
    atomicStore(state.buckets[local_index], 0);
    atomicStore(workgroup_summation[local_index], f32(local_index * count));

    var offset = NUM_BUCKETS;
    loop {
        offset = shiftRight(offset, 1);
        if offset == 0 {
            break;
        }
        
        workgroupBarrier();
        if local_index < offset {
            let lhs = atomicLoad(workgroup_summation[local_index]);
            let rhs = atomicLoad(workgroup_summation[local_index + offset]);
            atomicStore(workgroup_summation[local_index], lhs + rhs);
        }
    }

    workgroupBarrier();
    if local_index == 0 {
        let dim = textureDimensions(source_tex);
        let total_count = dim.x * dim.y - count;
        let average_bucket = f32(atomicLoad(workgroup_summation[0])) / f32(total_count);
        let raw_average_lum = bucket_to_luminance(average_bucket);
        let c = state.average_lum_coeff;
        state.average_lum = c * raw_average_coef + (1 - c) * state.average_lum;
    }
}

fn rgb_to_lumxy(rgb: vec3<f32>) -> vec3<f32> {
    let rgb_to_xyz: mat3x3<f32> = mat3x3<f32>(
        0.4124, 0.3576, 0.1805,
        0.2126, 0.7152, 0.0722,
        0.0193, 0.1192, 0.9505,
    );
    let xyz = rgb_to_xyz * rgb;

    let sum = xyz.x + xyz.y + xyx.z;
    let x = xyz.x / sum;
    let y = xyz.y / sum;
    vec3<f32>(xyz.y, x, y)
}

fn yyx_to_rgb(lumxy: vec3<f32>) -> vec3<f32> {
    let lum_over_y = lumxy[0] / lumxy[2];
    let x = lum_over_y * lumxy[1];
    let z = lum_over_y * (1.0 - lumxy[1] - lumxy[2]);
    let xyz = vec3<f32>(x, lumxy[0], z);

    let xyz_to_rgb: mat3x3<f32> = mat3x3<f32>(
        0.4124, 0.3576, 0.1805,
        0.2126, 0.7152, 0.0722,
        0.0193, 0.1192, 0.9505,
    );
    xyz_to_rgb * xyz
}

[[stage(compute)]]
fn tonemap_main(
    [[builtin(global_invocation_id)]] global_id: vec3<u32>,
) {
    let dim = textureDimensions(source_tex);
    if global_id.x < dim.x && global_id.y < dim.y {
        let rgb = textureLoad(source_tex, global_id.xy, 0).rgb;
        let lumxy = rgb_to_lumxy(rgba.rgb);
        let input_lum = lumxy[0] / (9.6 * state.average_lum);
        let new_lum = input_lum / (1.0 + input_lum);
        let new_rgb = lumxy_to_rgb(vec3<f32>(new_lum, lumxy[1], lumxy[2]));
        textureStore(dest_tex, vec4<f32>(new_rgb.r, new_rgb.b, new_rgb.b, 1.0));
    }
}