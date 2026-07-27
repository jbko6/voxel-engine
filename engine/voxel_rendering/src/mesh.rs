use vulkano::{buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage}, memory::allocator::{AllocationCreateInfo, DeviceLayout, MemoryTypeFilter}, pipeline::graphics::vertex_input::Vertex};
use vulkano_taskgraph::{Id, resource::HostAccessType};
use std::hash::Hash;

use crate::Renderer;

#[derive(Debug, Clone, Copy, BufferContents, Vertex, PartialEq)]
#[repr(C)]
pub struct MeshVertex {
    #[format(R32G32B32_SFLOAT)]
    #[name("position")]
    pos: [f32; 3],
    #[format(R32G32B32_SFLOAT)]
    normal: [f32; 3],
    #[format(R32G32B32_SFLOAT)]
    color: [f32; 3]
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Mesh {
    pub vertex_buffer_id: Id<Buffer>,
    pub index_buffer_id: Id<Buffer>,
    pub index_count: u32,
}

impl Renderer {
    pub fn load_obj(&self, path: &str) -> Mesh {
        let (models, _) = tobj::load_obj(path, &tobj::LoadOptions::default()).expect("Failed to load OBJ file");

        println!("Loaded {} models from {}", models.len(), path);

        if models.len() == 0 {
            panic!("No models found in OBJ file: {}", path);
        }

        let model = &models[0];

        let mesh = &model.mesh;

        let indices = mesh.indices.clone();

        let normals_present = mesh.normals.len() > 0;
        let colors_present = mesh.vertex_color.len() > 0;

        let vertices = mesh.positions.chunks(3).enumerate().map(|(i, pos)| {
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
            MeshVertex { pos: [pos[0], pos[1], pos[2]], normal, color }
        }).collect::<Vec<MeshVertex>>();

        println!("Loaded {} vertices and {} indices from {}", vertices.len(), indices.len(), path);

        let vertex_buffer_id = self
            .resources
            .create_buffer(
                &BufferCreateInfo {
                    // We are going to bind this buffer as a vertex buffer.
                    usage: BufferUsage::VERTEX_BUFFER,
                    ..Default::default()
                },
                &AllocationCreateInfo {
                    // We want the buffer to be located on the device (GPU) so it is fast to access
                    // from shaders. It must also be writable from the host side (CPU) to initially
                    // upload the data.
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                // The device layout determines the size and alignment of the buffer.
                DeviceLayout::for_value(vertices.as_slice()).unwrap(),
            )
            .unwrap();

        let index_buffer_id = self
            .resources
            .create_buffer(
                &BufferCreateInfo {
                    // We are going to bind this buffer as a index buffer.
                    usage: BufferUsage::INDEX_BUFFER,
                    ..Default::default()
                },
                &AllocationCreateInfo {
                    // We want the buffer to be located on the device (GPU) so it is fast to access
                    // from shaders. It must also be writable from the host side (CPU) to initially
                    // upload the data.
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                // The device layout determines the size and alignment of the buffer.
                DeviceLayout::for_value(indices.as_slice()).unwrap(),
            )
            .unwrap();

        // Upload to buffers
        unsafe {
            vulkano_taskgraph::execute(
                &self.queue,
                &self.resources,
                self.flight_id,
                |_cbf, tcx| {
                    tcx.try_write_buffer::<[MeshVertex]>(vertex_buffer_id, ..)?
                        .copy_from_slice(&vertices);

                    tcx.try_write_buffer::<[u32]>(index_buffer_id, ..)?
                        .copy_from_slice(&indices);

                    Ok(())
                },
                [(vertex_buffer_id, HostAccessType::Write),
                                        (index_buffer_id, HostAccessType::Write)],
                [],
                [],
            )
        }
        .unwrap();

        Mesh {
            vertex_buffer_id,
            index_buffer_id,
            index_count: indices.len() as u32,
        }
    }
}