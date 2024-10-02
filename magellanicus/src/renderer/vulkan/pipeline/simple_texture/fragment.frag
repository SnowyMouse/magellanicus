#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 f_color;

layout(set = 1, binding = 0) uniform sampler s;
layout(set = 1, binding = 1) uniform texture2D tex;

void main() {
    vec4 color = texture(sampler2D(tex, s), tex_coords);
    f_color = color;
}
