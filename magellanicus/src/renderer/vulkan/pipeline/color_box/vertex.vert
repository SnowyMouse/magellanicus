#version 450

#include "../include/material.vert"

void main() {
    gl_Position = vec4((position * 2.0) - 1.0, 1.0);
}
