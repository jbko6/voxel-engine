#version 460

layout(location = 0) out vec3 fragColor;
vec2 positions[3] = vec2[](
    vec2(-0.5, -0.5),
    vec2(-0.5,  0.5),
    vec2( 0.5,  0.5)
);
void main()
{
    vec2 position = positions[gl_VertexIndex];
    gl_Position = vec4(position, 0.0, 1.0);
    fragColor = vec3(1.0, 0.0, 0.0);
}