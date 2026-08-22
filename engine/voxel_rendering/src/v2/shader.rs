#[repr(C)]
pub struct GPUShader {
    pub module: vk::ShaderModule,
}

use std::sync::Arc;

use ash::vk;

use crate::v2::RenderingContext;

pub(crate) struct Shader {
    context: Arc<RenderingContext>,
    module: vk::ShaderModule,
}

impl Shader {
    pub fn new(context: Arc<RenderingContext>, path: &str) -> Self {
        let shader_bytes = std::fs::read(path).expect("Failed to read shader file");
        let shader_code = ash::util::read_spv(&mut std::io::Cursor::new(shader_bytes))
            .expect("Failed to read shader SPIR-V");

        let shader_create_info = vk::ShaderModuleCreateInfo::default().code(&shader_code);

        let shader_module = unsafe {
            context
                .device
                .create_shader_module(&shader_create_info, None)
                .expect("Failed to create shader module")
        };

        println!("Shader module created");

        Shader {
            context,
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
            self.context.device.destroy_shader_module(self.module, None);
        }
    }
}
