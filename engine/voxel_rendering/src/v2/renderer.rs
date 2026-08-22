use std::sync::Arc;

use ash::{khr::synchronization2, vk};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use vk_mem::Alloc;

use crate::v2::{
    BufferType, FrameData, GPUCamera, GPUScenario, GlobalBuffer, Image, Pipeline, RenderingContext,
    Scenario, Shader, Swapchain,
};

const FRAMES_IN_FLIGHT: usize = 2;

pub struct Renderer {
    pub(crate) context: Arc<RenderingContext>,
    width: u32,
    height: u32,
    frame_data: Option<[FrameData; FRAMES_IN_FLIGHT]>,
    current_frame: usize,
    surface: vk::SurfaceKHR,
    swapchain: Option<Swapchain>,
    pub(crate) global_buffer: GlobalBuffer,
    pub(crate) scenarios: Vec<Scenario>,
    pipeline: Option<Pipeline>, // TODO: add pipeline management
    frame_index: usize,
    frame_start_time: std::time::Instant,
}

impl Renderer {
    pub fn init(window: sdl3::video::Window) -> Self {
        let context = Arc::new(RenderingContext::init(window.clone()));
        let width = window.size().0;
        let height = window.size().1;

        let frame_data = [
            FrameData::init(context.clone()),
            FrameData::init(context.clone()),
        ];

        let surface = unsafe {
            ash_window::create_surface(
                &context.entry,
                &context.instance,
                window.display_handle().unwrap().as_raw(),
                window.window_handle().unwrap().as_raw(),
                None,
            )
            .unwrap()
        };

        let swapchain = Swapchain::init(context.clone(), surface, width, height).ok();

        println!(
            "Renderer initialized with width: {}, height: {}",
            width, height
        );

        let pipeline = Pipeline::init(
            context.clone(),
            swapchain
                .as_ref()
                .map_or(vk::Format::B8G8R8A8_UNORM, |sc| sc.format),
            Shader::new(context.clone(), "shaders/vertex.vert.spv"),
            Shader::new(context.clone(), "shaders/fragment.frag.spv"),
        );

        let global_buffer = GlobalBuffer::new(context.allocator());

        println!(
            "Renderer initialization complete. Swapchain format: {:?}",
            swapchain.as_ref().map(|sc| sc.format)
        );

        Renderer {
            context,
            width,
            height,
            frame_data: Some(frame_data),
            current_frame: 0,
            surface,
            swapchain,
            pipeline: Some(pipeline),
            global_buffer: global_buffer,
            scenarios: Vec::new(),
            frame_index: 0,
            frame_start_time: std::time::Instant::now(),
        }
    }

    pub(crate) fn current_frame(&self) -> &FrameData {
        &self.frame_data.as_ref().unwrap()[self.current_frame]
    }

    fn recreate_swapchain(&mut self) {
        if self.width == 0 || self.height == 0 {
            return;
        }

        self.swapchain = None;
        self.swapchain =
            Swapchain::init(self.context.clone(), self.surface, self.width, self.height).ok();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.recreate_swapchain();
    }

    pub fn draw(&mut self) {
        if let Some(mut frame) = self.begin_frame() {
            unsafe {
                frame.renderer.context.device.cmd_bind_pipeline(
                    frame.renderer.current_frame().command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    frame.renderer.pipeline.as_ref().unwrap().pipeline,
                );
            }
            let width = frame.renderer.swapchain.as_ref().unwrap().extent.width as f32;
            let height = frame.renderer.swapchain.as_ref().unwrap().extent.height as f32;
            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: width,
                height: height,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            let scissor = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: frame.renderer.swapchain.as_ref().unwrap().extent,
            };
            unsafe {
                frame.renderer.context.device.cmd_set_viewport(
                    frame.renderer.current_frame().command_buffer,
                    0,
                    &[viewport],
                );
                frame.renderer.context.device.cmd_set_scissor(
                    frame.renderer.current_frame().command_buffer,
                    0,
                    &[scissor],
                );
            }

            unsafe {
                frame
                    .renderer
                    .context
                    .push_descriptor_device
                    .cmd_push_descriptor_set(
                        frame.renderer.current_frame().command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        frame.renderer.context.pipeline_layout,
                        0,
                        &[
                            vk::WriteDescriptorSet::default()
                                .dst_binding(1)
                                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                                .buffer_info(&[vk::DescriptorBufferInfo {
                                    buffer: frame
                                        .renderer
                                        .global_buffer
                                        .get_buffer(BufferType::VERTEX),
                                    offset: 0,
                                    range: vk::WHOLE_SIZE,
                                }]),
                            vk::WriteDescriptorSet::default()
                                .dst_binding(2)
                                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                                .buffer_info(&[vk::DescriptorBufferInfo {
                                    buffer: frame
                                        .renderer
                                        .global_buffer
                                        .get_buffer(BufferType::NORMAL),
                                    offset: 0,
                                    range: vk::WHOLE_SIZE,
                                }]),
                            vk::WriteDescriptorSet::default()
                                .dst_binding(3)
                                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                                .buffer_info(&[vk::DescriptorBufferInfo {
                                    buffer: frame
                                        .renderer
                                        .global_buffer
                                        .get_buffer(BufferType::COLOR),
                                    offset: 0,
                                    range: vk::WHOLE_SIZE,
                                }]),
                        ],
                    );
            }

            unsafe {
                frame.renderer.context.device.cmd_bind_index_buffer(
                    frame.renderer.current_frame().command_buffer,
                    frame.renderer.global_buffer.get_buffer(BufferType::INDEX),
                    0,
                    vk::IndexType::UINT32,
                );
            }

            for (i, scenario) in frame.renderer.scenarios.iter().enumerate() {
                // Update descriptor sets for the scenario
                unsafe {
                    frame
                        .renderer
                        .context
                        .push_descriptor_device
                        .cmd_push_descriptor_set(
                            frame.renderer.current_frame().command_buffer,
                            vk::PipelineBindPoint::GRAPHICS,
                            frame.renderer.context.pipeline_layout,
                            0, // set number
                            &[vk::WriteDescriptorSet::default()
                                .dst_binding(0)
                                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                                .buffer_info(&[vk::DescriptorBufferInfo {
                                    buffer: scenario.buffer(),
                                    offset: 0,
                                    range: vk::WHOLE_SIZE,
                                }])],
                        );
                }

                unsafe {
                    frame.renderer.context.device.cmd_push_constants(
                        frame.renderer.current_frame().command_buffer,
                        frame.renderer.context.pipeline_layout,
                        vk::ShaderStageFlags::VERTEX,
                        0,
                        std::slice::from_raw_parts(
                            &scenario.camera as *const _ as *const u8,
                            std::mem::size_of::<GPUCamera>(),
                        ),
                    );
                }

                let (draw_calls, indexed_draw_calls) = scenario.collect_draw_calls();

                // Draw non-indexed indirect calls
                unsafe {
                    let indirect_buffer_ptr = frame
                        .renderer
                        .context
                        .allocator()
                        .map_memory(
                            &mut frame.renderer.frame_data.as_mut().unwrap()
                                [frame.renderer.current_frame]
                                .indirect_buffer_allocation,
                        )
                        .unwrap();
                    let indirect_buffer_data =
                        indirect_buffer_ptr as *mut vk::DrawIndirectCommand;
                    std::ptr::copy_nonoverlapping(
                        draw_calls.as_ptr(),
                        indirect_buffer_data,
                        draw_calls.len(),
                    );
                    frame.renderer.context.allocator().unmap_memory(
                        &mut frame.renderer.frame_data.as_mut().unwrap()
                            [frame.renderer.current_frame]
                            .indirect_buffer_allocation,
                    );
                    frame
                        .renderer
                        .context
                        .allocator()
                        .flush_allocation(
                            &mut frame.renderer.frame_data.as_mut().unwrap()
                                [frame.renderer.current_frame]
                                .indirect_buffer_allocation,
                            0,
                            vk::WHOLE_SIZE,
                        )
                        .unwrap();
                    frame.renderer.context.device.cmd_draw_indirect(
                        frame.renderer.current_frame().command_buffer,
                        frame.renderer.current_frame().indirect_buffer,
                        0,
                        draw_calls.len() as u32,
                        std::mem::size_of::<vk::DrawIndirectCommand>() as u32,
                    );
                }
                // Draw indexed indirect calls
                unsafe {
                    let indirect_buffer_ptr = frame
                        .renderer
                        .context
                        .allocator()
                        .map_memory(
                            &mut frame.renderer.frame_data.as_mut().unwrap()
                                [frame.renderer.current_frame]
                                .indirect_buffer_allocation,
                        )
                        .unwrap();
                    let indirect_buffer_data =
                        indirect_buffer_ptr as *mut vk::DrawIndexedIndirectCommand;
                    std::ptr::copy_nonoverlapping(
                        indexed_draw_calls.as_ptr(),
                        indirect_buffer_data,
                        indexed_draw_calls.len(),
                    );
                    frame.renderer.context.allocator().unmap_memory(
                        &mut frame.renderer.frame_data.as_mut().unwrap()
                            [frame.renderer.current_frame]
                            .indirect_buffer_allocation,
                    );
                    frame
                        .renderer
                        .context
                        .allocator()
                        .flush_allocation(
                            &mut frame.renderer.frame_data.as_mut().unwrap()
                                [frame.renderer.current_frame]
                                .indirect_buffer_allocation,
                            0,
                            vk::WHOLE_SIZE,
                        )
                        .unwrap();
                    frame.renderer.context.device.cmd_draw_indexed_indirect(
                        frame.renderer.current_frame().command_buffer,
                        frame.renderer.current_frame().indirect_buffer,
                        0,
                        indexed_draw_calls.len() as u32,
                        std::mem::size_of::<vk::DrawIndexedIndirectCommand>() as u32,
                    );
                }
            }

            frame.end_frame();
        }
    }

    fn begin_frame(&mut self) -> Option<Frame> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        if self.swapchain.is_none() {
            self.recreate_swapchain();
            if self.swapchain.is_none() {
                return None; // still none, skip this frame
            }
        }

        let frame = self.current_frame();

        unsafe {
            self.context
                .device
                .wait_for_fences(&[frame.in_flight_fence], true, u64::MAX)
                .unwrap();
        }

        // Acquire signals image_available_semaphore when the image is actually ready
        let acquire_result = unsafe {
            self.context.swapchain_device.acquire_next_image(
                self.swapchain.as_ref().unwrap().handle,
                u64::MAX,
                frame.image_available_semaphore,
                vk::Fence::null(),
            )
        };

        let image_index = match acquire_result {
            Ok((index, _suboptimal)) => index,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.swapchain = None;
                self.recreate_swapchain();
                return None; // skip this frame, try again next time draw() is called
            }
            Err(e) => panic!("failed to acquire swapchain image: {e:?}"),
        };

        unsafe {
            self.context
                .device
                .reset_fences(&[frame.in_flight_fence])
                .unwrap();
            self.context
                .device
                .reset_command_buffer(frame.command_buffer, vk::CommandBufferResetFlags::empty())
                .unwrap();
            self.context
                .device
                .begin_command_buffer(frame.command_buffer, &vk::CommandBufferBeginInfo::default())
                .unwrap();
        }

        let swap_image = self.swapchain.as_ref().unwrap().images[image_index as usize];
        let swap_view = self.swapchain.as_ref().unwrap().image_views[image_index as usize];

        // UNDEFINED -> COLOR_ATTACHMENT_OPTIMAL
        Self::transition_image(
            &self.context.synchronization2_device,
            frame.command_buffer,
            swap_image,
            vk::ImageAspectFlags::COLOR,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::PipelineStageFlags2::TOP_OF_PIPE,
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags2::empty(),
            vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
        );
        // UNDEFINED -> DEPTH_ATTACHMENT_OPTIMAL
        Self::transition_image(
            &self.context.synchronization2_device,
            frame.command_buffer,
            self.swapchain.as_ref().unwrap().depth_image.handle(),
            vk::ImageAspectFlags::DEPTH,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
            vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS,
            vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS,
            vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
            vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
        );

        let color_attachments = [vk::RenderingAttachmentInfo::default()
            .image_view(swap_view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            })];
        let depth_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(self.swapchain.as_ref().unwrap().depth_image_view.handle())
            .image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .clear_value(vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            });
        let rendering_info = vk::RenderingInfo::default()
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.swapchain.as_ref().unwrap().extent,
            })
            .layer_count(1)
            .color_attachments(&color_attachments)
            .depth_attachment(&depth_attachment);

        unsafe {
            self.context
                .dynamic_rendering_device
                .cmd_begin_rendering(frame.command_buffer, &rendering_info);
        }

        Some(Frame {
            renderer: self,
            image_index,
            frame_start_time: std::time::Instant::now(),
        })
    }

    fn transition_image(
        device: &synchronization2::Device,
        cmd: vk::CommandBuffer,
        image: vk::Image,
        aspect_mask: vk::ImageAspectFlags,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        src_stage: vk::PipelineStageFlags2,
        dst_stage: vk::PipelineStageFlags2,
        src_access: vk::AccessFlags2,
        dst_access: vk::AccessFlags2,
    ) {
        let barrier = vk::ImageMemoryBarrier2::default()
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_stage_mask(src_stage)
            .dst_stage_mask(dst_stage)
            .src_access_mask(src_access)
            .dst_access_mask(dst_access)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        let image_barriers = [barrier];
        let dependency_info = vk::DependencyInfo::default().image_memory_barriers(&image_barriers);

        unsafe {
            device.cmd_pipeline_barrier2(cmd, &dependency_info);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.context.device.device_wait_idle().unwrap();
            self.scenarios.clear(); // Drop scenarios first
            self.global_buffer.free(self.context.allocator());
            self.pipeline = None; // Drop pipeline next
            self.swapchain = None; // Drop swapchain next
            self.frame_data = None; // Drop frame data next
            self.context
                .surface_instance
                .destroy_surface(self.surface, None);
        }
    }
}

pub(crate) struct Frame<'a> {
    pub renderer: &'a mut Renderer,
    pub image_index: u32,
    pub frame_start_time: std::time::Instant,
}

impl<'a> Frame<'a> {
    pub fn end_frame(&mut self) {
        let r = &mut self.renderer;
        let frame = r.current_frame();
        let swapchain_image = r.swapchain.as_ref().unwrap().images[self.image_index as usize];

        unsafe {
            r.context
                .dynamic_rendering_device
                .cmd_end_rendering(frame.command_buffer);
        }

        // COLOR_ATTACHMENT_OPTIMAL -> PRESENT_SRC_KHR
        Renderer::transition_image(
            &r.context.synchronization2_device,
            frame.command_buffer,
            swapchain_image,
            vk::ImageAspectFlags::COLOR,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::PipelineStageFlags2::BOTTOM_OF_PIPE,
            vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
            vk::AccessFlags2::empty(),
        );

        unsafe {
            r.context
                .device
                .end_command_buffer(frame.command_buffer)
                .unwrap();
        }

        let wait_semaphores_info = [vk::SemaphoreSubmitInfo::default()
            .semaphore(frame.image_available_semaphore)
            .stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
            .device_index(0)];
        let command_buffers_info =
            [vk::CommandBufferSubmitInfo::default().command_buffer(frame.command_buffer)];
        let signal_semaphores_info = [vk::SemaphoreSubmitInfo::default()
            .semaphore(
                r.swapchain.as_ref().unwrap().render_finished_semaphores[self.image_index as usize],
            )
            .stage_mask(vk::PipelineStageFlags2::BOTTOM_OF_PIPE)
            .device_index(0)];
        let submit_info = vk::SubmitInfo2::default()
            .wait_semaphore_infos(&wait_semaphores_info)
            .command_buffer_infos(&command_buffers_info)
            .signal_semaphore_infos(&signal_semaphores_info);

        unsafe {
            r.context
                .device
                .queue_submit2(r.context.queue, &[submit_info], frame.in_flight_fence)
                .unwrap();
        }

        let swapchains = [r.swapchain.as_ref().unwrap().handle];
        let image_indices = [self.image_index];
        let signal_semaphores = [r.swapchain.as_ref().unwrap().render_finished_semaphores[self.image_index as usize]];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&signal_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);
        let present_result = unsafe {
            r.context
                .swapchain_device
                .queue_present(r.context.queue, &present_info)
        };

        match present_result {
            Ok(false) => {} // all good
            Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                r.swapchain = None;
                r.recreate_swapchain();
            }
            Err(e) => panic!("failed to present: {e:?}"),
        }

        if (r.frame_index + 1) % 100 == 0 {
            println!(
                "Average frame time over last 100 frames: {:.2} ms",
                r.frame_start_time.elapsed().as_secs_f32() * 1000.0 / 100.0
            );
            r.frame_start_time = std::time::Instant::now();
        }

        r.current_frame = (r.current_frame + 1) % FRAMES_IN_FLIGHT;
        r.frame_index += 1;
    }
}
