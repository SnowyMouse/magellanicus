layout(set = 2, binding = 0) uniform ShaderEnvironmentData {
    float primary_detail_map_scale;
    float secondary_detail_map_scale;
    float bump_map_scale;
    float micro_detail_map_scale;

    uint flags;
    uint shader_environment_type;
    uint detail_map_function;
    uint micro_detail_map_function;
} shader_environment_data;

#define SHADER_ENVIRONMENT_TYPE_NORMAL 0
#define SHADER_ENVIRONMENT_TYPE_BLENDED 1
#define SHADER_ENVIRONMENT_TYPE_BLENDED_BASE_SPECULAR 2
