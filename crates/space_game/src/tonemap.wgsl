[[group(0), binding(0)]]
var hdr_tex: texture_2d<f32>;
[[group(0), binding(1)]]
var hdr_sampler: sampler;

var<private> fullscreen_quad: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
    vec2<f32>(1.0, 1.0),
    vec2<f32>(-1.0, 1.0),
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(1.0, -1.0),
);

struct Vertex {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] screen_pos: vec2<f32>;
};

[[stage(vertex)]]
fn vert_main(
    [[builtin(vertex_index)]] index: u32,
) -> Vertex {
    var vert: Vertex;

    let pos_xy = fullscreen_quad[index];
    vert.position = vec4<f32>(pos_xy.x, pos_xy.y, 0.0, 1.0);
    vert.screen_pos = (pos_xy + 1.0) / 2.0;

    return vert;
}

[[stage(fragment)]]
fn frag_main(
    vert: Vertex,
) -> [[location(0)]] vec4<f32> {
    let intensity = textureSample(hdr_tex, hdr_sampler, vert.screen_pos).rgb;
    let ldr = intensity / (1.0 + intensity);
    return vec4<f32>(ldr.r, ldr.g, ldr.b, 1.0);
}   
