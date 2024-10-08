vec4 alpha_blend(vec4 base, vec4 with) {
    return mix(base, with, with.a);
}

vec3 double_biased_multiply(vec3 base, vec3 with) {
    vec3 multiplied = base * (with * 2);
    return clamp(multiplied, vec3(0.0), vec3(1.0));
}

vec3 double_biased_add(vec3 base, vec3 with) {
    vec3 added = base + (with * 2) - vec3(1);
    return clamp(added, vec3(0.0), vec3(1.0));
}

vec3 multiply(vec3 base, vec3 with) {
    vec3 multiplied = base * with;
    return clamp(multiplied, vec3(0.0), vec3(1.0));
}
