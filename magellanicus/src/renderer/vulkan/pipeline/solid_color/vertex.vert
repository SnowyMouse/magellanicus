#version 450

#include "../include/material.vert"

layout(location = 0) out vec3 color;

void main() {
    mat4 worldview = uniforms.view * uniforms.world;
    gl_Position = uniforms.proj * worldview * vec4((position.xyz + uniforms.offset.xyz), 1.0);
    color = mod(position, 1.0);
}
