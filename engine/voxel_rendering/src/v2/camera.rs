use glam::{Mat4, camera::rh::proj::vulkan::perspective};

use crate::v2::{GPUScenario, Renderer, ScenarioHandle};

/// GPU representation of a camera
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub(crate) struct GPUCamera {
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
}

/// Camera for client to use
pub struct Camera {
    transform: glam::Mat4,
    fov: f32,
    near_plane: f32,
    far_plane: f32,
}

impl Camera {
    pub fn new(fov_degrees: f32, near_plane: f32, far_plane: f32) -> Self {
        Camera {
            transform: glam::Mat4::IDENTITY,
            fov: fov_degrees.to_radians(),
            near_plane,
            far_plane,
        }
    }

    pub fn set_transform(&mut self, transform: glam::Mat4) {
        self.transform = transform;
    }

    pub fn get_transform(&self) -> &glam::Mat4 {
        &self.transform
    }

    pub fn get_view_matrix(&self) -> glam::Mat4 {
        self.transform.try_inverse().unwrap_or(glam::Mat4::IDENTITY)
    }

    pub fn get_projection_matrix(&self, aspect_ratio: f32) -> glam::Mat4 {
        perspective(self.fov, aspect_ratio, self.near_plane, self.far_plane)
    }

    pub(crate) fn get_gpu_camera(&self, aspect_ratio: f32) -> GPUCamera {
        let view_matrix = self.get_view_matrix();
        let projection_matrix = self.get_projection_matrix(aspect_ratio);

        GPUCamera {
            view_matrix,
            projection_matrix,
        }
    }
}

impl Renderer  {
    pub fn update_camera(&mut self, scenario: ScenarioHandle, camera: &Camera, aspect_ratio: f32) {
        let scenario = &mut self.scenarios[scenario.index];
        let gpu_camera = camera.get_gpu_camera(aspect_ratio);
        scenario.update_camera(gpu_camera);
    }
}