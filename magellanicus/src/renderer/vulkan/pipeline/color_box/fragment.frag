#version 450

layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0) uniform InputColor {
    vec4 color;
} color;

void main() {
    f_color = color.color;
}
