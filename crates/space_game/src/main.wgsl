[[group(0), binding(0)]]
var starmap_tex: texture_2d<f32>;
[[group(0), binding(1)]]
var starmap_sampler: sampler;

[[stage(vertex)]]
fn vert_main(
    [[builtin(vertex_index)]] vertex_index: u32,
) -> [[builtin(position)]] vec4<f32> {
    if (vertex_index == 0u) {
        return vec4<f32>(1.0, 1.0, 0.0, 1.0);
    } else if (vertex_index == 1u) {
        return vec4<f32>(-10.0, 1.0, 0.0, 1.0);
    } else {
        return vec4<f32>(1.0, -10.0, 0.0, 1.0);
    }
}

[[stage(fragment)]]
fn frag_main(
    [[builtin(position)]] frag_pos: vec4<f32>,
) -> [[location(0)]] vec4<f32> {
    let pos = frag_pos.xy / vec2<f32>(1024.0, 768.0);
    return textureLoad(starmap_tex, vec2<i32>(pos * vec2<f32>(4096.0, 2048.0)), 0);
}
