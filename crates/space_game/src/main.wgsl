struct Camera {
    inv_view_projection: mat4x4<f32>;
    viewport: vec2<f32>;
    near: f32;
    far: f32;
};

var<private> tau: f32 = 6.28318530717958647692528676655900577;
var<private> pi: f32 = 3.14159265358979323846264338327950288;

[[group(0), binding(0)]]
var starmap_tex: texture_2d<f32>;
[[group(0), binding(1)]]
var starmap_sampler: sampler;
[[group(0), binding(2)]]
var<uniform> camera: Camera;

struct Vertex {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] world_ray: vec3<f32>;
};

var<private> fullscreen_quad: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
    vec2<f32>(1.0, 1.0),
    vec2<f32>(-1.0, 1.0),
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(1.0, -1.0),
);

fn inv_project(
    ndc: vec3<f32>,
    inv_view_projection: mat4x4<f32>
) -> vec3<f32> {
    let proj = inv_view_projection * vec4<f32>(ndc.x, ndc.y, ndc.z, 1.0);
    return proj.xyz / proj.w;
}

[[stage(vertex)]]
fn vert_main(
    [[builtin(vertex_index)]] index: u32,
) -> Vertex {
    var vert: Vertex;

    let pos_xy = fullscreen_quad[index];
    vert.position = vec4<f32>(pos_xy.x, pos_xy.y, 0.0, 1.0);

    let near_world = inv_project(
        vec3<f32>(vert.position.x, vert.position.y, 0.0), 
        camera.inv_view_projection
    );
    let far_world = inv_project(
        vec3<f32>(vert.position.x, vert.position.y, 1.0), 
        camera.inv_view_projection
    );
    vert.world_ray = normalize(far_world - near_world);
    return vert;
}

[[stage(fragment)]]
fn frag_main(
    vert: Vertex,
) -> [[location(0)]] vec4<f32> {
    let pos = vec2<f32>(
        atan2(vert.world_ray.x, vert.world_ray.z) / tau + 0.5,
        atan(-vert.world_ray.y / length(vert.world_ray.xz)) / pi + 0.5,
    );

    return textureSample(starmap_tex, starmap_sampler, pos);
}
