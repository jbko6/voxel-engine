use std::sync::Arc;

use crate::v2::{Renderer, RenderingContext};

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
    vertex_buffer: vk::Buffer,
    vertex_buffer_allocation: vk_mem::Allocation,
    vertex_block: vk_mem::VirtualBlock,
    index_buffer: vk::Buffer,
    index_buffer_allocation: vk_mem::Allocation,
    index_block: vk_mem::VirtualBlock,
    normal_buffer: vk::Buffer,
    normal_buffer_allocation: vk_mem::Allocation,
    normal_block: vk_mem::VirtualBlock,
    color_buffer: vk::Buffer,
    color_buffer_allocation: vk_mem::Allocation,
    color_block: vk_mem::VirtualBlock,
}

impl GlobalBuffer {
    pub(crate) fn new(allocator: &vk_mem::Allocator) -> GlobalBuffer {
        let vertex_buffer_info = vk::BufferCreateInfo::default()
            .size(4 * 3 * MAX_VERTICES)
            .usage(vk::BufferUsageFlags::STORAGE_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let vertices_alloc_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::Auto,
            flags: vk_mem::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE,
            ..Default::default()
        };
        let (vertex_buffer, vertex_buffer_allocation) = unsafe {
            allocator
                .create_buffer(&vertex_buffer_info, &vertices_alloc_info)
                .unwrap()
        };

        let vertices_block_info = vk_mem::VirtualBlockCreateInfo {
            size: 4 * 3 * MAX_VERTICES,
            flags: vk_mem::VirtualBlockCreateFlags::empty(),
            ..Default::default()
        };
        let vertex_block = vk_mem::VirtualBlock::new(vertices_block_info).unwrap();

        let index_buffer_info = vk::BufferCreateInfo::default()
            .size(4 * MAX_INDICES)
            .usage(vk::BufferUsageFlags::INDEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let indices_alloc_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::Auto,
            flags: vk_mem::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE,
            ..Default::default()
        };
        let (index_buffer, index_buffer_allocation) = unsafe {
            allocator
                .create_buffer(&index_buffer_info, &indices_alloc_info)
                .unwrap()
        };
        let indices_block_info = vk_mem::VirtualBlockCreateInfo {
            size: 4 * MAX_INDICES,
            flags: vk_mem::VirtualBlockCreateFlags::empty(),
            ..Default::default()
        };
        let index_block = vk_mem::VirtualBlock::new(indices_block_info).unwrap();

        let normal_buffer_info = vk::BufferCreateInfo::default()
            .size(4 * 3 * MAX_VERTICES)
            .usage(vk::BufferUsageFlags::STORAGE_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let normals_alloc_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::Auto,
            flags: vk_mem::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE,
            ..Default::default()
        };
        let (normal_buffer, normal_buffer_allocation) = unsafe {
            allocator
                .create_buffer(&normal_buffer_info, &normals_alloc_info)
                .unwrap()
        };
        let normals_block_info = vk_mem::VirtualBlockCreateInfo {
            size: 4 * 3 * MAX_VERTICES,
            flags: vk_mem::VirtualBlockCreateFlags::empty(),
            ..Default::default()
        };
        let normal_block = vk_mem::VirtualBlock::new(normals_block_info).unwrap();

        let color_buffer_info = vk::BufferCreateInfo::default()
            .size(4 * 3 * MAX_VERTICES)
            .usage(vk::BufferUsageFlags::STORAGE_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let colors_alloc_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::Auto,
            flags: vk_mem::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE,
            ..Default::default()
        };
        let (color_buffer, color_buffer_allocation) = unsafe {
            allocator
                .create_buffer(&color_buffer_info, &colors_alloc_info)
                .unwrap()
        };
        let colors_block_info = vk_mem::VirtualBlockCreateInfo {
            size: 4 * 3 * MAX_VERTICES,
            flags: vk_mem::VirtualBlockCreateFlags::empty(),
            ..Default::default()
        };
        let color_block = vk_mem::VirtualBlock::new(colors_block_info).unwrap();

        GlobalBuffer {
            vertex_buffer,
            vertex_buffer_allocation,
            vertex_block,
            index_buffer,
            index_buffer_allocation,
            index_block,
            normal_buffer,
            normal_buffer_allocation,
            normal_block,
            color_buffer,
            color_buffer_allocation,
            color_block,
        }
    }

    pub(crate) fn get_buffer(&self, buffer_type: BufferType) -> vk::Buffer {
        match buffer_type {
            BufferType::VERTEX => self.vertex_buffer,
            BufferType::INDEX => self.index_buffer,
            BufferType::NORMAL => self.normal_buffer,
            BufferType::COLOR => self.color_buffer,
        }
    }

    pub(crate) fn get_allocation_mut(
        &mut self,
        buffer_type: BufferType,
    ) -> &mut vk_mem::Allocation {
        match buffer_type {
            BufferType::VERTEX => &mut self.vertex_buffer_allocation,
            BufferType::INDEX => &mut self.index_buffer_allocation,
            BufferType::NORMAL => &mut self.normal_buffer_allocation,
            BufferType::COLOR => &mut self.color_buffer_allocation,
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
        allocator: &vk_mem::Allocator,
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

        // Map the buffer memory and copy the data
        let allocation = self.get_allocation_mut(buffer_type);
        unsafe {
            let mapped_ptr = allocator
                .map_memory(allocation)
                .expect("Failed to map memory");
            std::ptr::copy_nonoverlapping(data.as_ptr(), mapped_ptr.add(offset as usize) as *mut T, data.len());
            println!("Copied {} bytes to buffer", data_size);
            allocator
                .unmap_memory(allocation);
            allocator
                .flush_allocation(allocation, 0, vk::WHOLE_SIZE)
                .unwrap();
        }

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

    pub(crate) fn free(
        &mut self,
        allocator: &vk_mem::Allocator,
    ) {
        unsafe {
            self.vertex_block.clear();
            self.index_block.clear();
            self.normal_block.clear();
            self.color_block.clear();
            allocator.destroy_buffer(self.vertex_buffer, &mut self.vertex_buffer_allocation);
            allocator.destroy_buffer(self.index_buffer, &mut self.index_buffer_allocation);
            allocator.destroy_buffer(self.normal_buffer, &mut self.normal_buffer_allocation);
            allocator.destroy_buffer(self.color_buffer, &mut self.color_buffer_allocation);
        }
    }
}
