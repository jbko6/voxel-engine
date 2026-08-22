#version 460
#extension GL_ARB_shader_draw_parameters : require

layout(push_constant) uniform PushConstants {
    vec3 camera_position;
    mat4 view_matrix;
    mat4 projection_matrix;
} camera_data;

const uint MAX_OBJECT_COUNT = 16384;

struct ObjectData
{
    mat4 transform;
};

layout(std430, set = 0, binding = 0) readonly buffer ObjectBuffer {
    ObjectData obj_data[];
} object_buffer;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec3 inColor;

layout(location = 0) out vec3 fragColor;

void main()
{
    mat4 transform = object_buffer.obj_data[gl_DrawID].transform;
    vec4 world_position = transform * vec4(inPosition, 1.0);
    vec4 view_position = camera_data.view_matrix * world_position;
    vec4 clip_position = camera_data.projection_matrix * view_position;
    gl_Position = clip_position;

    // basic diffuse shading
    vec3 sun_dir = vec3(0.2, -1.0, 0.5);
    mat3 normal_matrix = mat3(transpose(inverse(transform)));
    vec3 world_normal = normalize(normal_matrix * inNormal);
    float light = min(max(0.3, dot(normalize(sun_dir), world_normal)), 1.0);

    fragColor = inColor * light;
}