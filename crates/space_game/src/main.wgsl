struct Vertex {
    [[builtin(position)]] clip_position: vec4<f32>;
};

[[stage(vertex)]]
fn vert_main(
    [[builtin(vertex_index)]] vertex_index: u32,
) -> Vertex {
    var vert: Vertex;
    if (vertex_index == 0u) {
        vert.clip_position = vec4<f32>(0.0, 0.5, 0.0, 1.0);
    } else if (vertex_index == 1u) {
        vert.clip_position = vec4<f32>(-0.5, -0.5, 0.0, 1.0);
    } else {
        vert.clip_position = vec4<f32>(0.5, -0.5, 0.0, 1.0);
    }
    return vert;
}

[[stage(fragment)]]
fn frag_main(
    vert: Vertex
) -> [[location(0)]] vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
