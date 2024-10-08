#version 450

#include "shader_environment_data.glsl"

#define USE_FOG
#define USE_LIGHTMAPS
#include "../include/material.frag"
#include "../include/blend.frag"

layout(location = 0) out vec4 f_color;

layout(location = 0) in vec2 base_map_texture_coordinates;
layout(location = 1) in vec2 lightmap_texture_coordinates;
layout(location = 2) in float distance_from_camera;

layout(set = 3, binding = 1) uniform sampler map_sampler;
layout(set = 3, binding = 2) uniform texture2D base_map;
layout(set = 3, binding = 3) uniform texture2D primary_detail_map;
layout(set = 3, binding = 4) uniform texture2D secondary_detail_map;
layout(set = 3, binding = 5) uniform texture2D micro_detail_map;
layout(set = 3, binding = 6) uniform texture2D bump_map;

vec4 blend_with_mix_type(vec4 color, vec4 with, uint blend_type, float alpha) {
    vec4 blender;

    switch(blend_type) {
        case 0:
            blender = double_biased_multiply(color, with);
            break;
        case 1:
            blender = multiply(color, with);
            break;
        case 2:
            blender = double_biased_add(color, with);
            break;
        default:
            return vec4(0.0);
    }

    return mix(color, vec4(blender.rgb, 1.0), alpha);
}

void main() {
    vec4 base_map_color = texture(sampler2D(base_map, map_sampler), base_map_texture_coordinates);

    vec4 bump_color = texture(
        sampler2D(bump_map, map_sampler),
        base_map_texture_coordinates * shader_environment_data.bump_map_scale
    );

    // Alpha testing
    if((shader_environment_data.flags & 1) == 1) {
        // TODO: Is it just normal that discards 0-alpha pixels? The alpha is used for blending and specular on other
        // types, so it makes no sense to test alpha on those types.
        if(shader_environment_data.shader_environment_type == SHADER_ENVIRONMENT_TYPE_NORMAL && base_map_color.a == 0.0) {
            discard;
        }

        if(bump_color.a <= 0.5) {
            discard;
        }
    }
    bump_color.a = 1.0;

    vec4 primary_detail_map_color = texture(
        sampler2D(primary_detail_map, map_sampler),
        base_map_texture_coordinates * shader_environment_data.primary_detail_map_scale
    );

    vec4 secondary_detail_map_color = texture(
        sampler2D(secondary_detail_map, map_sampler),
        base_map_texture_coordinates * shader_environment_data.secondary_detail_map_scale
    );

    vec4 micro_detail_map_color = texture(
        sampler2D(micro_detail_map, map_sampler),
        base_map_texture_coordinates * shader_environment_data.micro_detail_map_scale
    );

    vec4 lightmap_color = texture(
        sampler2D(lightmap_texture, lightmap_sampler),
        lightmap_texture_coordinates
    );

    float primary_blending = primary_detail_map_color.a;
    float secondary_blending = secondary_detail_map_color.a;

    if(shader_environment_data.shader_environment_type == SHADER_ENVIRONMENT_TYPE_BLENDED || shader_environment_data.shader_environment_type == SHADER_ENVIRONMENT_TYPE_BLENDED_BASE_SPECULAR) {
        primary_blending *= base_map_color.a;
        secondary_blending *= 1.0 - primary_blending;
    }

    vec4 scratch_color = base_map_color;
    scratch_color = blend_with_mix_type(scratch_color, primary_detail_map_color, shader_environment_data.detail_map_function, primary_blending);
    scratch_color = blend_with_mix_type(scratch_color, secondary_detail_map_color, shader_environment_data.detail_map_function, secondary_blending);
    scratch_color = blend_with_mix_type(scratch_color, micro_detail_map_color, shader_environment_data.micro_detail_map_function, micro_detail_map_color.a);
    scratch_color = vec4(scratch_color.rgb * lightmap_color.rgb, 1.0);

    float clamped = clamp(distance_from_camera, sky_fog_data.sky_fog_from, sky_fog_data.sky_fog_to);
    float fog_density = (clamped - sky_fog_data.sky_fog_from) / (sky_fog_data.sky_fog_to - sky_fog_data.sky_fog_from);
    scratch_color.rgb = mix(scratch_color.rgb, sky_fog_data.sky_fog_color.rgb, sqrt(fog_density) * sky_fog_data.max_opacity);

    f_color = scratch_color;
}
