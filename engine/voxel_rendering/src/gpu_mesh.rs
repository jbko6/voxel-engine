use ash::vk;
use std::{sync::Arc};

use crate::Renderer;
use crate::Vertex;

#[derive(Clone)]
pub struct GPUMesh {
    pub vertex_buffer_memory: vk::DeviceMemory,
    pub vertex_buffer: vk::Buffer,
    pub index_buffer_memory: vk::DeviceMemory,
    pub index_buffer: vk::Buffer,
    pub index_count: u32,
    device: Arc<ash::Device>,
}

impl Renderer {
    pub fn load_obj(&self, path: &str) -> Option<GPUMesh> {

        // return none if the file doesn't exist
        let models =
            tobj::load_obj(path, &tobj::LoadOptions::default());

        if models.is_err() {
            println!("Failed to load OBJ file: {}", path);
            return None;
        }

        let (models, _) = models.unwrap();

        println!("Loaded {} models from {}", models.len(), path);

        if models.len() == 0 {
            print!("No models found in OBJ file: {}", path);
            return None;
        }

        let model = &models[0];

        let mesh = &model.mesh;

        let indices = mesh.indices.clone();

        let normals_present = mesh.normals.len() > 0;
        let colors_present = mesh.vertex_color.len() > 0;

        let vertices = mesh
            .positions
            .chunks(3)
            .enumerate()
            .map(|(i, pos)| {
                let normal = if normals_present {
                    let normals = &mesh.normals[i * 3..i * 3 + 3];
                    [normals[0], normals[1], normals[2]]
                } else {
                    [0.0, 0.0, 0.0]
                };
                let color = if colors_present {
                    let colors = &mesh.vertex_color[i * 3..i * 3 + 3];
                    [colors[0], colors[1], colors[2]]
                } else {
                    [1.0, 1.0, 1.0]
                };
                Vertex {
                    pos: [pos[0], pos[1], pos[2]],
                    normal,
                    color,
                }
            })
            .collect::<Vec<Vertex>>();

        println!(
            "Loaded {} vertices and {} indices from {}",
            vertices.len(),
            indices.len(),
            path
        );

        // Create vertex buffer
        let vertex_buffer_size =
            (std::mem::size_of::<Vertex>() * vertices.len()) as vk::DeviceSize;

        let (vertex_buffer, vertex_buffer_memory) = self.create_buffer(
            vertex_buffer_size,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );

        self.upload_to_buffer(vertex_buffer_memory, vertices.as_slice());

        let index_buffer_size = (std::mem::size_of::<u32>() * indices.len()) as vk::DeviceSize;
        let (index_buffer, index_buffer_memory) = self.create_buffer(
            index_buffer_size,
            vk::BufferUsageFlags::INDEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );

        self.upload_to_buffer(index_buffer_memory, indices.as_slice());

        Some(GPUMesh {
            vertex_buffer_memory,
            vertex_buffer,
            index_buffer_memory,
            index_buffer,
            device: self.device.clone(),
            index_count: indices.len() as u32,
        })
    }

    pub fn from_mesh(&self, mesh: &crate::Mesh) -> Option<GPUMesh> {
        let vertices = mesh.vertices();
        let indices = mesh.indices();

        if vertices.is_empty() || indices.is_empty() {
            return None;
        }

        // Create vertex buffer
        let vertex_buffer_size =
            (std::mem::size_of::<Vertex>() * vertices.len()) as vk::DeviceSize;

        let (vertex_buffer, vertex_buffer_memory) = self.create_buffer(
            vertex_buffer_size,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );

        self.upload_to_buffer(vertex_buffer_memory, vertices.as_slice());

        let index_buffer_size = (std::mem::size_of::<u32>() * indices.len()) as vk::DeviceSize;
        let (index_buffer, index_buffer_memory) = self.create_buffer(
            index_buffer_size,
            vk::BufferUsageFlags::INDEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );

        self.upload_to_buffer(index_buffer_memory, indices.as_slice());

        Some(GPUMesh {
            vertex_buffer_memory,
            vertex_buffer,
            index_buffer_memory,
            index_buffer,
            device: self.device.clone(),
            index_count: indices.len() as u32,
        })
    }
}

impl Drop for GPUMesh {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle()
                .expect("Failed to wait for device idle");

            self.device.destroy_buffer(self.vertex_buffer, None);
            self.device.free_memory(self.vertex_buffer_memory, None);
            if self.index_buffer != vk::Buffer::null() {
                self.device.destroy_buffer(self.index_buffer, None);
                self.device.free_memory(self.index_buffer_memory, None);
            }
        }
    }
}