#ifdef USE_LIGHTMAPS
layout(set = 1, binding = 0) uniform sampler lightmap_sampler;
layout(set = 1, binding = 1) uniform texture2D lightmap_texture;
#endif

#ifdef USE_FOG
layout(set = 2, binding = 0) uniform FogData {
    vec4 sky_fog_color;
    float sky_fog_from;
    float sky_fog_to;
    float min_opacity;
    float max_opacity;
} sky_fog_data;
#endif
