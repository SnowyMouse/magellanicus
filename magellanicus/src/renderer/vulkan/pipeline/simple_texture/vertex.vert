#version 450

#define USE_TEXTURE_COORDS
#define USE_LIGHTMAPS

#include "../include/material.vert"

layout(location = 0) out vec2 texcoords;
layout(location = 1) out vec2 lightmap_texcoords;

void main() {
    mat4 worldview = uniforms.view * uniforms.world;
    gl_Position = uniforms.proj * worldview * vec4((position.xyz + uniforms.offset.xyz), 1.0);
    texcoords = texture_coords.xy;
    lightmap_texcoords = lightmap_texture_coords.xy;
}
