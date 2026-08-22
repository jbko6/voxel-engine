use crate::v2::{GPUScenario, MeshHandle, Renderer, Scenario, ScenarioHandle};

#[derive(Clone, Copy)]
#[repr(C, align(16))]
pub(crate) struct GPUInstance {
    pub transform: glam::Mat4,
    pub normal_matrix: glam::Mat4,
    pub mesh_idx: u32,
}

pub(crate) struct Instance {
    pub transform: glam::Mat4,
    pub mesh_idx: u32,
}

impl Instance {
    pub(crate) fn gpu_instance(&self) -> GPUInstance {
        GPUInstance {
            transform: self.transform,
            normal_matrix: self.transform.inverse().transpose(),
            mesh_idx: self.mesh_idx,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct InstanceHandle {
    pub index: usize,
}

impl Renderer {

    pub fn create_instance(&mut self, scenario_handle: ScenarioHandle, transform: glam::Mat4, mesh_handle: MeshHandle) -> InstanceHandle {
        let scenario = &mut self.scenarios[scenario_handle.index];
        let instance = Instance { transform, mesh_idx: mesh_handle.index as u32 };
        let index = scenario.add_instance(instance);
        InstanceHandle { index }
    }

}