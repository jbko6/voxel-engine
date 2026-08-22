use std::sync::Arc;

use vk_mem::{Alloc};
use ash::{vk};

use crate::v2::{Frame, GPUCamera, GPUInstance, Instance, GPUMesh, Mesh, Renderer, RenderingContext};

pub const MAX_INSTANCES: usize = 1000;
pub const MAX_MESHES: usize = 1000;

#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct GPUScenario {
    pub instances: [GPUInstance; MAX_INSTANCES],
    pub meshes: [GPUMesh; MAX_MESHES],
}

pub struct Scenario {
    // later: environment, lighting
    context: Arc<RenderingContext>,
    scenario_allocation: vk_mem::Allocation,
    scenario_buffer: vk::Buffer,
    pub(crate) camera: GPUCamera,
    instances: Vec<Instance>,
    meshes: Vec<Mesh>,
}

#[derive(Clone, Copy)]
pub struct ScenarioHandle {
    pub index: usize,
}

impl Drop for Scenario {
    fn drop(&mut self) {
        unsafe {
            self.meshes.clear();
            self.instances.clear();
            self.context.allocator().destroy_buffer(self.scenario_buffer, &mut self.scenario_allocation);
        }
    }
}

impl Renderer {
    pub fn create_scenario(&mut self) -> ScenarioHandle {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(std::mem::size_of::<GPUScenario>() as u64)
            .usage(vk::BufferUsageFlags::STORAGE_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let create_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::Auto,
            flags: vk_mem::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE,
            ..Default::default()
        };

        let (buffer, allocation) = unsafe {
            self.context.allocator().create_buffer(&buffer_info, &create_info).unwrap()
        };
        
        let scenario = Scenario {
            context: self.context.clone(),
            scenario_allocation: allocation,
            scenario_buffer: buffer,
            camera: GPUCamera::default(),
            instances: Vec::new(),
            meshes: Vec::new(),
        };

        self.scenarios.push(scenario);

        ScenarioHandle { index: self.scenarios.len() - 1 }
    }
}

impl Scenario {
    pub(crate) fn update_camera(&mut self, camera: GPUCamera) {
        self.camera = camera;
    }

    pub(crate) fn add_instance(&mut self, instance: Instance) -> usize {
        let gpu_instance = instance.gpu_instance();

        unsafe {
            let ptr = self.context.allocator().map_memory(&mut self.scenario_allocation).unwrap();
            let data = ptr as *mut GPUScenario;
            (*data).instances[self.instances.len()] = gpu_instance;
            self.context.allocator().unmap_memory(&mut self.scenario_allocation);
            self.context.allocator().flush_allocation(&mut self.scenario_allocation, 0, vk::WHOLE_SIZE).unwrap();
        }

        self.instances.push(instance);
        self.instances.len() - 1
    }

    pub(crate) fn add_mesh(&mut self, mesh: Mesh) -> usize {
        let gpu_mesh = mesh.gpu_mesh();

        unsafe {
            let ptr = self.context.allocator().map_memory(&mut self.scenario_allocation).unwrap();
            let data = ptr as *mut GPUScenario;
            (*data).meshes[self.meshes.len()] = gpu_mesh;
            self.context.allocator().unmap_memory(&mut self.scenario_allocation);
            self.context.allocator().flush_allocation(&mut self.scenario_allocation, 0, vk::WHOLE_SIZE).unwrap();
        }

        self.meshes.push(mesh);
        self.meshes.len() - 1
    }

    pub(crate) fn buffer(&self) -> vk::Buffer {
        self.scenario_buffer
    }

    pub(crate) fn collect_draw_calls(&self) -> (Vec<vk::DrawIndirectCommand>, Vec<vk::DrawIndexedIndirectCommand>) {
        let mut indexed_draw_calls = Vec::new();
        let mut draw_calls = Vec::new();

        for (i, instance) in self.instances.iter().enumerate() {
            let mesh = &self.meshes[instance.mesh_idx as usize];
            // println!("Instance {} uses mesh {} with index count {}, vertex offset: {}, index offset: {}", i, instance.mesh_idx, mesh.index_count, mesh.vertex_offset, mesh.index_offset);
            if mesh.index_offset.is_none() {
                draw_calls.push(vk::DrawIndirectCommand {
                    vertex_count: mesh.vertex_count,
                    instance_count: 1,
                    first_vertex: mesh.vertex_offset as u32,
                    first_instance: i as u32,
                });
            } else {
                indexed_draw_calls.push(vk::DrawIndexedIndirectCommand {
                    index_count: mesh.index_count,
                    instance_count: 1,
                    first_index: mesh.index_offset.unwrap() as u32,
                    vertex_offset: mesh.vertex_offset as i32,
                    first_instance: i as u32,
                });
            }
        }

        (draw_calls, indexed_draw_calls)
    }
}