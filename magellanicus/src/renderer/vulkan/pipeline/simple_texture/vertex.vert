#version 450

#define USE_TEXTURE_COORDS
#define USE_LIGHTMAPS

#include "../include/material.vert"

layout(location = 0) out vec2 texcoords;
layout(location = 1) out vec2 lightmap_texcoords;
layout(location = 2) out float distance_from_camera;

void main() {
    mat4 worldview = uniforms.view * uniforms.world;
    vec3 offset = position.xyz + uniforms.offset.xyz;

    gl_Position = uniforms.proj * worldview * vec4(offset, 1.0);
    texcoords = texture_coords.xy;
    lightmap_texcoords = lightmap_texture_coords.xy;

    vec3 distance_bork = offset - uniforms.camera;
    vec3 distance = sqrt(distance_bork * distance_bork);
    distance_from_camera = distance.x + distance.y + distance.z;
}
