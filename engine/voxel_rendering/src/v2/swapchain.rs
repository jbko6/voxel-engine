use std::sync::Arc;

use ash::vk;

use crate::v2::{Image, ImageView, Renderer, RenderingContext};

pub(crate) struct Swapchain {
    pub context: Arc<RenderingContext>,
    pub handle: vk::SwapchainKHR,
    pub images: Vec<vk::Image>,
    pub image_views: Vec<vk::ImageView>,
    pub depth_image: Image,
    pub depth_image_view: ImageView,
    pub render_finished_semaphores: Vec<vk::Semaphore>,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
}

impl Swapchain {
    pub fn init(
        context: Arc<RenderingContext>,
        surface: vk::SurfaceKHR,
        width: u32,
        height: u32,
    ) -> Result<Self, ()> {
        if width == 0 || height == 0 {
            return Err(()); // minimized — skip, don't try to create a 0-size swapchain
        }

        let surface_formats = unsafe {
            context.surface_instance.get_physical_device_surface_formats(context.pdevice, surface).unwrap()
        };
        let surface_format = surface_formats
            .iter()
            .find(|f| f.format == vk::Format::B8G8R8A8_UNORM
                && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .unwrap_or(&surface_formats[0])
            .clone();

        let capabilities = unsafe {
            context.surface_instance.get_physical_device_surface_capabilities(context.pdevice, surface).unwrap()
        };

        let extent = if capabilities.current_extent.width != u32::MAX {
            capabilities.current_extent
        } else {
            vk::Extent2D {
                width: width.clamp(capabilities.min_image_extent.width, capabilities.max_image_extent.width),
                height: height.clamp(capabilities.min_image_extent.height, capabilities.max_image_extent.height),
            }
        };

        if extent.width == 0 || extent.height == 0 {
            return Err(()); // minimized — skip, don't try to create a 0-size swapchain
        }

        let mut image_count = capabilities.min_image_count + 1;
        if capabilities.max_image_count > 0 {
            image_count = image_count.min(capabilities.max_image_count);
        }

        let present_modes = unsafe {
            context.surface_instance.get_physical_device_surface_present_modes(context.pdevice, surface).unwrap()
        };
        let present_mode = present_modes
            .iter()
            .find(|&&m| m == vk::PresentModeKHR::MAILBOX)
            .copied()
            .unwrap_or(vk::PresentModeKHR::FIFO); // FIFO is always guaranteed

        let swapchain_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true);

        let handle = unsafe { context.swapchain_device.create_swapchain(&swapchain_info, None).unwrap() };
        let images = unsafe { context.swapchain_device.get_swapchain_images(handle).unwrap() };

        let image_views = images.iter().map(|&image| {
            let view_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(surface_format.format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            unsafe { context.device.create_image_view(&view_info, None).unwrap() }
        }).collect();

        let depth_image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::D32_SFLOAT)
            .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT);
        let depth_image_create_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::AutoPreferDevice,
            ..Default::default()
        };
        let depth_image = Renderer::create_image_ctx(context.clone(), depth_image_info, depth_image_create_info);
        let depth_image_view = depth_image.create_image_view(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::DEPTH,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });

        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let render_finished_semaphores = images.iter().map(|_| {
            unsafe { context.device.create_semaphore(&semaphore_info, None).unwrap() }
        }).collect();

        Ok(Self { context, handle, images, image_views, depth_image, depth_image_view, format: surface_format.format, extent, render_finished_semaphores })
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            self.context.device.device_wait_idle().unwrap();
            for &semaphore in &self.render_finished_semaphores {
                self.context.device.destroy_semaphore(semaphore, None);
            }
            for &image_view in &self.image_views {
                self.context.device.destroy_image_view(image_view, None);
            }
            self.context.swapchain_device.destroy_swapchain(self.handle, None);
        }
    }
}