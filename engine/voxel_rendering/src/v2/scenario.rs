use std::sync::Arc;

use vk_mem::{Alloc};
use ash::{vk};

use crate::v2::{Frame, GPUCamera, GPUInstance, Instance, GPUMesh, Mesh, Renderer, RenderingContext};

pub const MAX_INSTANCES: usize = 1000;
pub const MAX_MESHES: usize = 1000;

#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct GPUScenario {
    pub camera: GPUCamera,
    pub instances: [GPUInstance; MAX_INSTANCES],
    pub meshes: [GPUMesh; MAX_MESHES],
}

pub struct Scenario {
    // later: environment, lighting
    pub(crate) context: Arc<RenderingContext>,
    pub(crate) scenario_allocation: vk_mem::Allocation,
    pub(crate) scenario_buffer: vk::Buffer,
    pub(crate) instances: Vec<Instance>,
    pub(crate) meshes: Vec<Mesh>,
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
            instances: Vec::new(),
            meshes: Vec::new(),
        };

        self.scenarios.push(scenario);

        ScenarioHandle { index: self.scenarios.len() - 1 }
    }
}

impl Scenario {
    pub(crate) fn collect_draw_calls(&self) -> Vec<vk::DrawIndirectCommand> {
        let mut draw_calls = Vec::new();

        for (i, instance) in self.instances.iter().enumerate() {
            let mesh = &self.meshes[instance.mesh_idx as usize];
            if mesh.index_count > 0 {
                draw_calls.push(vk::DrawIndirectCommand {
                    vertex_count: mesh.index_count,
                    instance_count: 1,
                    first_vertex: 0,
                    first_instance: i as u32,
                });
            } else {
                draw_calls.push(vk::DrawIndirectCommand {
                    vertex_count: mesh.vertex_count,
                    instance_count: 1,
                    first_vertex: 0,
                    first_instance: i as u32,
                });
            }
        }

        draw_calls
    }
}