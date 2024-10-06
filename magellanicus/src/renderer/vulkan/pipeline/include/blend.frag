vec4 alpha_blend(vec4 base, vec4 with) {
    return mix(base, with, with.a);
}

vec4 double_biased_multiply(vec4 base, vec4 with) {
    return vec4(base.rgb * (with.rgb * 2), base.a);
}

vec4 double_biased_add(vec4 base, vec4 with) {
    return vec4(base.rgb + (with.rgb * 2), base.a);
}

vec4 multiply(vec4 base, vec4 with) {
    return vec4(base.rgb * with.rgb, base.a);
}
