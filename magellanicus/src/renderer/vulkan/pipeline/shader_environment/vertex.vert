#version 450

#include "shader_environment_data.glsl"

#define USE_TEXTURE_COORDS
#define USE_LIGHTMAPS

#include "../include/material.vert"

layout(location = 4) in vec3 normal;

layout(location = 0) out vec2 base_map_texture_coordinates;
layout(location = 1) out vec2 lightmap_texture_coordinates;
layout(location = 2) out vec3 camera_position;
layout(location = 3) out vec3 vertex_position;

void main() {
    mat4 worldview = uniforms.view * uniforms.world;
    vertex_position = position.xyz + uniforms.offset.xyz;
    camera_position = uniforms.camera;
    gl_Position = uniforms.proj * worldview * vec4(vertex_position, 1.0);
    base_map_texture_coordinates = texture_coords.xy;
    lightmap_texture_coordinates = lightmap_texture_coords.xy;
}
