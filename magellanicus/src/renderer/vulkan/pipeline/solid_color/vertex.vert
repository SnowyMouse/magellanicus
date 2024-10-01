#version 450

layout(location = 0) in vec3 position;
layout(location = 0) out vec3 color;

layout(set = 0, binding = 0) uniform ModelData {
    mat4 world;
    mat4 view;
    mat4 proj;
    vec3 offset;
    mat3 rotation;
} uniforms;

void main() {
    mat4 worldview = uniforms.view * uniforms.world;
    gl_Position = uniforms.proj * worldview * vec4((position.xyz + uniforms.offset.xyz), 1.0);
    color = mod(position, 1.0);
}
