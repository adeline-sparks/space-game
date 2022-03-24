#version 300 es

precision highp float;

uniform sampler2D tex_color;
uniform sampler2D tex_normal;
uniform float tex_scale;
uniform float tex_blend_sharpness;
uniform vec3 light_dir;

in vec3 frag_world_pos;
in vec3 frag_world_normal;
out vec4 out_color;

void main() {
    vec3 uv = frag_world_pos * tex_scale;
    vec3 weights = pow(abs(frag_world_normal), vec3(tex_blend_sharpness));
    weights /= (weights.x + weights.y + weights.z);

    mat3 colors = mat3(
        pow(texture(tex_color, uv.yz).rgb, vec3(2.2)),
        pow(texture(tex_color, uv.xz).rgb, vec3(2.2)),
        pow(texture(tex_color, uv.xy).rgb, vec3(2.2))
    );
    vec3 color = colors * weights;

    mat3 normals = mat3(
        texture(tex_normal, uv.yz).rgb,
        texture(tex_normal, uv.xz).rgb,
        texture(tex_normal, uv.xy).rgb
    );
    normals = 2.0 * normals - 1.0;
    normals[0].xy += frag_world_normal.zy;
    normals[1].xy += frag_world_normal.xz;
    normals[2].xy += frag_world_normal.xy;
    normals[0].z = abs(normals[0].z) * frag_world_normal.x;
    normals[1].z = abs(normals[1].z) * frag_world_normal.y;
    normals[2].z = abs(normals[2].z) * frag_world_normal.z;
    normals[0] = normals[0].zyx;
    normals[1] = normals[1].xzy;
    vec3 normal = normalize(normals * weights);

    out_color.rgb = .3 * dot(light_dir, normal) + color;
    //out_color.rgb = normal / 2.0 + 0.5;
    out_color.rgb = pow(out_color.rgb, vec3(1.0/2.2));
    out_color.a = 1.0;
}