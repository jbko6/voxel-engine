#version 460

layout(location = 0) in vec3 fragColor;
layout(location = 0) out vec4 fragment_color;

void main()
{
    fragment_color = vec4(fragColor, 1.0);
}