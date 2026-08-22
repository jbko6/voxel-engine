use crate::v2::{GPUScenario, MeshHandle, Renderer, Scenario, ScenarioHandle};

#[derive(Clone, Copy)]
#[repr(C, align(16))]
pub(crate) struct GPUInstance {
    pub transform: glam::Mat4,
    pub mesh_idx: u32,
}

pub(crate) struct Instance {
    pub mesh_idx: u32,
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct InstanceHandle {
    pub index: usize,
}

impl Renderer {

    pub fn create_instance(&mut self, scenario_handle: ScenarioHandle, transform: glam::Mat4, mesh_handle: MeshHandle) -> InstanceHandle {
        let scenario = &mut self.scenarios[scenario_handle.index];
        unsafe {
            let ptr = self.context.allocator().map_memory(&mut scenario.scenario_allocation).unwrap();
            let data = ptr as *mut GPUScenario;
            (*data).instances[scenario.instances.len()] = GPUInstance {
                transform,
                mesh_idx: mesh_handle.index as u32,
            };
            scenario.instances.push(Instance { mesh_idx: mesh_handle.index as u32 });
            scenario.context.allocator().unmap_memory(&mut scenario.scenario_allocation);
        }
        InstanceHandle { index: scenario.instances.len() - 1 }
    }

}