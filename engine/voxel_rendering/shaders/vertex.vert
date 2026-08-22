#version 460

struct Instance {
    mat4 transform;
    mat4 normal_matrix;
    uint mesh_idx;
};

struct Mesh {
    uint vertex_offset;
    uint normal_offset;
    uint normals_present;
    uint color_offset;
    uint colors_present;
};

const uint MAX_INSTANCES = 1000;
const uint MAX_MESHES = 1000;

layout(push_constant) uniform PushConstants {
    mat4 view_matrix;
    mat4 projection_matrix;
} cam;

layout(std430, set = 0, binding = 0) readonly buffer ScenarioBuffer {
    Instance instances[MAX_INSTANCES];
    Mesh meshes[MAX_MESHES];
} sb;

layout(std430, set = 0, binding = 1) readonly buffer VertexBuffer {
    float vertices[];
} vb;

layout(std430, set = 0, binding = 2) readonly buffer NormalBuffer {
    float normals[];
} nb;

layout(std430, set = 0, binding = 3) readonly buffer ColorBuffer {
    float colors[];
} cb;

layout(location = 0) out vec3 fragColor;

void main()
{
    Instance instance = sb.instances[gl_InstanceIndex];
    Mesh mesh = sb.meshes[instance.mesh_idx];
    uint local_index = gl_VertexIndex - mesh.vertex_offset;

    vec3 position = vec3(
        vb.vertices[(mesh.vertex_offset + local_index) * 3],
        vb.vertices[(mesh.vertex_offset + local_index) * 3 + 1],
        vb.vertices[(mesh.vertex_offset + local_index) * 3 + 2]
    );

    mat4 mvp = cam.projection_matrix * cam.view_matrix * instance.transform;
    gl_Position = mvp * vec4(position, 1.0);

    vec3 normal = vec3(0.0, 1.0, 0.0);
    if (mesh.normals_present > 0) {
        normal = vec3(
            nb.normals[(mesh.normal_offset + local_index) * 3],
            nb.normals[(mesh.normal_offset + local_index) * 3 + 1],
            nb.normals[(mesh.normal_offset + local_index) * 3 + 2]
        );
    }

    vec3 sun_dir = vec3(0.2, 1.0, 0.5);
    vec3 world_normal = normalize(mat3(instance.normal_matrix) * normal);
    float light = min(max(0.3, dot(normalize(sun_dir), world_normal)), 1.0);

    vec3 color = vec3(1.0);
    if (mesh.colors_present > 0) {
        color = vec3(
            cb.colors[(mesh.color_offset + local_index) * 3],
            cb.colors[(mesh.color_offset + local_index) * 3 + 1],
            cb.colors[(mesh.color_offset + local_index) * 3 + 2]
        );
    }

    fragColor = light * color;
}