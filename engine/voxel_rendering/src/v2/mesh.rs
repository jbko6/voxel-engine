use std::sync::Arc;

use ash::vk;
use vk_mem::Alloc;
use glam::Vec3;

use crate::{v2::{Renderer, GPUScenario, RenderingContext, ScenarioHandle}};

#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct GPUMesh {
    pub verts: vk::DeviceAddress,
    pub indices: vk::DeviceAddress, // 0x0 if absent
    pub index_count: u32, // 0 if absent
    pub normals: vk::DeviceAddress, // 0x0 if absent
    pub normal_count: u32, // 0 if absent
    pub colors: vk::DeviceAddress, // 0x0 if absent
    pub color_count: u32, // 0 if absent
}

pub(crate) struct Mesh {
    context: Arc<RenderingContext>,
    pub(crate) vertex_count: u32,
    pub(crate) index_count: u32,
    pub(crate) vertex_buffer: vk::Buffer,
    pub(crate) vertex_allocation: vk_mem::Allocation,
    pub(crate) index_buffer: Option<vk::Buffer>,
    pub(crate) index_allocation: Option<vk_mem::Allocation>,
    pub(crate) normal_buffer: Option<vk::Buffer>,
    pub(crate) normal_allocation: Option<vk_mem::Allocation>,
    pub(crate) color_buffer: Option<vk::Buffer>,
    pub(crate) color_allocation: Option<vk_mem::Allocation>,
}

impl Drop for Mesh {
    fn drop(&mut self) {
        unsafe {
            self.context.allocator().destroy_buffer(self.vertex_buffer, &mut self.vertex_allocation);
            if let Some(index_buffer) = self.index_buffer {
                if let Some(mut index_allocation) = self.index_allocation.take() {
                    self.context.allocator().destroy_buffer(index_buffer, &mut index_allocation);
                }
            }
            if let Some(normal_buffer) = self.normal_buffer {
                if let Some(mut normal_allocation) = self.normal_allocation.take() {
                    self.context.allocator().destroy_buffer(normal_buffer, &mut normal_allocation);
                }
            }
            if let Some(color_buffer) = self.color_buffer {
                if let Some(mut color_allocation) = self.color_allocation.take() {
                    self.context.allocator().destroy_buffer(color_buffer, &mut color_allocation);
                }
            }
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

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut normals = Vec::new();
        let mut colors = Vec::new();

        for model in models {
            let mesh = model.mesh;

            for i in 0..mesh.positions.len() / 3 {
                vertices.push([
                    mesh.positions[i * 3],
                    mesh.positions[i * 3 + 1],
                    mesh.positions[i * 3 + 2],
                ]);
            }

            indices.extend(mesh.indices.iter().map(|&i| i as u32));

            if !mesh.normals.is_empty() {
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

            if !mesh.vertex_color.is_empty() {
                for i in 0..mesh.vertex_color.len() / 3 {
                    colors.push([
                        mesh.vertex_color[i * 3],
                        mesh.vertex_color[i * 3 + 1],
                        mesh.vertex_color[i * 3 + 2]
                    ]);
                }
            }
        }

        self.create_mesh(scenario_handle, &vertices, Some(&indices), Some(&normals), Some(&colors))
    }

    fn create_buffer<T>(&self, data: &[T]) -> Option<(vk::Buffer, vk_mem::Allocation)> {
        if data.is_empty() {
            return None;
        }

        println!("Creating buffer of size {} bytes storing {} items each of size {}", std::mem::size_of_val(data), data.len(), std::mem::size_of::<T>());
        let buffer_info = vk::BufferCreateInfo::default()
            .size((std::mem::size_of_val(data)) as u64)
            .usage(vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS);

        let create_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::Auto,
            flags: vk_mem::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE,
            ..Default::default()
        };

        let (buffer, mut allocation) = unsafe {
            self.context.allocator().create_buffer(&buffer_info, &create_info).unwrap()
        };

        // Map memory and copy data
        unsafe {
            let mapped_ptr = self.context.allocator().map_memory(&mut allocation).unwrap();
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                mapped_ptr as *mut T,
                data.len()
            );
            self.context.allocator().unmap_memory(&mut allocation);
        }
        Some((buffer, allocation))
    }

    pub fn create_mesh(
        &mut self,
        scenario_handle: ScenarioHandle,
        vertices: &[[f32; 3]],
        indices: Option<&[u32]>,
        normals: Option<&[[f32; 3]]>,
        colors: Option<&[[f32; 3]]>,
    ) -> MeshHandle {
        let (vertex_buffer, vertex_allocation) = self.create_buffer(vertices).unwrap();
        let (index_buffer, index_allocation) = indices.map(|idx| self.create_buffer(idx)).flatten().unzip();
        let (normal_buffer, normal_allocation) = normals.map(|n| self.create_buffer(n)).flatten().unzip();
        let (color_buffer, color_allocation) = colors.map(|c| self.create_buffer(c)).flatten().unzip();

        let mesh = Mesh {
            context: self.context.clone(),
            vertex_count: vertices.len() as u32,
            index_count: indices.map_or(0, |idx| idx.len() as u32),
            vertex_buffer,
            vertex_allocation,
            index_buffer,
            index_allocation,
            normal_buffer,
            normal_allocation,
            color_buffer,
            color_allocation,
        };

        let scenario = &mut self.scenarios[scenario_handle.index];
        scenario.meshes.push(mesh);

        // Figure out addresses
        let verts = unsafe {
            self.context.device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(vertex_buffer))
        };
        let (indices, index_count) = if let Some(idx_buf) = index_buffer {
            let addr = unsafe { self.context.device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(idx_buf)) };
            (addr, indices.unwrap().len() as u32)
        } else {
            (0x0, 0)
        };
        let (normals, normal_count) = if let Some(norm_buf) = normal_buffer {
            let addr = unsafe { self.context.device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(norm_buf)) };
            (addr, normals.unwrap().len() as u32)
        } else {
            (0x0, 0)
        };
        let (colors, color_count) = if let Some(col_buf) = color_buffer {
            let addr = unsafe { self.context.device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(col_buf)) };
            (addr, colors.unwrap().len() as u32)
        } else {
            (0x0, 0)
        };

        let gpu_mesh = GPUMesh {
            verts,
            indices,
            index_count,
            normals,
            normal_count,
            colors,
            color_count,
        };

        unsafe {
            let ptr = scenario.context.allocator().map_memory(&mut scenario.scenario_allocation).unwrap();
            let data = ptr as *mut GPUScenario;
            (*data).meshes[scenario.meshes.len() - 1] = gpu_mesh;
            scenario.context.allocator().unmap_memory(&mut scenario.scenario_allocation);
        }

        MeshHandle { index: scenario.meshes.len() - 1 }
    }
}
