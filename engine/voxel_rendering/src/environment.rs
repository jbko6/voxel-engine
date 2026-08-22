use std::sync::Arc;
use ash::vk;

use glam::{Mat4, Vec3};

use crate::{Buffer, Mesh, Vertex};

#[derive(Clone, Copy)]
#[repr(C)]
pub struct CameraData {
    pub position: Vec3,
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct GPUObjectData {
    pub transform: Mat4
}

#[derive(Clone, Copy)]
pub struct ObjectData {
    pub transform: Mat4,
    pub mesh_descriptor_idx: usize,
}

pub struct MeshDescriptor {
    pub first_index: u32,
    pub index_count: u32,
    pub vertex_offset: i32,
}

pub struct Environment {
    device: Arc<ash::Device>,
    vertex_buffer: Box<dyn Buffer<Vertex>>,
    vertex_count: usize,
    index_buffer: Box<dyn Buffer<u32>>,
    index_count: usize,
    object_buffer: Box<dyn Buffer<GPUObjectData>>,
    mesh_descriptors: Vec<MeshDescriptor>,
    objects: Vec<ObjectData>,
    camera: CameraData,
}

impl Environment {
    pub fn new(device: Arc<ash::Device>, vertex_buffer: Box<dyn Buffer<Vertex>>, index_buffer: Box<dyn Buffer<u32>>, object_buffer: Box<dyn Buffer<GPUObjectData>>) -> Self {
        Environment {
            device,
            vertex_buffer,
            vertex_count: 0,
            index_buffer,
            index_count: 0,
            object_buffer,
            mesh_descriptors: Vec::new(),
            objects: Vec::new(),
            camera: CameraData {
                position: Vec3::new(0.0, 0.0, 0.0),
                view_matrix: Mat4::IDENTITY,
                projection_matrix: Mat4::IDENTITY,
            },
        }
    }

    pub fn vertex_buffer(&self) -> &dyn Buffer<Vertex> {
        self.vertex_buffer.as_ref()
    }

    pub fn index_buffer(&self) -> &dyn Buffer<u32> {
        self.index_buffer.as_ref()
    }

    pub fn object_buffer(&self) -> &dyn Buffer<GPUObjectData> {
        self.object_buffer.as_ref()
    }

    pub fn set_camera_data(&mut self, camera_data: CameraData) {
        self.camera = camera_data;
    }

    pub fn camera_data(&self) -> &CameraData {
        &self.camera
    }

    pub fn add_mesh(&mut self, mesh: &Mesh) -> usize {
        let vertices = mesh.vertices();
        let indices = mesh.indices();
        let vertex_offset = self.vertex_count as i32;
        let first_index = self.index_count as u32;
        let index_count = indices.len() as u32;

        self.vertex_buffer.upload(vertices, vertex_offset as u32);
        self.index_buffer.upload(indices, first_index);

        let mesh_descriptor = MeshDescriptor {
            first_index,
            index_count,
            vertex_offset,
        };

        self.vertex_count += vertices.len();
        self.index_count += indices.len();

        self.mesh_descriptors.push(mesh_descriptor);
        self.mesh_descriptors.len() - 1 // Return the index of the newly added mesh descriptor
    }

    pub fn update_mesh(&mut self, mesh_descriptor_idx: usize, mesh: &Mesh) {
        if mesh_descriptor_idx >= self.mesh_descriptors.len() {
            panic!("Invalid mesh descriptor index");
        }

        let mesh_descriptor = &self.mesh_descriptors[mesh_descriptor_idx];

        let vertices = mesh.vertices();
        let indices = mesh.indices();

        if vertices.len() != mesh_descriptor.index_count as usize || indices.len() != mesh_descriptor.index_count as usize {
            panic!("Vertex or index count does not match the original mesh descriptor");
        }

        // Update vertex buffer
        self.vertex_buffer.upload(vertices, mesh_descriptor.vertex_offset as u32);

        // Update index buffer
        self.index_buffer.upload(indices, mesh_descriptor.first_index);
    }

    pub fn mesh_descriptors(&self) -> &Vec<MeshDescriptor> {
        &self.mesh_descriptors
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    pub fn add_object(&mut self, object: ObjectData) {
        self.object_buffer.upload(&[GPUObjectData { transform: object.transform }], self.object_count() as u32);
        self.objects.push(object);
    }

    pub fn objects(&self) -> &Vec<ObjectData> {
        &self.objects
    }

    pub fn draw_commands(&self) -> Vec<vk::DrawIndexedIndirectCommand> {
        // later: group by mesh descriptor and instance count, but for now just return one command per object
        self.objects.iter().map(|object| {
            let mesh_descriptor = &self.mesh_descriptors[object.mesh_descriptor_idx];
            vk::DrawIndexedIndirectCommand {
                index_count: mesh_descriptor.index_count,
                instance_count: 1,
                first_index: mesh_descriptor.first_index,
                vertex_offset: mesh_descriptor.vertex_offset,
                first_instance: 0,
            }
        }).collect()
    }
}