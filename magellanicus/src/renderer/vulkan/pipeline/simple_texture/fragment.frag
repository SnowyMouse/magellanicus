#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 1) uniform sampler s;
layout(set = 0, binding = 2) uniform texture2D tex;

void main() {
    vec4 color = texture(sampler2D(tex, s), tex_coords);
    if(color.a == 0.0) {
        discard;
    }
    f_color = color;
}
