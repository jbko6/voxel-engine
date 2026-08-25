use std::sync::Arc;

use crate::v2::{Buffer, Renderer, RenderingContext};

use ash::vk;
use vk_mem::Alloc;

const MAX_VERTICES: u64 = 100_000;
const MAX_INDICES: u64 = 400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BufferType {
    VERTEX,
    INDEX,
    NORMAL,
    COLOR,
}

pub(crate) struct GlobalBuffer {
    vertex_buffer: Buffer,
    vertex_block: vk_mem::VirtualBlock,
    index_buffer: Buffer,
    index_block: vk_mem::VirtualBlock,
    normal_buffer: Buffer,
    normal_block: vk_mem::VirtualBlock,
    color_buffer: Buffer,
    color_block: vk_mem::VirtualBlock,
}

impl GlobalBuffer {
    pub(crate) fn new(context: Arc<RenderingContext>, alloc_info: &vk_mem::AllocationCreateInfo) -> GlobalBuffer {
        let vertex_buffer_info = vk::BufferCreateInfo::default()
            .size(4 * 3 * MAX_VERTICES)
            .usage(vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let vertex_buffer = Buffer::new(context.clone(), &vertex_buffer_info, alloc_info);

        let vertices_block_info = vk_mem::VirtualBlockCreateInfo {
            size: 4 * 3 * MAX_VERTICES,
            flags: vk_mem::VirtualBlockCreateFlags::empty(),
            ..Default::default()
        };
        let vertex_block = vk_mem::VirtualBlock::new(vertices_block_info).unwrap();

        let index_buffer_info = vk::BufferCreateInfo::default()
            .size(4 * MAX_INDICES)
            .usage(vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let index_buffer = Buffer::new(context.clone(), &index_buffer_info, alloc_info);
        let indices_block_info = vk_mem::VirtualBlockCreateInfo {
            size: 4 * MAX_INDICES,
            flags: vk_mem::VirtualBlockCreateFlags::empty(),
            ..Default::default()
        };
        let index_block = vk_mem::VirtualBlock::new(indices_block_info).unwrap();

        let normal_buffer_info = vk::BufferCreateInfo::default()
            .size(4 * 3 * MAX_VERTICES)
            .usage(vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let normal_buffer = Buffer::new(context.clone(), &normal_buffer_info, alloc_info);
        let normals_block_info = vk_mem::VirtualBlockCreateInfo {
            size: 4 * 3 * MAX_VERTICES,
            flags: vk_mem::VirtualBlockCreateFlags::empty(),
            ..Default::default()
        };
        let normal_block = vk_mem::VirtualBlock::new(normals_block_info).unwrap();

        let color_buffer_info = vk::BufferCreateInfo::default()
            .size(4 * 3 * MAX_VERTICES)
            .usage(vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let color_buffer = Buffer::new(context.clone(), &color_buffer_info, alloc_info);
        let colors_block_info = vk_mem::VirtualBlockCreateInfo {
            size: 4 * 3 * MAX_VERTICES,
            flags: vk_mem::VirtualBlockCreateFlags::empty(),
            ..Default::default()
        };
        let color_block = vk_mem::VirtualBlock::new(colors_block_info).unwrap();

        GlobalBuffer {
            vertex_buffer,
            vertex_block,
            index_buffer,
            index_block,
            normal_buffer,
            normal_block,
            color_buffer,
            color_block,
        }
    }

    pub(crate) fn get_buffer(&self, buffer_type: BufferType) -> &Buffer {
        match buffer_type {
            BufferType::VERTEX => &self.vertex_buffer,
            BufferType::INDEX => &self.index_buffer,
            BufferType::NORMAL => &self.normal_buffer,
            BufferType::COLOR => &self.color_buffer,
        }
    }

    pub(crate) fn get_buffer_mut(&mut self, buffer_type: BufferType) -> &mut Buffer {
        match buffer_type {
            BufferType::VERTEX => &mut self.vertex_buffer,
            BufferType::INDEX => &mut self.index_buffer,
            BufferType::NORMAL => &mut self.normal_buffer,
            BufferType::COLOR => &mut self.color_buffer,
        }
    }

    pub(crate) fn get_block_mut(&mut self, buffer_type: BufferType) -> &mut vk_mem::VirtualBlock {
        match buffer_type {
            BufferType::VERTEX => &mut self.vertex_block,
            BufferType::INDEX => &mut self.index_block,
            BufferType::NORMAL => &mut self.normal_block,
            BufferType::COLOR => &mut self.color_block,
        }
    }

    pub(crate) fn init_buffer<T>(
        &mut self,
        buffer_type: BufferType,
        data: &[T],
    ) -> (vk_mem::VirtualAllocation, u64) {
        let data_size = std::mem::size_of_val(data) as u64;
        let block = self.get_block_mut(buffer_type);
        let alloc_info = vk_mem::VirtualAllocationCreateInfo {
            size: data_size,
            alignment: 4,
            flags: vk_mem::VirtualAllocationCreateFlags::empty(),
            user_data: 0,
        };
        let (v_allocation, offset) = unsafe {
            block
                .allocate(alloc_info)
                .expect("Failed to allocate from virtual block")
        };

        self.get_buffer_mut(buffer_type).upload(data);

        (v_allocation, offset)
    }

    pub(crate) fn free_buffer(
        &mut self,
        buffer_type: BufferType,
        v_allocation: &mut vk_mem::VirtualAllocation,
    ) {
        let block = self.get_block_mut(buffer_type);
        unsafe {
            block.free(v_allocation);
        }
    }
}

impl Drop for GlobalBuffer {
    fn drop(&mut self) {
        unsafe {
            self.vertex_block.clear();
            self.index_block.clear();
            self.normal_block.clear();
            self.color_block.clear();
        }
    }
}
