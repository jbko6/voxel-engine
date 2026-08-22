#version 460
#extension GL_EXT_buffer_reference : require
#extension GL_EXT_buffer_reference2 : require

layout(buffer_reference, std430, buffer_reference_align = 4) readonly buffer VertexBuffer {
    float vertices[];
};

layout(buffer_reference, std430, buffer_reference_align = 4) readonly buffer IndexBuffer {
    uint indices[];
};

layout(buffer_reference, std430, buffer_reference_align = 4) readonly buffer NormalBuffer {
    float normals[];
};

layout(buffer_reference, std430, buffer_reference_align = 4) readonly buffer ColorBuffer {
    float colors[];
};

// Align 16, size 128
struct Camera {
    mat4 view_matrix;       // 64 bytes
    mat4 projection_matrix; // 64 bytes
};

// Align 16, size 80
struct Instance {
    mat4 transform;         // 64 bytes
    uint mesh_idx;          // 4 bytes
    // 12 bytes of padding
};

// Align 8, size 56
struct Mesh {
    VertexBuffer vertex_buff;  // 8 bytes
    IndexBuffer index_buff;    // 8 bytes
    uint index_count;          // 4 bytes
    // 4 bytes padding
    NormalBuffer normal_buff;  // 8 bytes
    uint normal_count;         // 4 bytes
    // 4 bytes padding
    ColorBuffer color_buff;    // 8 bytes
    // 4 bytes padding
    uint color_count;          // 4 bytes
};

const uint MAX_INSTANCES = 1000;
const uint MAX_MESHES = 1000;

// Align 16, size 136,128
layout(std430, set = 0, binding = 0) readonly buffer ScenarioBuffer {
    Camera cam;
    Instance instances[MAX_INSTANCES];
    Mesh meshes[MAX_MESHES];
} sb;

layout(location = 0) out vec3 fragColor;

void main()
{
    Instance instance = sb.instances[gl_InstanceIndex];
    Mesh mesh = sb.meshes[instance.mesh_idx];
    VertexBuffer vb = mesh.vertex_buff;

    uint index;
    //if (mesh.index_count > 0) {
    //    IndexBuffer ib = mesh.index_buff;
    //    index = ib.indices[gl_VertexIndex];
    //} else {
        index = gl_VertexIndex;
    //}

    vec3 position = vec3(
        vb.vertices[index * 3],
        vb.vertices[index * 3 + 1],
        vb.vertices[index * 3 + 2]
    );
    
    Camera cam = sb.cam;
    mat4 mvp = cam.projection_matrix * cam.view_matrix * instance.transform;
    gl_Position = mvp * vec4(position, 1.0);

    //vec3 normal = vec3(0.0, 1.0, 0.0);
    //if (mesh.normal_count > 0) {
    //    NormalBuffer nb = mesh.normal_buff;
    //    normal = vec3(
    //        nb.normals[index * 3],
    //        nb.normals[index * 3 + 1],
    //        nb.normals[index * 3 + 2]
    //    );
    //}

    // basic diffuse shading
    //vec3 sun_dir = vec3(0.2, 1.0, 0.5);
    //mat3 normal_matrix = mat3(transpose(inverse(instance.transform)));
    //vec3 world_normal = normalize(normal_matrix * normal);
    //float light = min(max(0.3, dot(normalize(sun_dir), world_normal)), 1.0);

    //vec3 color = vec3(1.0);
    //if (mesh.color_count > 0) {
    //    ColorBuffer cb = mesh.color_buff;
    //    color = vec3(
    //        cb.colors[index * 3],
    //        cb.colors[index * 3 + 1],
    //        cb.colors[index * 3 + 2]
    //    );
    //}
    //fragColor = light * color;
}