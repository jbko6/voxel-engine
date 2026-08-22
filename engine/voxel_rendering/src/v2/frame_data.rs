use ash::vk;
use vk_mem::Alloc;

use std::sync::Arc;

use crate::v2::RenderingContext;

pub(crate) struct FrameData {
    pub context: Arc<RenderingContext>,
    pub command_buffer: vk::CommandBuffer,
    pub image_available_semaphore: vk::Semaphore,
    pub in_flight_fence: vk::Fence,
    pub indirect_buffer: vk::Buffer,
    pub indirect_buffer_allocation: vk_mem::Allocation,
}

impl FrameData {
    pub fn init(
        context: Arc<RenderingContext>,
    ) -> Self {
        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(context.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let command_buffer = unsafe {
            context
                .device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .unwrap()[0]
        };

        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

        let image_available_semaphore = unsafe {
            context
                .device
                .create_semaphore(&semaphore_info, None)
                .unwrap()
        };
        let in_flight_fence = unsafe { context.device.create_fence(&fence_info, None).unwrap() };

        let indirect_buffer_size = std::mem::size_of::<vk::DrawIndirectCommand>() * 1000; // space for 1000 indirect draw commands
        let (indirect_buffer, indirect_buffer_allocation) = unsafe {
            context.allocator().create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(indirect_buffer_size as u64)
                    .usage(vk::BufferUsageFlags::INDIRECT_BUFFER)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                &vk_mem::AllocationCreateInfo {
                    usage: vk_mem::MemoryUsage::Auto,
                    flags: vk_mem::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
            ).unwrap()
        };

        Self {
            context: context.clone(),
            command_buffer,
            image_available_semaphore,
            in_flight_fence,
            indirect_buffer,
            indirect_buffer_allocation,
        }
    }
}

impl Drop for FrameData {
    fn drop(&mut self) {
        unsafe {
            self.context.device.destroy_semaphore(self.image_available_semaphore, None);
            self.context.device.destroy_fence(self.in_flight_fence, None);
            self.context.device.free_command_buffers(self.context.command_pool, &[self.command_buffer]);
            self.context.allocator().destroy_buffer(self.indirect_buffer, &mut self.indirect_buffer_allocation);
        }
    }
}