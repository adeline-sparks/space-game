let NUM_BUCKETS = 256u;

@group(0) @binding(0)
var hdr_tex: texture_2d<f32>;

@group(0) @binding(1)
var<storage, read_write> buckets: array<atomic<u32>, NUM_BUCKETS>;

var<workgroup> workgroup_buckets: array<atomic<u32>, NUM_BUCKETS>;

fn rgb_to_luminance(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.2127, 0.7152, 0.0722));
}

fn luminance_to_bucket(lum: f32) -> u32 {
    let min_lum = 0.0001;
    let log_min_lum = log2(min_lum);
    let max_lum = 1.0;
    let log_max_lum = log2(max_lum);

    if (lum < min_lum) {
        return 0u;
    } 

    let scaled = (log2(lum) - log_min_lum) / (log_max_lum - log_min_lum);
    let bucket = i32(scaled * f32(NUM_BUCKETS - 1u)) + 1;
    return u32(clamp(bucket, 1, i32(NUM_BUCKETS)));
}

@compute @workgroup_size(16, 16)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_index) local_index: u32,
) {
    workgroup_buckets[local_index] = 0u;
    workgroupBarrier();

    let dim = textureDimensions(hdr_tex);
    let pos = vec2<i32>(global_id.xy);
    if (pos.x < dim.x && pos.y < dim.y) {
        let texel = textureLoad(hdr_tex, pos, 0);
        let lum = rgb_to_luminance(texel.rgb);
        let bucket = luminance_to_bucket(lum);
        atomicAdd(&workgroup_buckets[bucket], 1u);
    }

    workgroupBarrier();
    atomicAdd(&buckets[local_index], workgroup_buckets[local_index]);
}