use ash::vk;
use std::sync::Arc;

use crate::{Shader};

#[derive(Clone)]
pub struct PipelineBuilder<'a> {
    device: Arc<ash::Device>,
    render_pass: vk::RenderPass,
    shader_stages: Vec<vk::PipelineShaderStageCreateInfo<'a>>,
    push_constant_ranges: Vec<vk::PushConstantRange>,
    descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    vertex_input_state: vk::PipelineVertexInputStateCreateInfo<'a>,
    input_assembly_state: vk::PipelineInputAssemblyStateCreateInfo<'a>,
    viewport_state: vk::PipelineViewportStateCreateInfo<'a>,
    rasterization_state: vk::PipelineRasterizationStateCreateInfo<'a>,
    multisample_state: vk::PipelineMultisampleStateCreateInfo<'a>,
    depth_stencil_state: vk::PipelineDepthStencilStateCreateInfo<'a>,
    color_blend_state: vk::PipelineColorBlendStateCreateInfo<'a>,
    dynamic_state: vk::PipelineDynamicStateCreateInfo<'a>,
}

impl<'a> PipelineBuilder<'a> {
    pub fn new(device: Arc<ash::Device>, render_pass: vk::RenderPass) -> Self {
        PipelineBuilder {
            device,
            render_pass,
            push_constant_ranges: Vec::new(),
            descriptor_set_layouts: Vec::new(),
            shader_stages: Vec::new(),
            vertex_input_state: vk::PipelineVertexInputStateCreateInfo::default(),
            input_assembly_state: vk::PipelineInputAssemblyStateCreateInfo::default(),
            viewport_state: vk::PipelineViewportStateCreateInfo::default(),
            rasterization_state: vk::PipelineRasterizationStateCreateInfo::default(),
            multisample_state: vk::PipelineMultisampleStateCreateInfo::default(),
            depth_stencil_state: vk::PipelineDepthStencilStateCreateInfo::default(),
            color_blend_state: vk::PipelineColorBlendStateCreateInfo::default(),
            dynamic_state: vk::PipelineDynamicStateCreateInfo::default(),
        }
    }

    pub fn build(
        &self
    ) -> Pipeline {
        let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::default()
            .push_constant_ranges(&self.push_constant_ranges)
            .set_layouts(&self.descriptor_set_layouts);

        let pipeline_layout = unsafe {
            self.device
                .create_pipeline_layout(&pipeline_layout_create_info, None)
                .expect("Failed to create pipeline layout")
        };

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&self.shader_stages)
            .vertex_input_state(&self.vertex_input_state)
            .input_assembly_state(&self.input_assembly_state)
            .viewport_state(&self.viewport_state)
            .rasterization_state(&self.rasterization_state)
            .multisample_state(&self.multisample_state)
            .depth_stencil_state(&self.depth_stencil_state)
            .color_blend_state(&self.color_blend_state)
            .dynamic_state(&self.dynamic_state)
            .layout(pipeline_layout)
            .render_pass(self.render_pass);

        let pipeline = unsafe {
            self.device
                .create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    &[pipeline_info],
                    None,
                )
                .expect("Failed to create graphics pipeline")
        };

        // Assuming only one pipeline is created, we can return the first one
        let pipeline = pipeline[0];

        Pipeline {
            device: self.device.clone(),
            pipeline,
            pipeline_layout,
        }
    }

    pub fn add_shader_stage(&mut self, shader: &Shader, stage: vk::ShaderStageFlags) -> &mut Self {
        self.shader_stages.push(
            vk::PipelineShaderStageCreateInfo::default()
                .stage(stage)
                .module(shader.module())
                .name(std::ffi::CStr::from_bytes_with_nul(b"main\0").unwrap()),
        );
        self
    }

    pub fn add_push_constant_range(&mut self, push_constant_range: vk::PushConstantRange) -> &mut Self {
        self.push_constant_ranges.push(push_constant_range);
        self
    }

    pub fn add_descriptor_set_layout(&mut self, descriptor_set_layout: vk::DescriptorSetLayout) -> &mut Self {
        self.descriptor_set_layouts.push(descriptor_set_layout);
        self
    }

    pub fn vertex_input_state(&mut self, vertex_input_state: vk::PipelineVertexInputStateCreateInfo<'a>) -> &mut Self {
        self.vertex_input_state = vertex_input_state;
        self
    }

    pub fn input_assembly_state(&mut self, input_assembly_state: vk::PipelineInputAssemblyStateCreateInfo<'a>) -> &mut Self {
        self.input_assembly_state = input_assembly_state;
        self
    }

    pub fn viewport_state(&mut self, viewport_state: vk::PipelineViewportStateCreateInfo<'a>) -> &mut Self {
        self.viewport_state = viewport_state;
        self
    }

    pub fn rasterization_state(&mut self, rasterization_state: vk::PipelineRasterizationStateCreateInfo<'a>) -> &mut Self {
        self.rasterization_state = rasterization_state;
        self
    }

    pub fn multisample_state(&mut self, multisample_state: vk::PipelineMultisampleStateCreateInfo<'a>) -> &mut Self {
        self.multisample_state = multisample_state;
        self
    }

    pub fn depth_stencil_state(&mut self, depth_stencil_state: vk::PipelineDepthStencilStateCreateInfo<'a>) -> &mut Self {
        self.depth_stencil_state = depth_stencil_state;
        self
    }

    pub fn color_blend_state(&mut self, color_blend_state: vk::PipelineColorBlendStateCreateInfo<'a>) -> &mut Self {
        self.color_blend_state = color_blend_state;
        self
    }

    pub fn dynamic_state(&mut self, dynamic_states: &'a [vk::DynamicState]) -> &mut Self {
        self.dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(dynamic_states);
        self
    }
}

pub struct Pipeline {
    device: Arc<ash::Device>,
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
}

impl Pipeline {
    pub fn get(&self) -> vk::Pipeline {
        self.pipeline
    }

    pub fn layout(&self) -> vk::PipelineLayout {
        self.pipeline_layout
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}