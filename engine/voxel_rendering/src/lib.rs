use std::{
    borrow::Cow,
    ffi::{CStr, c_char},
    sync::Arc,
};

use ash::{
    Device, Entry, Instance, ext::debug_utils, khr::{dynamic_rendering, surface, swapchain}, vk::{self, ExtendsPhysicalDeviceVideoFormatInfoKHR, ShaderStageFlags},
};
use raw_window_handle::{HasWindowHandle, RawDisplayHandle, RawWindowHandle};

// pub mod gpu_mesh;
// pub use gpu_mesh::*;

pub mod mesh;
pub use mesh::*;

pub mod shader;
pub use shader::*;

pub mod pipeline;
pub use pipeline::*;

pub mod environment;
pub use environment::*;

pub mod buffer;
pub use buffer::*;

pub mod v2;

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
    instance: Arc<Instance>,
    pdevice: Arc<vk::PhysicalDevice>,
    device: Arc<Device>,
    surface_loader: Arc<surface::Instance>,
    swapchain_loader: Arc<swapchain::Device>,
    debug_utils_loader: debug_utils::Instance,
    debug_callback: vk::DebugUtilsMessengerEXT,
    queue: vk::Queue,
    pool: vk::CommandPool,
    staging_command_buffer: vk::CommandBuffer,
    frame_command_buffers: Vec<vk::CommandBuffer>,
    present_complete_semaphores: [vk::Semaphore; MAX_FRAMES_IN_FLIGHT],
    draw_commands_reuse_fences: [vk::Fence; MAX_FRAMES_IN_FLIGHT],
    frame_submitted: [bool; MAX_FRAMES_IN_FLIGHT],
    indirect_command_buffer: Option<[FixedSizeBuffer<vk::DrawIndexedIndirectCommand, 1024>; MAX_FRAMES_IN_FLIGHT]>,
    frame_index: usize,
    descriptor_set: Vec<vk::DescriptorSet>,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    wctx: Option<WindowContext>,
    environment: Option<Environment>,
}

struct WindowContext {
    window: Arc<winit::window::Window>,
    device: Arc<Device>,
    swapchain_loader: Arc<swapchain::Device>,
    surface_loader: Arc<surface::Instance>,
    frame_width: u32,
    frame_height: u32,
    surface: vk::SurfaceKHR,
    swapchain: vk::SwapchainKHR,
    swapchain_out_of_date: bool,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    depth_images: Vec<vk::Image>,
    depth_image_memories: Vec<vk::DeviceMemory>,
    depth_image_views: Vec<vk::ImageView>,
    render_pass: vk::RenderPass,
    pipeline: Pipeline,
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
        let (pool, staging_command_buffer, frame_command_buffers) =
            Self::init_command_buffer(&device, queue_family_index);

        let (present_complete_semaphores, draw_commands_reuse_fences) = Self::init_syncs(&device);

        let instance =  Arc::new(instance);
        let pdevice = Arc::new(pdevice);
        let device = Arc::new(device);

        let indirect_command_buffer = [
            self::create_fixed_size_buffer(instance.clone(), pdevice.clone(), device.clone(), BufferUsage::Indirect),
            self::create_fixed_size_buffer(instance.clone(), pdevice.clone(), device.clone(), BufferUsage::Indirect),
        ];

        let vertex_buffer = self::create_fixed_size_buffer::<Vertex, 16384000>(instance.clone(), pdevice.clone(), device.clone(), BufferUsage::Vertex);
        let index_buffer = self::create_fixed_size_buffer::<u32, 16384000>(instance.clone(), pdevice.clone(), device.clone(), BufferUsage::Index);
        let object_buffer = self::create_fixed_size_buffer::<GPUObjectData, 16384>(instance.clone(), pdevice.clone(), device.clone(), BufferUsage::Storage);

        let environment = Environment::new(device.clone(), Box::new(vertex_buffer), Box::new(index_buffer), Box::new(object_buffer));

        let descriptor_pool_size = [
            vk::DescriptorPoolSize::default()
                .descriptor_count(1)
                .ty(vk::DescriptorType::STORAGE_BUFFER)
        ];
        let descriptor_pool_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(1)
            .pool_sizes(&descriptor_pool_size);
        let descriptor_pool = unsafe {
            device
                .create_descriptor_pool(&descriptor_pool_info, None)
                .expect("Failed to create descriptor pool")
        };

        let descriptor_set_layout_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                    .binding(0)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::VERTEX),
        ];
        let descriptor_set_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(&descriptor_set_layout_bindings);
        let descriptor_set_layout = unsafe {
            device
                .create_descriptor_set_layout(&descriptor_set_info, None)
                .expect("Failed to create descriptor set layout")
        };

        let descriptor_set_layouts = [
            descriptor_set_layout
        ];
        let descriptor_set_alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&descriptor_set_layouts);
        let descriptor_set = unsafe {
            device
                .allocate_descriptor_sets(&descriptor_set_alloc_info)
                .expect("Failed to allocate descriptor sets")
        };

        let buffer_info = [
            vk::DescriptorBufferInfo::default()
                .buffer(environment.object_buffer().handle())
                .offset(0)
                .range(vk::WHOLE_SIZE)
        ];
        let descriptor_writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(descriptor_set[0])
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&buffer_info),
        ];
        unsafe {
            device.update_descriptor_sets(&descriptor_writes, &[]);
        }


        Self {
            entry,
            display_handle,
            instance,
            pdevice,
            device,
            surface_loader: Arc::new(surface_loader),
            swapchain_loader: Arc::new(swapchain_loader),
            debug_utils_loader,
            debug_callback,
            queue,
            pool,
            staging_command_buffer,
            frame_command_buffers,
            present_complete_semaphores,
            draw_commands_reuse_fences,
            frame_submitted: [false; MAX_FRAMES_IN_FLIGHT],
            indirect_command_buffer: Some(indirect_command_buffer),
            frame_index: 0,
            wctx: None,
            descriptor_set,
            descriptor_set_layout,
            descriptor_pool,
            environment: Some(environment),
        }
    }

    pub fn set_window(&mut self, window: Arc<winit::window::Window>, debug: bool) {
        let window_handle = window.window_handle().unwrap().as_raw();
        let window_size = window.inner_size();
        let window_width = window_size.width;
        let window_height = window_size.height;

        // Create Vulkan surface
        let (surface, surface_loader) = self.init_surface(window_handle);

        // Create swapchain and related resources
        let (swapchain, present_images, present_image_views, rendering_complete_semaphores) =
            self.init_swapchain(&surface, window_width, window_height);

        // Create renderpass
        let surface_format = unsafe {
            surface_loader
                .get_physical_device_surface_formats(*self.pdevice.as_ref(), surface)
                .expect("Failed to get surface formats")[0]
        };
        let render_pass = self.init_renderpass(surface_format.format);
        let (depth_images, depth_image_memories, depth_image_views) =
            self.init_depth_resources(window_width, window_height, present_image_views.len() as u32);

        // Create framebuffers
        let framebuffers = self.init_framebuffers(
            &present_image_views,
            &depth_image_views,
            render_pass,
            window_width,
            window_height,
        );

        // Load shaders
        let vertex_shader = Shader::new(self.device.clone(), "shaders/vertex.vert.spv");
        let fragment_shader = Shader::new(self.device.clone(), "shaders/fragment.frag.spv");

        // Create pipeline
        let dynamic_states = [
            vk::DynamicState::VIEWPORT,
            vk::DynamicState::SCISSOR,
        ];
        let camera_data_range = vk::PushConstantRange::default()
            .stage_flags(ShaderStageFlags::VERTEX)
            .offset(0)
            .size(std::mem::size_of::<CameraData>() as u32);
        let vertex_binding_descriptions = [vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)];
        let vertex_attribute_descriptions = [
            vk::VertexInputAttributeDescription::default()
                .location(0)
                .binding(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(0),
            vk::VertexInputAttributeDescription::default()
                .location(1)
                .binding(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(12),
            vk::VertexInputAttributeDescription::default()
                .location(2)
                .binding(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(24),
        ];
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vertex_binding_descriptions)
            .vertex_attribute_descriptions(&vertex_attribute_descriptions);
        let color_attachment_state = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA);
        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);
        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(if debug { vk::PolygonMode::LINE } else { vk::PolygonMode::FILL })
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let pipeline = PipelineBuilder::new(self.device.clone(), render_pass)
            .add_push_constant_range(camera_data_range)
            .add_descriptor_set_layout(self.descriptor_set_layout)
            .add_shader_stage(&vertex_shader, vk::ShaderStageFlags::VERTEX)
            .add_shader_stage(&fragment_shader, vk::ShaderStageFlags::FRAGMENT)
            .vertex_input_state(vertex_input_state)
            .input_assembly_state(vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST))
            .rasterization_state(rasterization_state)
            .depth_stencil_state(depth_stencil_state)
            .multisample_state(vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1))
            .color_blend_state(vk::PipelineColorBlendStateCreateInfo::default().attachments(&[color_attachment_state]))
            .viewport_state(vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1))
            .dynamic_state(&dynamic_states)
            .build();

        self.wctx = Some(WindowContext {
            window,
            device: self.device.clone(),
            swapchain_loader: self.swapchain_loader.clone(),
            surface_loader: self.surface_loader.clone(),
            frame_width: window_width,
            frame_height: window_height,
            surface,
            swapchain,
            swapchain_images: present_images,
            swapchain_image_views: present_image_views,
            depth_images,
            depth_image_memories,
            depth_image_views,
            render_pass,
            pipeline,
            framebuffers,
            swapchain_out_of_date: false,
            rendering_complete_semaphores,
        })
    }

    pub fn environment(&self) -> Option<&Environment> {
        self.environment.as_ref()
    }

    pub fn environment_mut(&mut self) -> Option<&mut Environment> {
        self.environment.as_mut()
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
                                RawDisplayHandle::Wayland(wayland_display) => {
                                    let wayland_surface =
                                        ash::khr::wayland_surface::Instance::new(entry, instance);
                                    wayland_surface.get_physical_device_wayland_presentation_support(
                                        *pdevice,
                                        index as u32,
                                        &mut *(wayland_display.display.as_ptr() as *mut std::os::raw::c_void),
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
    ) -> Device {
        let device_extension_names_raw = [
            swapchain::NAME.as_ptr(),
            vk::KHR_MAINTENANCE1_NAME.as_ptr(),
            vk::KHR_SHADER_DRAW_PARAMETERS_NAME.as_ptr(),
            dynamic_rendering::NAME.as_ptr(),
        ];
        let features = vk::PhysicalDeviceFeatures {
            shader_clip_distance: 1,
            fill_mode_non_solid: 1,
            multi_draw_indirect: 1,
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
            instance
                .create_device(pdevice, &device_create_info, None)
                .expect("Failed to create logical device")
        }
    }

    fn init_command_buffer(
        device: &Device,
        queue_family_index: u32,
    ) -> (
        vk::CommandPool,
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
            .command_buffer_count(1 + MAX_FRAMES_IN_FLIGHT as u32);

        let command_buffers = unsafe {
            device
                .allocate_command_buffers(&command_buffer_allocatea_info)
                .expect("Failed to allocate command buffers")
        };
        let staging_command_buffer = command_buffers[0];
        let frame_command_buffers = &command_buffers[1..];

        (
            pool,
            staging_command_buffer,
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
                .get_physical_device_surface_formats(*self.pdevice.as_ref(), *surface)
                .expect("Failed to get surface formats")[0]
        };

        let surface_capabilities = unsafe {
            self.surface_loader
                .get_physical_device_surface_capabilities(*self.pdevice.as_ref(), *surface)
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

    pub fn recreate_swapchain(&mut self) {
        let wctx = self.wctx.as_ref().unwrap();

        let window_size = wctx.window.as_ref().inner_size();
        let window_width = window_size.width;
        let window_height = window_size.height;

        if window_width == 0 || window_height == 0 {
            let wctx = self.wctx.as_mut().unwrap();
            wctx.swapchain_out_of_date = true;
            return;
        }

        // Find valid fences
        let fences = self
            .draw_commands_reuse_fences
            .iter()
            .enumerate()
            .filter(|(i, _)| self.frame_submitted[*i])
            .map(|(_, &fence)| fence)
            .collect::<Vec<_>>();
        
        // Reset frame_submitted flags for all frames
        for i in 0..MAX_FRAMES_IN_FLIGHT {
            self.frame_submitted[i] = false;
        }

        // Destroy old swapchain and related resources
        unsafe {
            // Wait for fences
            if fences.len() > 0 {
                self.device
                    .wait_for_fences(&fences, true, u64::MAX)
                    .expect("Failed to wait for fences");
            }

            // Reset fences
            self.device
                .reset_fences(&self.draw_commands_reuse_fences)
                .expect("Failed to reset fences");

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
            wctx.depth_image_views.iter().for_each(|&image_view| {
                if image_view != vk::ImageView::null() {
                    self.device.destroy_image_view(image_view, None);
                }
            });
            wctx.depth_images.iter().for_each(|&image| {
                if image != vk::Image::null() {
                    self.device.destroy_image(image, None);
                }
            });
            wctx.depth_image_memories.iter().for_each(|&memory| {
                if memory != vk::DeviceMemory::null() {
                    self.device.free_memory(memory, None);
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

        let (swapchain, swapchain_images, swapchain_image_views, rendering_complete_semaphores) =
            self.init_swapchain(&wctx.surface, window_width, window_height);

        let (depth_images, depth_image_memories, depth_image_views) =
            self.init_depth_resources(window_width, window_height, swapchain_image_views.len() as u32);

        // Create framebuffers for the new swapchain images using the existing render pass
        let new_framebuffers = self.init_framebuffers(
            &swapchain_image_views,
            &depth_image_views,
            wctx.render_pass,
            window_width,
            window_height,
        );

        let wctx = self.wctx.as_mut().unwrap();
        wctx.frame_width = window_width;
        wctx.frame_height = window_height;
        wctx.swapchain = swapchain;
        wctx.swapchain_images = swapchain_images;
        wctx.swapchain_image_views = swapchain_image_views;
        wctx.depth_images = depth_images;
        wctx.depth_image_memories = depth_image_memories;
        wctx.depth_image_views = depth_image_views;
        wctx.framebuffers = new_framebuffers;
        wctx.rendering_complete_semaphores = rendering_complete_semaphores;
        wctx.swapchain_out_of_date = false;
    }

    fn init_renderpass(&self, surface_format: vk::Format) -> vk::RenderPass {
        let attachments = [
            vk::AttachmentDescription::default()
            .format(surface_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR),
            vk::AttachmentDescription::default()
                .format(vk::Format::D32_SFLOAT)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::DONT_CARE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
        ];

        let color_attachment_refs = [vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];

        let depth_attachment_ref = vk::AttachmentReference::default()
            .attachment(1)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let subpass_descs = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachment_refs)
            .depth_stencil_attachment(&depth_attachment_ref)];

        let subpass_deps = [vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)];

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
        depth_image_views: &[vk::ImageView],
        render_pass: vk::RenderPass,
        width: u32,
        height: u32,
    ) -> Vec<vk::Framebuffer> {
        swapchain_image_views
            .iter()
            .zip(depth_image_views.iter())
            .map(|(&color_image_view, &depth_image_view)| {
                let framebuffer_attachments = [color_image_view, depth_image_view];
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

    fn init_depth_resources(
        &self,
        width: u32,
        height: u32,
        image_count: u32,
    ) -> (Vec<vk::Image>, Vec<vk::DeviceMemory>, Vec<vk::ImageView>) {
        let mut depth_images = Vec::with_capacity(image_count as usize);
        let mut depth_image_memories = Vec::with_capacity(image_count as usize);
        let mut depth_image_views = Vec::with_capacity(image_count as usize);

        for _ in 0..image_count {
            let image_create_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::D32_SFLOAT)
                .extent(vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED);

            let image = unsafe {
                self.device
                    .create_image(&image_create_info, None)
                    .expect("Failed to create depth image")
            };

            let mem_requirements = unsafe { self.device.get_image_memory_requirements(image) };
            let mem_type_index = self
                .find_memory_type(
                    mem_requirements.memory_type_bits,
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                )
                .expect("Failed to find suitable memory type for depth image");

            let alloc_info = vk::MemoryAllocateInfo::default()
                .allocation_size(mem_requirements.size)
                .memory_type_index(mem_type_index);

            let image_memory = unsafe {
                self.device
                    .allocate_memory(&alloc_info, None)
                    .expect("Failed to allocate depth image memory")
            };

            unsafe {
                self.device
                    .bind_image_memory(image, image_memory, 0)
                    .expect("Failed to bind depth image memory");
            }

            let image_view_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(vk::Format::D32_SFLOAT)
                .components(vk::ComponentMapping::default())
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::DEPTH,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });

            let image_view = unsafe {
                self.device
                    .create_image_view(&image_view_info, None)
                    .expect("Failed to create depth image view")
            };

            depth_images.push(image);
            depth_image_memories.push(image_memory);
            depth_image_views.push(image_view);
        }

        (depth_images, depth_image_memories, depth_image_views)
    }

    fn find_memory_type(
        &self,
        type_filter: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> Option<u32> {
        let mem_properties = unsafe {
            self.instance
                .get_physical_device_memory_properties(*self.pdevice.as_ref())
        };

        for (i, memory_type) in mem_properties.memory_types.iter().enumerate() {
            if (type_filter & (1 << i)) != 0 && memory_type.property_flags.contains(properties) {
                return Some(i as u32);
            }
        }
        None
    }

    // TODO: init shaders and pipeline

    /// draw for one frame, should be called in a loop
    /// later: make this a method of a renderpass?
    pub fn draw(&mut self) {
        if self.wctx.as_ref().unwrap().swapchain_out_of_date {
            self.recreate_swapchain();
        }

        // Wait for the previous frame to finish
        let fence = self.draw_commands_reuse_fences[self.frame_index];
        if self.frame_submitted[self.frame_index] {
            unsafe {
                self.device
                    .wait_for_fences(&[fence], true, std::u64::MAX)
                    .expect("Failed to wait for fence");
            }
        }
        unsafe {
            self.device
                .reset_fences(&[fence])
                .expect("Failed to reset fence");
        }
        self.frame_submitted[self.frame_index] = true;

        // Acquire the next image from the swapchain
        let wctx = self.wctx.as_ref().unwrap();
        let (image_index, suboptimal) = unsafe {
            self.swapchain_loader
                .acquire_next_image(
                    wctx.swapchain,
                    std::u64::MAX,
                    self.present_complete_semaphores[self.frame_index],
                    vk::Fence::null(),
                )
                .expect("Failed to acquire next image")
        };

        if suboptimal {
            println!("Swapchain is suboptimal, recreating...");
            self.recreate_swapchain();
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
            self.device
                .begin_command_buffer(command_buffer, &vk::CommandBufferBeginInfo::default())
                .expect("Failed to begin command buffer");
        }

        // Clear screen and start renderpass

        let clear_values = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            },
        ];

        let render_pass_begin_info = vk::RenderPassBeginInfo::default()
            .render_pass(wctx.render_pass)
            .framebuffer(wctx.framebuffers[image_index as usize])
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D {
                    width: wctx.frame_width,
                    height: wctx.frame_height,
                },
            })
            .clear_values(&clear_values);

        // begin render pass
        unsafe {
            self.device.cmd_begin_render_pass(
                command_buffer,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );
        }
        
        // bind pipeline
        unsafe {
            self.device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                wctx.pipeline.get(),
            );
        }

        // update viewport and scissor dynamically
        let viewport = vk::Viewport {
            x: 0.0,
            y: wctx.frame_height as f32,
            width: wctx.frame_width as f32,
            height: -(wctx.frame_height as f32),
            min_depth: 0.0,
            max_depth: 1.0,
        };
        unsafe {
            self.device.cmd_set_viewport(command_buffer, 0, &[viewport]);
        }

        let scissors = [
            vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D {
                    width: wctx.frame_width,
                    height: wctx.frame_height,
                },
            }
        ];
        unsafe {
            self.device.cmd_set_scissor(command_buffer, 0, &scissors);
        }

        // Update push constants with camera data
        let camera_data = self.environment.as_ref().unwrap().camera_data();
        unsafe {
            self.device.cmd_push_constants(
                command_buffer,
                wctx.pipeline.layout(),
                vk::ShaderStageFlags::VERTEX,
                0,
                std::slice::from_raw_parts(
                    camera_data as *const CameraData as *const u8,
                    std::mem::size_of::<CameraData>(),
                ),
            );
        }

        // Upload draw commands to the indirect command buffer
        let draw_commands = self.environment.as_ref().unwrap().draw_commands();
        let indirect_command_buffer = &self.indirect_command_buffer.as_ref().unwrap()[self.frame_index];
        indirect_command_buffer.upload(&draw_commands, 0);

        // Bind vertex and index buffers
        let vertex_buffer = self.environment.as_ref().unwrap().vertex_buffer();
        let index_buffer = self.environment.as_ref().unwrap().index_buffer();
        unsafe {
            self.device.cmd_bind_vertex_buffers(
                command_buffer,
                0,
                &[vertex_buffer.handle()],
                &[0],
            );
            self.device.cmd_bind_index_buffer(
                command_buffer,
                index_buffer.handle(),
                0,
                vk::IndexType::UINT32,
            );
        }

        // bind descriptor sets
        unsafe {
            self.device.cmd_bind_descriptor_sets(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                wctx.pipeline.layout(),
                0,
                &self.descriptor_set,
                &[],
            );
        }

        unsafe {
            self.device.cmd_draw_indexed_indirect(
                command_buffer,
                indirect_command_buffer.handle(),
                0,
                draw_commands.len() as u32,
                std::mem::size_of::<vk::DrawIndexedIndirectCommand>() as u32
            );
        }

        // end render pass
        unsafe {
            self.device.cmd_end_render_pass(command_buffer);
        }

        // End command buffer recording
        unsafe {
            self.device
                .end_command_buffer(command_buffer)
                .expect("Failed to end command buffer");
        }

        let wait_semaphores = [self.present_complete_semaphores[self.frame_index]];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = [command_buffer];
        let signal_semaphores = [wctx.rendering_complete_semaphores[image_index as usize]];

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

        let present_result = unsafe {
            self.swapchain_loader
                .queue_present(self.queue, &present_info)
        };

        if let Err(err) = present_result {
            if err == vk::Result::ERROR_OUT_OF_DATE_KHR || err == vk::Result::SUBOPTIMAL_KHR {
                self.wctx.as_mut().unwrap().swapchain_out_of_date = true;
            } else {
                panic!("Failed to present swapchain image: {err:?}");
            }
        }

        self.frame_index = (self.frame_index + 1) % MAX_FRAMES_IN_FLIGHT;
    }
}

impl Drop for WindowContext {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().expect("Failed to wait for device idle");
            self.rendering_complete_semaphores
                .iter()
                .for_each(|&semaphore| {
                    self.device.destroy_semaphore(semaphore, None);
                });
            // Destroy framebuffers created for the swapchain first
            self.framebuffers.iter().for_each(|&fb| {
                if fb != vk::Framebuffer::null() {
                    self.device.destroy_framebuffer(fb, None);
                }
            });
            self.depth_image_views.iter().for_each(|&image_view| {
                if image_view != vk::ImageView::null() {
                    self.device.destroy_image_view(image_view, None);
                }
            });
            self.depth_images.iter().for_each(|&image| {
                if image != vk::Image::null() {
                    self.device.destroy_image(image, None);
                }
            });
            self.depth_image_memories.iter().for_each(|&memory| {
                if memory != vk::DeviceMemory::null() {
                    self.device.free_memory(memory, None);
                }
            });
            // Then destroy image views
            self.swapchain_image_views.iter().for_each(|&image_view| {
                if image_view != vk::ImageView::null() {
                    self.device.destroy_image_view(image_view, None);
                }
            });
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.device.destroy_render_pass(self.render_pass, None);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {

            self.device
                .device_wait_idle()
                .expect("Failed to wait for device idle");

            // Drop the environment first to free GPU resources
            self.environment = None;
            self.wctx = None;
            self.indirect_command_buffer = None;

            // clean descriptor stuff
            self.device.destroy_descriptor_pool(self.descriptor_pool, None);
            self.device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);

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
