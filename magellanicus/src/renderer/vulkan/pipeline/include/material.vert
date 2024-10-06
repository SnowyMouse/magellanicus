layout(location = 0) in vec3 position;

#ifdef USE_TEXTURE_COORDS
layout(location = 1) in vec2 texture_coords;
#endif
#ifdef USE_LIGHTMAPS
layout(location = 2) in vec2 lightmap_texture_coords;
#endif

layout(set = 0, binding = 0) uniform ModelData {
    mat4 world;
    mat4 view;
    mat4 proj;
    vec3 offset;
    mat3 rotation;
} uniforms;
