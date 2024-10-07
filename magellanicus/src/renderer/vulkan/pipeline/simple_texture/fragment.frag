#version 450

#define USE_LIGHTMAPS
#define USE_FOG
#include "../include/material.frag"

layout(location = 0) out vec4 f_color;

layout(location = 0) in vec2 tex_coords;
layout(location = 1) in vec2 lightmap_texcoords;
layout(location = 2) in float distance_from_camera;

layout(set = 3, binding = 0) uniform sampler s;
layout(set = 3, binding = 1) uniform texture2D tex;

void main() {
    vec4 lightmap_color = texture(sampler2D(lightmap_texture, lightmap_sampler), lightmap_texcoords);
    vec4 color = texture(sampler2D(tex, s), tex_coords);
    vec4 lightmapped_color = vec4(color.rgb * lightmap_color.rgb, 1.0);

    // FIXME: Messes with additive transparent stuff
    float clamped = clamp(distance_from_camera, sky_fog_data.sky_fog_from, sky_fog_data.sky_fog_to);
    float fog_density = (clamped - sky_fog_data.sky_fog_from) / (sky_fog_data.sky_fog_to - sky_fog_data.sky_fog_from) * sky_fog_data.max_opacity;
    lightmapped_color.rgb = mix(lightmapped_color.rgb, sky_fog_data.sky_fog_color.rgb, fog_density * 0.0);

    f_color = lightmapped_color;
}
