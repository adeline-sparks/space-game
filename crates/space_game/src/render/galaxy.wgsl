struct Camera {
    inv_view_projection: mat4x4<f32>,
    viewport: vec2<f32>,
    near: f32,
    far: f32,
};

@group(0) @binding(0)
var starmap_tex: texture_cube<f32>;
@group(0) @binding(1)
var starmap_sampler: sampler;
@group(0) @binding(2)
var<uniform> camera: Camera;

struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) world_ray: vec3<f32>,
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

@vertex
fn vert_main(
    @builtin(vertex_index) index: u32,
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
    vert.world_ray = far_world - near_world;
    return vert;
}

@fragment
fn frag_main(
    vert: Vertex,
) -> @location(0) vec4<f32> {
    return textureSample(starmap_tex, starmap_sampler, vert.world_ray);
}
