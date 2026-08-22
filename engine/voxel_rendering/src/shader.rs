use std::sync::Arc;

use ash::vk;

pub struct Shader {
    device: Arc<ash::Device>,
    module: vk::ShaderModule,
}

impl Shader {
    pub fn new(device: Arc<ash::Device>, path: &str) -> Self {
        let shader_bytes = std::fs::read(path).expect("Failed to read shader file");
        let shader_code = ash::util::read_spv(&mut std::io::Cursor::new(shader_bytes))
            .expect("Failed to read shader SPIR-V");

        let shader_create_info = vk::ShaderModuleCreateInfo::default().code(&shader_code);

        let shader_module = unsafe {
            device
                .create_shader_module(&shader_create_info, None)
                .expect("Failed to create shader module")
        };

        Shader {
            device,
            module: shader_module,
        }
    }

    pub fn module(&self) -> vk::ShaderModule {
        self.module
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_shader_module(self.module, None);
        }
    }
}
