@group(0) @binding(0)
var hdr_tex: texture_2d<f32>;

@group(0) @binding(1)
var<storage, read_write> histogram_buffer: array<atomic<u32>, 256>;

var<workgroup> workgroup_buckets: array<atomic<u32>, 256>;

fn rgb_to_luminance(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.2127, 0.7152, 0.0722));
}

fn luminance_to_bucket(lum: f32) -> u32 {
    let min_lum = 0.01;
    let log_min_lum = log2(min_lum);
    let max_lum = 50.0;
    let log_max_lum = log2(max_lum);

    if (lum < min_lum) {
        return 0u;
    } 

    let bucket = (log2(lum) - log_min_lum) / log_max_lum * 255.0;
    return u32(floor(clamp(bucket + 1.0, 1.0, 256.0)));
}

@compute @workgroup_size(16, 16)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_index) local_index: u32,
) {
    atomicStore(&workgroup_buckets[local_index], 0u);
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
    atomicAdd(&histogram_buffer[local_index], atomicLoad(&workgroup_buckets[local_index]));
}