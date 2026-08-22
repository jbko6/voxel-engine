use std::sync::Arc;

use ash::vk;
use vk_mem::Alloc;
use glam::Vec3;

use crate::v2::{BufferType, GPUScenario, Renderer, RenderingContext, ScenarioHandle};

#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct GPUMesh {
    pub vertex_offset: u32,
    pub normal_offset: u32,
    pub normals_present: u32,
    pub color_offset: u32,
    pub colors_present: u32,
}

pub(crate) struct Mesh {
    pub(crate) index_count: u32,
    pub(crate) vertex_alloc: vk_mem::VirtualAllocation,
    pub(crate) vertex_offset: u64,
    pub(crate) index_alloc: vk_mem::VirtualAllocation,
    pub(crate) index_offset: u64,
    pub(crate) normal_alloc: Option<vk_mem::VirtualAllocation>,
    pub(crate) normal_offset: Option<u64>,
    pub(crate) color_alloc: Option<vk_mem::VirtualAllocation>,
    pub(crate) color_offset: Option<u64>,
}

impl Mesh {
    pub(crate) fn gpu_mesh(&self) -> GPUMesh {
        GPUMesh {
            vertex_offset: self.vertex_offset as u32,
            normal_offset: self.normal_offset.unwrap_or(0) as u32,
            normals_present: self.normal_alloc.is_some() as u32,
            color_offset: self.color_offset.unwrap_or(0) as u32,
            colors_present: self.color_alloc.is_some() as u32,
        }
    }
}

impl Drop for Mesh {
    fn drop(&mut self) {
        unsafe {
            
        }
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct MeshHandle {
    pub index: usize,
}

impl Renderer {
    pub fn load_obj(&mut self, scenario_handle: ScenarioHandle, path: &str) -> MeshHandle {
        let (models, _materials) = tobj::load_obj(path, &tobj::LoadOptions::default())
            .expect("Failed to load OBJ file");

        let mesh = &models[0].mesh;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut normals = Vec::new();
        let mut colors = if mesh.vertex_color.is_empty() { None } else { Some(Vec::new()) };

        for i in 0..mesh.positions.len() / 3 {
            vertices.push([
                mesh.positions[i * 3],
                mesh.positions[i * 3 + 1],
                mesh.positions[i * 3 + 2],
            ]);
        }

        indices.extend(mesh.indices.iter().map(|&i| i as u32));

        if mesh.normals.len() == mesh.positions.len() {
            for i in 0..mesh.normals.len() / 3 {
                normals.push([
                    mesh.normals[i * 3],
                    mesh.normals[i * 3 + 1],
                    mesh.normals[i * 3 + 2],
                ]);
            }
        } else {
            println!("No normals in the file for this model — generating smooth per-vertex normals via face-normal accumulation.");
            // No normals in the file for this model — generate smooth
            // per-vertex normals via face-normal accumulation.
            let vertex_count = mesh.positions.len() / 3;
            let mut generated = vec![Vec3::ZERO; vertex_count];

            for tri in mesh.indices.chunks_exact(3) {
                let i0 = tri[0] as usize;
                let i1 = tri[1] as usize;
                let i2 = tri[2] as usize;

                // Get the positions of the three vertices of the triangle.
                let p0 = Vec3::from(vertices[i0]);
                let p1 = Vec3::from(vertices[i1]);
                let p2 = Vec3::from(vertices[i2]);

                // Unnormalized face normal — its length is proportional to
                // triangle area, so larger triangles contribute more weight
                // to the vertices they share. This is standard practice for
                // smooth-normal generation (area-weighted averaging).
                let face_normal = (p1 - p0).cross(p2 - p0);

                generated[i0] += face_normal;
                generated[i1] += face_normal;
                generated[i2] += face_normal;
            }

            for n in generated {
                let normalized = if n.length_squared() > 0.0 {
                    n.normalize()
                } else {
                    // Degenerate case: isolated vertex or all adjacent
                    // triangles were zero-area. Fall back to a sane default
                    // rather than propagating a NaN from normalizing zero.
                    Vec3::Y
                };
                normals.push([normalized.x, normalized.y, normalized.z]);
            }
        }

        if let Some(colors) = &mut colors {
            for i in 0..mesh.vertex_color.len() / 3 {
                colors.push([
                    mesh.vertex_color[i * 3],
                    mesh.vertex_color[i * 3 + 1],
                    mesh.vertex_color[i * 3 + 2]
                ]);
            }
        }

        self.create_mesh(scenario_handle, &vertices, &indices, Some(&normals), colors.as_deref())
    }

    pub fn create_mesh(
        &mut self,
        scenario_handle: ScenarioHandle,
        vertices: &[[f32; 3]],
        indices: &[u32],
        normals: Option<&[[f32; 3]]>,
        colors: Option<&[[f32; 3]]>,
    ) -> MeshHandle {
        let global_buffer = &mut self.global_buffer;
        let allocator = self.context.allocator();
        let (vertex_alloc, vertex_offset) = global_buffer.init_buffer(allocator, BufferType::VERTEX, vertices);
        let (index_alloc, index_offset) = global_buffer.init_buffer(allocator, BufferType::INDEX, indices);
        let (normal_alloc, normal_offset) = normals.map(|n| global_buffer.init_buffer(allocator, BufferType::NORMAL, n)).unzip();
        let (color_alloc, color_offset) = colors.map(|c| global_buffer.init_buffer(allocator, BufferType::COLOR, c)).unzip();

        let mesh = Mesh {
            index_count: indices.len() as u32,
            vertex_alloc,
            vertex_offset: vertex_offset / std::mem::size_of::<[f32; 3]>() as u64,
            index_alloc,
            index_offset: index_offset / std::mem::size_of::<u32>() as u64,
            normal_alloc,
            normal_offset: normal_offset.map(|o| o / std::mem::size_of::<[f32; 3]>() as u64),
            color_alloc,
            color_offset: color_offset.map(|o| o / std::mem::size_of::<[f32; 3]>() as u64),
        };

        let scenario = &mut self.scenarios[scenario_handle.index];

        let index = scenario.add_mesh(mesh);

        MeshHandle { index }
    }
}
