use std::sync::Arc;

use ash::vk;
use vk_mem::Alloc;

use crate::v2::{Renderer, RenderingContext};

pub struct Image {
    context: Arc<RenderingContext>,
    image: vk::Image,
    allocation: vk_mem::Allocation,
    image_type: vk::ImageType,
    format: vk::Format,
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe {
            self.context
                .allocator()
                .destroy_image(self.image, &mut self.allocation);
        }
    }
}

pub struct ImageView {
    context: Arc<RenderingContext>,
    view: vk::ImageView,
}

impl Drop for ImageView {
    fn drop(&mut self) {
        unsafe {
            self.context.device.destroy_image_view(self.view, None);
        }
    }
}

impl Renderer {

    pub(crate) fn create_image_ctx(
        context: Arc<RenderingContext>,
        image_info: vk::ImageCreateInfo,
        create_info: vk_mem::AllocationCreateInfo,
    ) -> Image {
        let (image, allocation) = unsafe {
            context
                .allocator()
                .create_image(&image_info, &create_info)
                .unwrap()
        };

        Image {
            context: context.clone(),
            image,
            allocation,
            image_type: image_info.image_type,
            format: image_info.format,
        }
    }

    pub fn create_image(
        &self,
        image_info: vk::ImageCreateInfo,
        create_info: vk_mem::AllocationCreateInfo,
    ) -> Image {
        Renderer::create_image_ctx(self.context.clone(), image_info, create_info)
    }
}

fn image_type_to_view_type(image_type: vk::ImageType) -> vk::ImageViewType {
    match image_type {
        vk::ImageType::TYPE_1D => vk::ImageViewType::TYPE_1D,
        vk::ImageType::TYPE_2D => vk::ImageViewType::TYPE_2D,
        vk::ImageType::TYPE_3D => vk::ImageViewType::TYPE_3D,
        _ => panic!("Unsupported image type"),
    }
}

impl Image {
    pub fn create_image_view(&self, subresource_range: vk::ImageSubresourceRange) -> ImageView {
        let view_info = vk::ImageViewCreateInfo::default()
            .image(self.image)
            .view_type(image_type_to_view_type(self.image_type))
            .format(self.format)
            .subresource_range(subresource_range);

        let view = unsafe {
            self.context
                .device
                .create_image_view(&view_info, None)
                .unwrap()
        };

        ImageView {
            context: self.context.clone(),
            view,
        }
    }

    pub fn handle(&self) -> vk::Image {
        self.image
    }
}

impl ImageView {
    pub fn handle(&self) -> vk::ImageView {
        self.view
    }
}