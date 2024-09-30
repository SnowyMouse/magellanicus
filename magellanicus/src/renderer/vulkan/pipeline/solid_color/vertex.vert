#version 450

layout(location = 0) in vec3 position;
layout(location = 0) out vec3 color;

void main() {
    gl_Position = vec4(position.xyz, 1.0);
    color = mod(position, 1.0);
}
