#version 450

#include "shader_environment_data.glsl"

#define USE_FOG
#define USE_LIGHTMAPS
#include "../include/material.frag"
#include "../include/blend.frag"

layout(location = 0) out vec4 f_color;

layout(location = 0) in vec2 base_map_texture_coordinates;
layout(location = 1) in vec2 lightmap_texture_coordinates;
layout(location = 2) in vec3 camera_position;
layout(location = 3) in vec3 vertex_position;

layout(location = 4) in vec3 normal;
layout(location = 5) in vec3 binormal;
layout(location = 6) in vec3 tangent;

layout(set = 3, binding = 1) uniform sampler map_sampler;
layout(set = 3, binding = 2) uniform texture2D base_map;
layout(set = 3, binding = 3) uniform texture2D primary_detail_map;
layout(set = 3, binding = 4) uniform texture2D secondary_detail_map;
layout(set = 3, binding = 5) uniform texture2D micro_detail_map;
layout(set = 3, binding = 6) uniform texture2D bump_map;
layout(set = 3, binding = 7) uniform textureCube cubemap;

vec3 calculate_world_tangent(vec3 base) {
    return base.xxx * tangent + base.yyy * binormal + base.zzz * normal;
}

vec3 blend_with_mix_type(vec3 color, vec3 with, uint blend_type) {
    switch(blend_type) {
        case 0:
            return double_biased_multiply(color, with);
        case 1:
            return multiply(color, with);
        case 2:
            return double_biased_add(color, with);
        default:
            return vec3(0.0);
    }
}

void main() {
    vec3 camera_difference = camera_position - vertex_position;
    float distance_from_camera = distance(camera_position, vertex_position);

    vec4 base_map_color = texture(sampler2D(base_map, map_sampler), base_map_texture_coordinates);

    vec4 bump_color = texture(
        sampler2D(bump_map, map_sampler),
        base_map_texture_coordinates * shader_environment_data.bump_map_scale
    );

    // Alpha testing
    if((shader_environment_data.flags & SHADER_ENVIRONMENT_FLAGS_ALPHA_TEST) == 1) {
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

    vec3 bump_vector = bump_color.rgb * 2.0 - 1.0;

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

    vec4 blended_detail;
    switch(shader_environment_data.shader_environment_type) {
        case SHADER_ENVIRONMENT_TYPE_NORMAL:
            blended_detail = mix(secondary_detail_map_color, primary_detail_map_color, secondary_detail_map_color.a);
            break;
        case SHADER_ENVIRONMENT_TYPE_BLENDED:
        case SHADER_ENVIRONMENT_TYPE_BLENDED_BASE_SPECULAR:
            blended_detail = mix(secondary_detail_map_color, primary_detail_map_color, base_map_color.a);
            break;
        default:
            f_color = vec4(1.0);
            return;
    }

    // Specular
    vec3 camera_normal = normalize(camera_difference);
    float normal_on_camera = dot(normal, camera_normal);
    vec3 reflection_tangent = calculate_world_tangent(vec3(0.0, 0.0, 1.0));
    vec3 reflection_normal = normalize(2.0 * normal_on_camera * normal - camera_normal);
    vec3 reflection_color = texture(samplerCube(cubemap, map_sampler), reflection_normal + vec3(bump_vector.xy, 0.0)).xyz;
    vec3 specular_color = pow(reflection_color, vec3(8.0));
    float diffuse_reflection = normal_on_camera * normal_on_camera;
    float reflect_attenuation = mix(shader_environment_data.parallel_color.a, shader_environment_data.perpendicular_color.a, diffuse_reflection);
    vec3 specular = mix(shader_environment_data.parallel_color.rgb, shader_environment_data.perpendicular_color.rgb, diffuse_reflection);
    specular = mix(specular_color, reflection_color, specular);
    specular *= reflect_attenuation;

    float specular_mask;
    if((shader_environment_data.flags & SHADER_ENVIRONMENT_FLAGS_BUMPMAP_ALPHA_SPECULAR_MASK) != 0) {
        specular_mask = bump_color.a;
    }
    else if(shader_environment_data.shader_environment_type == SHADER_ENVIRONMENT_TYPE_BLENDED_BASE_SPECULAR) {
        specular_mask = blended_detail.a;
    }
    else {
        specular_mask = base_map_color.a;
    }
    specular *= specular_mask;

    // Specular
    base_map_color.rgb = clamp(base_map_color.rgb + specular.rgb, vec3(0.0), vec3(1.0));

    // Lightmap stage
    base_map_color.rgb *= lightmap_color.rgb;

    // Detail
    vec3 scratch_color = blended_detail.rgb;
    scratch_color = blend_with_mix_type(base_map_color.rgb, scratch_color, shader_environment_data.detail_map_function);
    scratch_color = blend_with_mix_type(micro_detail_map_color.rgb, scratch_color, shader_environment_data.micro_detail_map_function);

    // Bumpmap
    float base_shading = dot(bump_vector, vec3(0.0, 0.0, 1.0));
    scratch_color.rgb *= vec3(base_shading);

    // Fog stage
    scratch_color.rgb = apply_fog(distance_from_camera, scratch_color.rgb);

    f_color = vec4(scratch_color, 1.0);
}
