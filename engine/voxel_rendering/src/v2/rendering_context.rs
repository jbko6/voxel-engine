use std::{
    borrow::Cow,
    ffi::{CStr, c_char},
};

use ash::{
    ext::{buffer_device_address, debug_utils, validation_features}, khr::{create_renderpass2, depth_stencil_resolve, dynamic_rendering, push_descriptor, shader_draw_parameters, swapchain, synchronization2}, vk::{self, KHR_BUFFER_DEVICE_ADDRESS_NAME, KHR_SHADER_DRAW_PARAMETERS_NAME},
};
use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

use crate::v2::{GPUCamera, GlobalBuffer};

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

pub(crate) struct RenderingContext {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub surface_instance: ash::khr::surface::Instance,
    pub swapchain_instance: ash::khr::swapchain::Instance,
    pub swapchain_device: ash::khr::swapchain::Device,
    pub push_descriptor_device: ash::khr::push_descriptor::Device,
    pub synchronization2_device: ash::khr::synchronization2::Device,
    pub dynamic_rendering_device: ash::khr::dynamic_rendering::Device,
    debug_utils_instance: ash::ext::debug_utils::Instance,
    debug_call_back: vk::DebugUtilsMessengerEXT,
    pub pdevice: vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue: vk::Queue,
    pub queue_family_index: u32,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
    pub command_pool: vk::CommandPool,
    pub staging_cmd_buffer: vk::CommandBuffer,
    pub pipeline_layout: vk::PipelineLayout,
    allocator: Option<vk_mem::Allocator>,
}

impl RenderingContext {
    pub fn init(window: sdl3::video::Window) -> Self {
        // Entry
        let display_handle = window.display_handle().unwrap().as_raw();
        let entry = unsafe { ash::Entry::load().unwrap() };

        // Instance
        let layer_names = [c"VK_LAYER_KHRONOS_validation"];
        let layer_names_raw: Vec<*const c_char> =
            layer_names.iter().map(|&name| name.as_ptr()).collect();

        let mut extension_names = ash_window::enumerate_required_extensions(display_handle)
            .unwrap()
            .to_vec();
        extension_names.push(debug_utils::NAME.as_ptr());

        let app_info = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_3);

        let instance_info = vk::InstanceCreateInfo::default()
            .enabled_layer_names(&layer_names_raw)
            .enabled_extension_names(&extension_names)
            .application_info(&app_info);

        let instance = unsafe { entry.create_instance(&instance_info, None).unwrap() };

        // Physical Device
        let pdevices = unsafe { instance.enumerate_physical_devices().unwrap() };
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
                            let supports_presentation = match display_handle {
                                RawDisplayHandle::Windows(_) => {
                                    let win32_instance =
                                        ash::khr::win32_surface::Instance::new(&entry, &instance);
                                    win32_instance.get_physical_device_win32_presentation_support(
                                        *pdevice,
                                        index as u32,
                                    )
                                },
                                RawDisplayHandle::Wayland(handle) => {
                                    let wayland_instance =
                                        ash::khr::wayland_surface::Instance::new(&entry, &instance);
                                    wayland_instance.get_physical_device_wayland_presentation_support(
                                        *pdevice,
                                        index as u32,
                                        &mut *(handle.display.as_ptr() as *mut std::os::raw::c_void),
                                    )
                                },
                                _ => false,
                            };
                            if supports_graphics && supports_presentation {
                                Some((*pdevice, index as u32))
                            } else {
                                None
                            }
                        })
                })
                .expect("No available physical device")
        };

        let pdevice_properties = unsafe { instance.get_physical_device_properties(pdevice) };
        println!(
            "Selected device: {} (API version: {}.{}.{})",
            pdevice_properties.device_name_as_c_str().unwrap().to_string_lossy(),
            vk::api_version_major(pdevice_properties.api_version),
            vk::api_version_minor(pdevice_properties.api_version),
            vk::api_version_patch(pdevice_properties.api_version),
        );

        assert!(
            pdevice_properties.api_version >= vk::API_VERSION_1_3,
            "Selected physical device does not support Vulkan 1.3"
        );

        let mut supported_features12 = vk::PhysicalDeviceVulkan12Features::default();
        let mut supported_features13 = vk::PhysicalDeviceVulkan13Features::default();
        let mut supported_features2 = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut supported_features12)
            .push_next(&mut supported_features13);

        unsafe {
            instance.get_physical_device_features2(pdevice, &mut supported_features2);
        }

        assert!(
            supported_features12.buffer_device_address == vk::TRUE,
            "Device does not support bufferDeviceAddress"
        );
        assert!(
            supported_features13.dynamic_rendering == vk::TRUE,
            "Device does not support dynamicRendering"
        );
        assert!(
            supported_features13.synchronization2 == vk::TRUE,
            "Device does not support synchronization2"
        );

        // Device
        let queue_priorities = [1.0];
        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priorities);
        let queue_infos = [queue_info];

        let supported_extensions = unsafe {
            instance
                .enumerate_device_extension_properties(pdevice)
                .unwrap()
        };
        let supported_extension_names: std::collections::HashSet<String> = supported_extensions
            .iter()
            .map(|ext| ext.extension_name_as_c_str().unwrap().to_string_lossy().into_owned())
            .collect();

        let required_extensions = [
            create_renderpass2::NAME,
            depth_stencil_resolve::NAME,
            dynamic_rendering::NAME,
            swapchain::NAME,
            push_descriptor::NAME,
            vk::KHR_BUFFER_DEVICE_ADDRESS_NAME,
            vk::KHR_MAINTENANCE1_NAME,
            synchronization2::NAME,
        ];

        for ext in required_extensions {
            let name = ext.to_string_lossy();
            assert!(
                supported_extension_names.contains(name.as_ref()),
                "Required device extension not supported: {}",
                name
            );
        }

        let device_extensions = required_extensions
            .iter()
            .map(|&ext| ext.as_ptr())
            .collect::<Vec<*const c_char>>();

        let features = vk::PhysicalDeviceFeatures {
            multi_draw_indirect: 1,
            ..Default::default()
        };
        let mut base_features = vk::PhysicalDeviceFeatures2::default()
            .features(features);
        
        let mut buffer_device_address_features =
            vk::PhysicalDeviceBufferDeviceAddressFeaturesKHR::default()
                .buffer_device_address(true);

        let mut dynamic_rendering_features =
            vk::PhysicalDeviceDynamicRenderingFeaturesKHR::default()
                .dynamic_rendering(true);

        let mut synchronization2_features =
            vk::PhysicalDeviceSynchronization2FeaturesKHR::default()
                .synchronization2(true);

        let device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_infos)
            .enabled_extension_names(&device_extensions)
            .push_next(&mut base_features)
            .push_next(&mut buffer_device_address_features)
            .push_next(&mut dynamic_rendering_features)
            .push_next(&mut synchronization2_features);
        let device = unsafe { instance.create_device(pdevice, &device_info, None).unwrap() };

        // Queue
        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        // Allocator
        let mut allocator_info = vk_mem::AllocatorCreateInfo::new(&instance, &device, pdevice.clone());
        allocator_info.vulkan_api_version = vk::API_VERSION_1_3;
        allocator_info.flags = vk_mem::AllocatorCreateFlags::BUFFER_DEVICE_ADDRESS;
        let allocator = unsafe { vk_mem::Allocator::new(allocator_info).unwrap() };

        // Loaders
        let surface_instance = ash::khr::surface::Instance::new(&entry, &instance);
        let swapchain_instance = ash::khr::swapchain::Instance::new(&entry, &instance);
        let swapchain_device = ash::khr::swapchain::Device::new(&instance, &device);
        let push_descriptor_device = ash::khr::push_descriptor::Device::new(&instance, &device);
        let synchronization2_device = ash::khr::synchronization2::Device::new(&instance, &device);
        let dynamic_rendering_device = ash::khr::dynamic_rendering::Device::new(&instance, &device);

        // Debug
        let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                    | vk::DebugUtilsMessageTypeFlagsEXT::GENERAL,
            )
            .pfn_user_callback(Some(vulkan_debug_callback));
        let debug_utils_instance = ash::ext::debug_utils::Instance::new(&entry, &instance);
        let debug_call_back = unsafe {
            debug_utils_instance
                .create_debug_utils_messenger(&debug_info, None)
                .expect("Failed to create debug callback")
        };

        // Command Pool
        let command_pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let command_pool = unsafe {
            device
                .create_command_pool(&command_pool_info, None)
                .unwrap()
        };

        // Staging Command Buffer
        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let staging_cmd_buffer = unsafe {
            device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .unwrap()
                .into_iter()
                .next()
                .unwrap()
        };

        // Descriptor Set Layout
        let descriptor_layout_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX),
            vk::DescriptorSetLayoutBinding::default()
                .binding(3)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX),
        ];

        let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(&descriptor_layout_bindings)
            .flags(vk::DescriptorSetLayoutCreateFlags::PUSH_DESCRIPTOR_KHR);
        let descriptor_set_layout = unsafe {
            device
                .create_descriptor_set_layout(&descriptor_set_layout_create_info, None)
                .unwrap()
        };

        // Pipeline Layout
        let set_layouts = [
            descriptor_set_layout
        ];
        let push_constants = [
            vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .offset(0)
            .size(std::mem::size_of::<GPUCamera>() as u32)
        ];
        let pipeline_layout_info =
            vk::PipelineLayoutCreateInfo::default()
                .set_layouts(&set_layouts)
                .push_constant_ranges(&push_constants);
        let pipeline_layout = unsafe {
            device
                .create_pipeline_layout(&pipeline_layout_info, None)
                .expect("Failed to create pipeline layout")
        };


        println!("Vulkan context initialized successfully.");

        RenderingContext {
            entry,
            instance,
            pdevice,
            device,
            surface_instance,
            swapchain_instance,
            swapchain_device,
            push_descriptor_device,
            synchronization2_device,
            dynamic_rendering_device,
            debug_utils_instance,
            debug_call_back,
            allocator: Some(allocator),
            queue,
            queue_family_index,
            descriptor_set_layout,
            command_pool,
            staging_cmd_buffer,
            pipeline_layout,
        }
    }

    pub fn allocator(&self) -> &vk_mem::Allocator {
        self.allocator.as_ref().unwrap()
    }
}

impl Drop for RenderingContext {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();

            // Drop allocator first
            self.allocator = None;

            // Drop debug callback
            self.debug_utils_instance
                .destroy_debug_utils_messenger(self.debug_call_back, None);

            // Drop pipeline layout
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);

            // Drop descriptor set layout
            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);

            // Drop staging command buffer
            self.device
                .free_command_buffers(self.command_pool, &[self.staging_cmd_buffer]);

            // Drop command pool
            self.device.destroy_command_pool(self.command_pool, None);

            // Drop device and instance
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}
