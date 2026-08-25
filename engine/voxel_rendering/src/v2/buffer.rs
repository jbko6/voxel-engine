use std::sync::{Arc, Mutex, OnceLock};

use crate::v2::{Renderer, RenderingContext};

use ash::vk;
use vk_mem::Alloc;

static STAGING_BUFFER: OnceLock<Mutex<Buffer>> = OnceLock::new();

pub(crate) struct Buffer {
    context: Arc<RenderingContext>,
    requires_staging: bool,
    pub(crate) handle: vk::Buffer,
    pub(crate) allocation: vk_mem::Allocation,
}

impl Buffer {
    pub fn init_staging_buffer(context: Arc<RenderingContext>) {
        let staging_buffer = Buffer::new(
            context,
            &vk::BufferCreateInfo::default()
                .size(1024 * 1024) // 1 MB staging buffer
                .usage(vk::BufferUsageFlags::TRANSFER_SRC)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            &vk_mem::AllocationCreateInfo {
                usage: vk_mem::MemoryUsage::Auto,
                flags: vk_mem::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE | vk_mem::AllocationCreateFlags::MAPPED,
                ..Default::default()
            },
        );
        STAGING_BUFFER.set(Mutex::new(staging_buffer)).ok().expect("Staging buffer already initialized");
    }

    pub fn new(context: Arc<RenderingContext>, info: &vk::BufferCreateInfo, alloc_info: &vk_mem::AllocationCreateInfo) -> Buffer {
        let (handle, allocation) = unsafe {
            context.allocator().create_buffer(info, alloc_info).unwrap()
        };

        Buffer {
            context,
            requires_staging: !alloc_info.flags.contains(vk_mem::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE),
            handle,
            allocation,
        }
    }

    pub fn upload<T>(&mut self, data: &[T]) {
        if self.requires_staging {
            println!("Uploading data using staging buffer");
            // Upload data to staging buffer
            let mut staging_buffer = STAGING_BUFFER.get().unwrap().lock().unwrap();
            staging_buffer.upload(data);

            // Copy data from staging buffer to this buffer
            let cmd_begin_info = vk::CommandBufferBeginInfo::default();
            unsafe {
                self.context.device.begin_command_buffer(self.context.staging_cmd_buffer, &cmd_begin_info).unwrap();
            }

            let copy_region = vk::BufferCopy {
                src_offset: 0,
                dst_offset: 0,
                size: std::mem::size_of_val(data) as u64,
            };
            unsafe {
                self.context.device.cmd_copy_buffer(
                    self.context.staging_cmd_buffer,
                    staging_buffer.handle,
                    self.handle,
                    &[copy_region],
                );
            }

            let cmd_buffers = [self.context.staging_cmd_buffer];
            let submit_info = vk::SubmitInfo::default()
                    .command_buffers(&cmd_buffers);
            unsafe {
                self.context.device.end_command_buffer(self.context.staging_cmd_buffer).unwrap();
                
                self.context.device.queue_submit(self.context.queue, &[submit_info], vk::Fence::null()).unwrap();

                // TODO: use a fence instead of waiting for the queue to be idle
                self.context.device.queue_wait_idle(self.context.queue).unwrap();
            }
        } else {
            println!("Uploading data directly to buffer");
            // Upload data directly to the buffer
            let data_size = std::mem::size_of_val(data) as u64;
            unsafe {
                let ptr = self.context.allocator().map_memory(&mut self.allocation).unwrap();
                std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut T, data.len());
                self.context.allocator().flush_allocation(&mut self.allocation, 0, data_size).unwrap();
                self.context.allocator().unmap_memory(&mut self.allocation);
            }
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            self.context.allocator().destroy_buffer(self.handle, &mut self.allocation);
        }
    }
}