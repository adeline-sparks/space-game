#version 300 es

#include "test_include.glsl"

uniform mat4x4 model_view_projection;
uniform mat4x4 model_matrix;
uniform mat4x4 normal_matrix;

in vec3 vert_pos;
in vec3 vert_normal;
out vec3 frag_world_pos;
out vec3 frag_world_normal;

void main() { 
    vec4 pos;
    pos.xyz = vert_pos;
    pos.w = 1.0;

    gl_Position = model_view_projection * pos;
    frag_world_pos = (model_matrix * pos).xyz;

    vec4 normal;
    normal.xyz = vert_normal;
    normal.w = 0.0;
    frag_world_normal = (normal_matrix * normal).xyz;
}