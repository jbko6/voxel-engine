use ash::vk;
use std::{hash::Hash, sync::Arc};

use crate::Renderer;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct MeshVertex {
    pos: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

pub struct Mesh {
    pub vertex_buffer_memory: vk::DeviceMemory,
    pub vertex_buffer: vk::Buffer,
    pub index_buffer_memory: vk::DeviceMemory,
    pub index_buffer: vk::Buffer,
    pub index_count: u32,
    device: Arc<ash::Device>,
}

impl Renderer {
    pub fn load_obj(&self, path: &str) -> Mesh {
        let (models, _) =
            tobj::load_obj(path, &tobj::LoadOptions::default()).expect("Failed to load OBJ file");

        println!("Loaded {} models from {}", models.len(), path);

        if models.len() == 0 {
            panic!("No models found in OBJ file: {}", path);
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
                    // maybe later compute?
                    [0.0, 0.0, 1.0]
                };
                let color = if colors_present {
                    let colors = &mesh.vertex_color[i * 3..i * 3 + 3];
                    [colors[0], colors[1], colors[2]]
                } else {
                    [1.0, 1.0, 1.0]
                };
                MeshVertex {
                    pos: [pos[0], pos[1], pos[2]],
                    normal,
                    color,
                }
            })
            .collect::<Vec<MeshVertex>>();

        println!(
            "Loaded {} vertices and {} indices from {}",
            vertices.len(),
            indices.len(),
            path
        );

        // Create vertex buffer
        let vertex_buffer_size =
            (std::mem::size_of::<MeshVertex>() * vertices.len()) as vk::DeviceSize;

        let (vertex_buffer, vertex_buffer_memory) = self.create_buffer(
            vertex_buffer_size,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            // just for now, we can use HOST_VISIBLE | HOST_COHERENT,
            // but ideally we would use DEVICE_LOCAL and do a staging buffer
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );

        // Upload to buffers
        let vertex_buffer_data = vertices.as_slice();

        self.upload_to_buffer(vertex_buffer_memory, vertex_buffer_data);

        Mesh {
            vertex_buffer_memory: vertex_buffer_memory,
            vertex_buffer: vertex_buffer,
            index_buffer_memory: vk::DeviceMemory::null(),
            index_buffer: vk::Buffer::null(),
            device: self.device.clone(),
            index_count: indices.len() as u32,
        }
    }
}

impl Drop for Mesh {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_buffer(self.vertex_buffer, None);
            self.device.free_memory(self.vertex_buffer_memory, None);
            if self.index_buffer != vk::Buffer::null() {
                self.device.destroy_buffer(self.index_buffer, None);
                self.device.free_memory(self.index_buffer_memory, None);
            }
        }
    }
}