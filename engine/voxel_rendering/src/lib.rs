use std::{
    borrow::Cow,
    ffi::{CStr, c_char},
    sync::Arc,
};

use ash::{
    Device, Entry, Instance, ext::{debug_utils}, khr::{surface, swapchain}, vk,
};
use raw_window_handle::{HasWindowHandle, RawDisplayHandle, RawWindowHandle};

pub mod mesh;
pub use mesh::*;

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = unsafe { *p_callback_data };
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        unsafe { CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy() }
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        unsafe { CStr::from_ptr(callback_data.p_message).to_string_lossy() }
    };

    println!(
        "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
    );

    vk::FALSE
}

const MAX_FRAMES_IN_FLIGHT: usize = 2;
pub struct Renderer {
    entry: Entry,
    display_handle: RawDisplayHandle,
    instance: Instance,
    pdevice: vk::PhysicalDevice,
    device: Arc<Device>,
    surface_loader: surface::Instance,
    swapchain_loader: swapchain::Device,
    debug_utils_loader: debug_utils::Instance,
    debug_callback: vk::DebugUtilsMessengerEXT,
    queue: vk::Queue,
    pool: vk::CommandPool,
    setup_command_buffer: vk::CommandBuffer,
    app_setup_command_buffer: vk::CommandBuffer,
    frame_command_buffers: Vec<vk::CommandBuffer>,
    present_complete_semaphores: [vk::Semaphore; MAX_FRAMES_IN_FLIGHT],
    draw_commands_reuse_fences: [vk::Fence; MAX_FRAMES_IN_FLIGHT],
    frame_index: usize,
    wctx: Option<WindowContext>,
    meshes: Vec<Mesh>,
}

struct WindowContext {
    window_handle: RawWindowHandle,
    window_width: u32,
    window_height: u32,
    surface: vk::SurfaceKHR,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,
    rendering_complete_semaphores: Vec<vk::Semaphore>,
}

impl Renderer {
    pub fn new(display_handle: RawDisplayHandle) -> Self {
        // Load Vulkan entry
        let entry = unsafe { Entry::load().expect("Failed to load Vulkan entry") };

        let instance = Self::init_instance(&entry, display_handle);

        // Set up debug callback
        let (debug_utils_loader, debug_callback) = Self::init_debugging(&entry, &instance);

        // Enumerate physical devices and select one
        let (pdevice, queue_family_index) =
            Self::init_physical_device(&entry, &instance, &display_handle);

        let props = unsafe { instance.get_physical_device_properties(pdevice) };
        println!("Selected physical device: {:?}", unsafe {
            CStr::from_ptr(props.device_name.as_ptr())
        });

        // Create logical device and retrieve queue
        let device = Self::init_device(&instance, pdevice, queue_family_index);
        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        // Init extension device loaders
        let surface_loader = surface::Instance::new(&entry, &instance);
        let swapchain_loader = swapchain::Device::new(&instance, &device);

        // Initialize command buffers
        let (pool, setup_command_buffer, app_setup_command_buffer, frame_command_buffers) =
            Self::init_command_buffer(&device, queue_family_index);

        let (present_complete_semaphores, draw_commands_reuse_fences) = Self::init_syncs(&device);

        Self {
            entry,
            display_handle,
            instance,
            pdevice,
            device,
            surface_loader,
            swapchain_loader,
            debug_utils_loader,
            debug_callback,
            queue,
            pool,
            setup_command_buffer,
            app_setup_command_buffer,
            frame_command_buffers,
            present_complete_semaphores,
            draw_commands_reuse_fences,
            frame_index: 0,
            meshes: Vec::new(),
            wctx: None,
        }
    }

    pub fn set_window(
        &mut self,
        window_handle: RawWindowHandle,
        window_width: u32,
        window_height: u32,
    ) {
        // Create Vulkan surface
        let (surface, surface_loader) = self.init_surface(window_handle);

        // Create swapchain and related resources
        let (
            swapchain,
            present_images,
            present_image_views,
            rendering_complete_semaphores,
        ) = self.init_swapchain( &surface, window_width, window_height);

        // Create renderpass
        let surface_format = unsafe {
            surface_loader
                .get_physical_device_surface_formats(self.pdevice, surface)
                .expect("Failed to get surface formats")[0]
        };
        let render_pass = self.init_renderpass(surface_format.format);

        // Create framebuffers
        let framebuffers = self.init_framebuffers(
            &present_image_views,
            render_pass,
            window_width,
            window_height,
        );

        self.wctx = Some(WindowContext {
            window_handle,
            window_width,
            window_height,
            surface,
            swapchain,
            swapchain_images: present_images,
            swapchain_image_views: present_image_views,
            render_pass,
            framebuffers,
            rendering_complete_semaphores,
        })
    }

    pub fn set_meshes(&mut self, meshes: Vec<Mesh>) {
        self.meshes = meshes;
    }

    fn init_instance(entry: &Entry, display_handle: RawDisplayHandle) -> Instance {
        // Initialize Vulkan instance
        let app_name = c"VulkanApp";

        let layer_names = [c"VK_LAYER_KHRONOS_validation"];
        let layer_names_raw: Vec<*const c_char> =
            layer_names.iter().map(|&name| name.as_ptr()).collect();

        let mut extension_names = ash_window::enumerate_required_extensions(display_handle)
            .unwrap()
            .to_vec();
        extension_names.push(debug_utils::NAME.as_ptr());

        let appinfo = vk::ApplicationInfo::default()
            .application_name(app_name)
            .application_version(0)
            .engine_name(app_name)
            .engine_version(0)
            .api_version(vk::make_api_version(0, 1, 0, 0));

        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&appinfo)
            .enabled_layer_names(&layer_names_raw)
            .enabled_extension_names(&extension_names);

        unsafe {
            entry
                .create_instance(&create_info, None)
                .expect("Failed to create Vulkan instance")
        }
    }

    fn init_debugging(
        entry: &Entry,
        instance: &Instance,
    ) -> (debug_utils::Instance, vk::DebugUtilsMessengerEXT) {
        let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .pfn_user_callback(Some(vulkan_debug_callback));

        let debug_utils_loader = debug_utils::Instance::new(entry, instance);
        let debug_call_back = unsafe {
            debug_utils_loader
                .create_debug_utils_messenger(&debug_info, None)
                .expect("Failed to create debug callback")
        };

        (debug_utils_loader, debug_call_back)
    }

    fn init_physical_device(
        entry: &Entry,
        instance: &Instance,
        display_handle: &raw_window_handle::RawDisplayHandle,
    ) -> (vk::PhysicalDevice, u32) {
        let pdevices = unsafe {
            instance
                .enumerate_physical_devices()
                .expect("Failed to enumerate physical devices")
        };
        let (pdevice, queue_family_index) = unsafe {
            pdevices
                .iter()
                .find_map(|pdevice| {
                    instance
                        .get_physical_device_queue_family_properties(*pdevice)
                        .iter()
                        .enumerate()
                        .find_map(|(index, info)| {
                            let supports_graphics =
                                info.queue_flags.contains(vk::QueueFlags::GRAPHICS);
                            let supports_present = match display_handle {
                                RawDisplayHandle::Windows(_) => {
                                    let win32_surface =
                                        ash::khr::win32_surface::Instance::new(entry, instance);
                                    win32_surface.get_physical_device_win32_presentation_support(
                                        *pdevice,
                                        index as u32,
                                    )
                                }
                                _ => false, // Add support for other platforms as needed
                            };
                            if supports_graphics && supports_present {
                                Some((*pdevice, index as u32))
                            } else {
                                None
                            }
                        })
                })
                .expect("No available device.")
        };
        (pdevice, queue_family_index)
    }

    fn init_device(
        instance: &Instance,
        pdevice: vk::PhysicalDevice,
        queue_family_index: u32,
    ) -> Arc<Device> {
        let device_extension_names_raw = [
            swapchain::NAME.as_ptr(),
        ];
        let features = vk::PhysicalDeviceFeatures {
            shader_clip_distance: 1,
            ..Default::default()
        };
        let priorities = [1.0];

        let queue_info = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&priorities)];

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_info)
            .enabled_extension_names(&device_extension_names_raw)
            .enabled_features(&features);

        unsafe {
            Arc::new(
                instance
                    .create_device(pdevice, &device_create_info, None)
                    .expect("Failed to create logical device"),
            )
        }
    }

    fn init_command_buffer(
        device: &Device,
        queue_family_index: u32,
    ) -> (
        vk::CommandPool,
        vk::CommandBuffer,
        vk::CommandBuffer,
        Vec<vk::CommandBuffer>,
    ) {
        let pool_create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(queue_family_index);

        let pool = unsafe {
            device
                .create_command_pool(&pool_create_info, None)
                .expect("Failed to create command pool")
        };

        let command_buffer_allocatea_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(2 + MAX_FRAMES_IN_FLIGHT as u32);

        let command_buffers = unsafe {
            device
                .allocate_command_buffers(&command_buffer_allocatea_info)
                .expect("Failed to allocate command buffers")
        };
        let setup_comamnd_buffer = command_buffers[0];
        let app_setup_command_buffer = command_buffers[1];
        let frame_command_buffers = &command_buffers[2..];

        (
            pool,
            setup_comamnd_buffer,
            app_setup_command_buffer,
            frame_command_buffers.to_vec(),
        )
    }

    fn init_syncs(
        device: &Device,
    ) -> (
        [vk::Semaphore; MAX_FRAMES_IN_FLIGHT],
        [vk::Fence; MAX_FRAMES_IN_FLIGHT],
    ) {
        let semaphore_create_info = vk::SemaphoreCreateInfo::default();

        let present_complete_semaphore = std::array::from_fn(|_| unsafe {
            device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap()
        });

        let fence_create_info =
            vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

        let draw_commands_reuse_fences = std::array::from_fn(|_| unsafe {
            device.create_fence(&fence_create_info, None).unwrap()
        });

        (present_complete_semaphore, draw_commands_reuse_fences)
    }

    fn init_surface(&self, window_handle: RawWindowHandle) -> (vk::SurfaceKHR, surface::Instance) {
        let surface = unsafe {
            ash_window::create_surface(
                &self.entry,
                &self.instance,
                self.display_handle,
                window_handle,
                None,
            )
            .expect("Failed to create Vulkan surface")
        };
        let surface_loader = surface::Instance::new(&self.entry, &self.instance);

        (surface, surface_loader)
    }

    fn init_swapchain(
        &self,
        surface: &vk::SurfaceKHR,
        window_width: u32,
        window_height: u32,
    ) -> (
        vk::SwapchainKHR,
        Vec<vk::Image>,
        Vec<vk::ImageView>,
        Vec<vk::Semaphore>,
    ) {
        let surface_format = unsafe {
            self.surface_loader
                .get_physical_device_surface_formats(self.pdevice, *surface)
                .expect("Failed to get surface formats")[0]
        };

        let surface_capabilities = unsafe {
            self.surface_loader
                .get_physical_device_surface_capabilities(self.pdevice, *surface)
                .expect("Failed to get surface capabilities")
        };
        let mut desired_image_count = surface_capabilities.min_image_count + 1;
        if surface_capabilities.max_image_count > 0
            && desired_image_count > surface_capabilities.max_image_count
        {
            desired_image_count = surface_capabilities.max_image_count;
        }
        let surface_resolution = match surface_capabilities.current_extent.width {
            std::u32::MAX => vk::Extent2D {
                width: window_width,
                height: window_height,
            },
            _ => surface_capabilities.current_extent,
        };
        let pre_transform = if surface_capabilities
            .supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            surface_capabilities.current_transform
        };

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(*surface)
            .min_image_count(desired_image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(surface_resolution)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(pre_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::FIFO)
            .clipped(true);

        let swapchain = unsafe {
            self.swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .expect("Failed to create swapchain")
        };

        let present_images = unsafe {
            self.swapchain_loader
                .get_swapchain_images(swapchain)
                .expect("Failed to get swapchain images")
        };
        let present_image_views: Vec<vk::ImageView> = present_images
            .iter()
            .map(|&image| {
                let create_view_info = vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(surface_format.format)
                    .components(vk::ComponentMapping::default())
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });
                unsafe {
                    self.device
                        .create_image_view(&create_view_info, None)
                        .expect("Failed to create image view")
                }
            })
            .collect();

        let rendering_complete_semaphores: Vec<vk::Semaphore> = (0..present_images.len())
            .map(|_| {
                let semaphore_create_info = vk::SemaphoreCreateInfo::default();
                unsafe {
                    self.device
                        .create_semaphore(&semaphore_create_info, None)
                        .expect("Failed to create rendering complete semaphore")
                }
            })
            .collect();

        (
            swapchain,
            present_images,
            present_image_views,
            rendering_complete_semaphores,
        )
    }

    pub fn recreate_swapchain(&mut self, window_width: u32, window_height: u32) {
        let wctx = self.wctx.as_ref().unwrap();

        // Destroy old swapchain and related resources
        unsafe {
            self.device
                .device_wait_idle()
                .expect("Failed to wait for device idle");
            wctx.rendering_complete_semaphores
                .iter()
                .for_each(|&semaphore| {
                    self.device.destroy_semaphore(semaphore, None);
                });
            // Destroy framebuffers created for the swapchain first
            wctx.framebuffers.iter().for_each(|&fb| {
                if fb != vk::Framebuffer::null() {
                    self.device.destroy_framebuffer(fb, None);
                }
            });
            // Then destroy image views
            wctx.swapchain_image_views.iter().for_each(|&image_view| {
                if image_view != vk::ImageView::null() {
                    self.device.destroy_image_view(image_view, None);
                }
            });
            self.swapchain_loader
                .destroy_swapchain(wctx.swapchain, None);
        }

        let (
            swapchain,
            swapchain_images,
            swapchain_image_views,
            rendering_complete_semaphores,
        ) = self.init_swapchain(
            &wctx.surface,
            window_width,
            window_height,
        );

        // Create framebuffers for the new swapchain images using the existing render pass
        let new_framebuffers = self.init_framebuffers(
            &swapchain_image_views,
            wctx.render_pass,
            window_width,
            window_height,
        );

        let wctx = self.wctx.as_mut().unwrap();
        wctx.swapchain = swapchain;
        wctx.swapchain_images = swapchain_images;
        wctx.swapchain_image_views = swapchain_image_views;
        wctx.framebuffers = new_framebuffers;
        wctx.rendering_complete_semaphores = rendering_complete_semaphores;
    }

    fn init_renderpass(&self, surface_format: vk::Format) -> vk::RenderPass {
        let attachments = [vk::AttachmentDescription::default()
            .format(surface_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];

        let color_attachment_refs = [vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];

        let subpass_descs = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachment_refs)];

        let subpass_deps = [vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)];

        let renderpass_create_info = vk::RenderPassCreateInfo::default()
            .attachments(&attachments)
            .subpasses(&subpass_descs)
            .dependencies(&subpass_deps);

        unsafe {
            self.device
                .create_render_pass(&renderpass_create_info, None)
                .expect("Failed to create render pass")
        }
    }

    fn init_framebuffers(
        &self,
        swapchain_image_views: &[vk::ImageView],
        render_pass: vk::RenderPass,
        width: u32,
        height: u32,
    ) -> Vec<vk::Framebuffer> {
        swapchain_image_views
            .iter()
            .map(|&image_view| {
                let framebuffer_attachments = [image_view];
                let framebuffer_create_info = vk::FramebufferCreateInfo::default()
                    .render_pass(render_pass)
                    .attachments(&framebuffer_attachments)
                    .width(width)
                    .height(height)
                    .layers(1);
                unsafe {
                    self.device
                        .create_framebuffer(&framebuffer_create_info, None)
                        .expect("Failed to create framebuffer")
                }
            })
            .collect()
    }

    fn init_shaders(&self) {

    }

    fn find_memory_type(
        &self,
        type_filter: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> Option<u32> {
        let mem_properties = unsafe {
            self.instance
                .get_physical_device_memory_properties(self.pdevice)
        };

        for (i, memory_type) in mem_properties.memory_types.iter().enumerate() {
            if (type_filter & (1 << i)) != 0 && memory_type.property_flags.contains(properties) {
                return Some(i as u32);
            }
        }
        None
    }

    fn create_buffer(
        &self,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
    ) -> (vk::Buffer, vk::DeviceMemory) {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe {
            self.device
                .create_buffer(&buffer_info, None)
                .expect("Failed to create buffer")
        };

        let mem_requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };

        let mem_type_index = self
            .find_memory_type(mem_requirements.memory_type_bits, properties)
            .unwrap();

        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_requirements.size)
            .memory_type_index(mem_type_index);

        let buffer_memory = unsafe {
            self.device
                .allocate_memory(&alloc_info, None)
                .expect("Failed to allocate buffer memory")
        };

        unsafe {
            self.device
                .bind_buffer_memory(buffer, buffer_memory, 0)
                .expect("Failed to bind buffer memory");
        }

        (buffer, buffer_memory)
    }

    fn upload_to_buffer<T>(&self, buffer_memory: vk::DeviceMemory, data: &[T]) {
        let data_size = (std::mem::size_of::<T>() * data.len()) as vk::DeviceSize;
        let data_ptr = data.as_ptr() as *const u8;

        unsafe {
            // Map memory
            let mapped_memory = self
                .device
                .map_memory(buffer_memory, 0, data_size, vk::MemoryMapFlags::empty())
                .expect("Failed to map buffer memory");

            // Copy data
            std::ptr::copy_nonoverlapping(data_ptr, mapped_memory as *mut u8, data_size as usize);

            // Unmap memory
            self.device.unmap_memory(buffer_memory);
        }
    }

    // TODO: init shaders and pipeline

    // draw for one frame, should be called in a loop
    pub fn draw(&mut self) {
        // Wait for the previous frame to finish
        let fence = self.draw_commands_reuse_fences[self.frame_index];
        unsafe {
            self.device
                .wait_for_fences(&[fence], true, std::u64::MAX)
                .expect("Failed to wait for fence");
            self.device
                .reset_fences(&[fence])
                .expect("Failed to reset fence");
        }
        
        // Acquire the next image from the swapchain
        let wctx = self.wctx.as_ref().unwrap();
        let (image_index, suboptimal) = unsafe {
            self.swapchain_loader
                .acquire_next_image(
                    wctx.swapchain,
                    std::u64::MAX,
                    wctx.rendering_complete_semaphores[self.frame_index],
                    vk::Fence::null(),
                )
                .expect("Failed to acquire next image")
        };

        if suboptimal {
            self.recreate_swapchain(wctx.window_width, wctx.window_height);
            return;
        }

        // Reset command buffer
        let command_buffer = self.frame_command_buffers[self.frame_index];
        unsafe {
            self.device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
                .expect("Failed to reset command buffer");
        
        }

        // Record command buffer

        unsafe {
            self.device.begin_command_buffer(
                command_buffer,
                &vk::CommandBufferBeginInfo::default(),
            ).expect("Failed to begin command buffer");
        }

        // Clear screen and start renderpass

        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.9, 0.0, 0.0, 1.0],
            },
        }];

        let render_pass_begin_info = vk::RenderPassBeginInfo::default()
            .render_pass(wctx.render_pass)
            .framebuffer(wctx.framebuffers[image_index as usize])
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D {
                    width: wctx.window_width,
                    height: wctx.window_height,
                },
            })
            .clear_values(&clear_values);
            
        unsafe {
            self.device.cmd_begin_render_pass(
                command_buffer,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_end_render_pass(command_buffer);
        }

        // End command buffer recording
        unsafe {
            self.device
                .end_command_buffer(command_buffer)
                .expect("Failed to end command buffer");
        }

        let wait_semaphores = [wctx.rendering_complete_semaphores[self.frame_index]];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = [command_buffer];
        let signal_semaphores = [wctx.rendering_complete_semaphores[self.frame_index]];

        // Submit command buffer to the graphics queue
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_semaphores);

        unsafe {
            self.device
                .queue_submit(self.queue, &[submit_info], fence)
                .expect("Failed to submit draw command buffer");
        }

        // Present the rendered image to the swapchain
        let swapchains = [wctx.swapchain];
        let images_indices = [image_index];

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&signal_semaphores)
            .swapchains(&swapchains)
            .image_indices(&images_indices);

        unsafe {
            self.swapchain_loader
                .queue_present(self.queue, &present_info)
                .expect("Failed to present swapchain image");
        }

        self.frame_index = (self.frame_index + 1) % MAX_FRAMES_IN_FLIGHT;
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.device
                .device_wait_idle()
                .expect("Failed to wait for device idle");
            if let Some(wctx) = self.wctx.as_ref() {
                wctx.rendering_complete_semaphores
                    .iter()
                    .for_each(|&semaphore| {
                        self.device.destroy_semaphore(semaphore, None);
                    });
                // Destroy framebuffers created for the swapchain first
                wctx.framebuffers.iter().for_each(|&fb| {
                    if fb != vk::Framebuffer::null() {
                        self.device.destroy_framebuffer(fb, None);
                    }
                });
                // Then destroy image views
                wctx.swapchain_image_views.iter().for_each(|&image_view| {
                    if image_view != vk::ImageView::null() {
                        self.device.destroy_image_view(image_view, None);
                    }
                });
                self.swapchain_loader
                    .destroy_swapchain(wctx.swapchain, None);
                self.surface_loader.destroy_surface(wctx.surface, None);
                self.device.destroy_render_pass(wctx.render_pass, None);
            }
            self.present_complete_semaphores
                .iter()
                .for_each(|&semaphore| {
                    self.device.destroy_semaphore(semaphore, None);
                });
            self.draw_commands_reuse_fences.iter().for_each(|&fence| {
                self.device.destroy_fence(fence, None);
            });
            self.device.destroy_command_pool(self.pool, None);
            self.device.destroy_device(None);
            self.debug_utils_loader
                .destroy_debug_utils_messenger(self.debug_callback, None);
            self.instance.destroy_instance(None);
        }
    }
}
