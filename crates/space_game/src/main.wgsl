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
    [[builtin(position)]] position: vec4<f32>,
) -> [[location(0)]] vec4<f32> {
    var color: vec4<f32>;
    color.r = position.x / 1024.0;
    color.g = position.y / 768.0;
    color.b = 0.2;
    color.a = 1.0;
    return color;
}
