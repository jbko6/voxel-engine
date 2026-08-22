use std::{marker::PhantomData, sync::Arc};

use ash::vk;

use crate::Renderer;

pub enum BufferUsage {
    Vertex,
    Index,
    Uniform,
    Storage,
    Indirect,
}

impl BufferUsage {
    fn to_vk_usage(&self) -> vk::BufferUsageFlags {
        match self {
            BufferUsage::Vertex => vk::BufferUsageFlags::VERTEX_BUFFER,
            BufferUsage::Index => vk::BufferUsageFlags::INDEX_BUFFER,
            BufferUsage::Uniform => vk::BufferUsageFlags::UNIFORM_BUFFER,
            BufferUsage::Storage => vk::BufferUsageFlags::STORAGE_BUFFER,
            BufferUsage::Indirect => vk::BufferUsageFlags::INDIRECT_BUFFER,
        }
    }
}

/// Trait for buffers that can be used to store data for GPU rendering.
pub trait Buffer<T: Sized> {
    fn handle(&self) -> vk::Buffer;
    fn size(&self) -> usize;
    fn upload(&self, data: &[T], offset: u32);
}

/// Opaque struct representing a GPU buffer and its associated memory.
pub struct GPUBuffer {
    pub gpu_buffer: vk::Buffer,
    pub gpu_memory: vk::DeviceMemory,
    pub staging_mem_index: Option<u32>,
    pub device: Arc<ash::Device>,
}

impl Drop for GPUBuffer {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().expect("Failed to wait for device idle before dropping GPUBuffer");
            self.device.destroy_buffer(self.gpu_buffer, None);
            self.device.free_memory(self.gpu_memory, None);
        }
    }
}

/// Buffer with a fixed size.
/// Useful for things that don't change size often
pub struct FixedSizeBuffer<T: Sized, const N: usize> {
    data: PhantomData<T>,
    usage: BufferUsage,
    gpu_buffer: GPUBuffer,
}

impl<T: Sized, const N: usize> Buffer<T> for FixedSizeBuffer<T, N> {
    fn handle(&self) -> vk::Buffer {
        self.gpu_buffer.gpu_buffer
    }

    fn size(&self) -> usize {
        N
    }

    fn upload(&self, data: &[T], offset: u32) {
        assert!(data.len() <= N, "Data length exceeds buffer size");
        assert!((offset as usize) + data.len() <= N, "Offset + data length exceeds buffer size");

        if self.gpu_buffer.staging_mem_index.is_none() {
            // Upload directly to the GPU buffer if it's host visible

            let data_ptr = unsafe {
                self.gpu_buffer.device.map_memory(
                    self.gpu_buffer.gpu_memory,
                    (offset as usize * std::mem::size_of::<T>()) as vk::DeviceSize,
                    (data.len() * std::mem::size_of::<T>()) as vk::DeviceSize,
                    vk::MemoryMapFlags::empty(),
                )
            }.expect("Failed to map GPU buffer memory") as *mut T;

            unsafe {
                data_ptr.copy_from_nonoverlapping(data.as_ptr(), data.len());
                self.gpu_buffer.device.unmap_memory(self.gpu_buffer.gpu_memory);
            }

        } else {

            let staging_buffer_size = (data.len() * std::mem::size_of::<T>()) as vk::DeviceSize;

            let staging_buffer_info = vk::BufferCreateInfo {
                size: staging_buffer_size,
                usage: vk::BufferUsageFlags::TRANSFER_SRC,
                sharing_mode: vk::SharingMode::EXCLUSIVE,
                ..Default::default()
            };

            let staging_buffer = unsafe { self.gpu_buffer.device.create_buffer(&staging_buffer_info, None) }
                .expect("Failed to create staging buffer");

            let mem_requirements = unsafe { self.gpu_buffer.device.get_buffer_memory_requirements(staging_buffer) };
            let mem_index = self.gpu_buffer.staging_mem_index.expect("Staging memory index not set");

            let alloc_info = vk::MemoryAllocateInfo {
                allocation_size: mem_requirements.size,
                memory_type_index: mem_index,
                ..Default::default()
            };

            let staging_buffer_memory = unsafe { self.gpu_buffer.device.allocate_memory(&alloc_info, None) }
                .expect("Failed to allocate staging buffer memory");

            unsafe { self.gpu_buffer.device.bind_buffer_memory(staging_buffer, staging_buffer_memory, 0) }
                .expect("Failed to bind staging buffer memory");

            // Map memory and copy data
            let data_ptr = unsafe {
                self.gpu_buffer.device.map_memory(
                    staging_buffer_memory,
                    0,
                    staging_buffer_size,
                    vk::MemoryMapFlags::empty(),
                )
            }.expect("Failed to map staging buffer memory") as *mut T;

            unsafe {
                data_ptr.copy_from_nonoverlapping(data.as_ptr(), data.len());
                self.gpu_buffer.device.unmap_memory(staging_buffer_memory);
            }

            // TODO: Use a command buffer to copy from the staging buffer to the GPU buffer

            // Cleanup
            unsafe {
                self.gpu_buffer.device.destroy_buffer(staging_buffer, None);
                self.gpu_buffer.device.free_memory(staging_buffer_memory, None);
            }
        }
    }
}

// eventually: pool of staging buffers, so we don't have to create/destroy them every time we want to stage data

/// Buffer with a dynamic size.
/// Supports resizing and reallocation
pub struct DynamicBuffer<T: Sized> {
    data: PhantomData<T>,
    size: usize,
    gpu_buffer: GPUBuffer
}

impl<T: Sized> Buffer<T> for DynamicBuffer<T> {
    fn handle(&self) -> vk::Buffer {
        self.gpu_buffer.gpu_buffer
    }

    fn size(&self) -> usize {
        self.size
    }

    fn upload(&self, data: &[T], offset: u32) {
        if (data.len() + offset as usize) * std::mem::size_of::<T>() > self.size {
            // Resize the buffer if the new data exceeds the current size
            self.resize(self.size + data.len().saturating_sub(offset as usize) * 2);
        }

        if self.gpu_buffer.staging_mem_index.is_none() {
            // Upload directly to the GPU buffer if it's host visible

            let data_ptr = unsafe {
                self.gpu_buffer.device.map_memory(
                    self.gpu_buffer.gpu_memory,
                    (offset as usize * std::mem::size_of::<T>()) as vk::DeviceSize,
                    (data.len() * std::mem::size_of::<T>()) as vk::DeviceSize,
                    vk::MemoryMapFlags::empty(),
                )
            }.expect("Failed to map GPU buffer memory") as *mut T;

            unsafe {
                data_ptr.copy_from_nonoverlapping(data.as_ptr(), data.len());
                self.gpu_buffer.device.unmap_memory(self.gpu_buffer.gpu_memory);
            }

        } else {
            // Similar staging buffer logic as in FixedSizeBuffer
        }
    }
}

impl<T: Sized> DynamicBuffer<T> {
    /// Resize the buffer to a new size.
    fn resize(&self, new_size: usize) {
        // todo later
    }
}

// /// Used for staging data before transferring to GPU buffers
// /// Created from a FixedSizeBuffer or DynamicBuffer, and can be used to transfer data to the GPU buffer
// pub struct StagingBuffer {
//     pub gpu_buffer: GPUBuffer,
// }

pub fn find_mem_index(instance: Arc<ash::Instance>, pdevice: Arc<vk::PhysicalDevice>, type_filter: u32, properties: vk::MemoryPropertyFlags) -> Option<u32> {
    let mem_properties = unsafe {
        instance
            .get_physical_device_memory_properties(*pdevice.as_ref())
    };

    for (i, memory_type) in mem_properties.memory_types.iter().enumerate() {
        if (type_filter & (1 << i)) != 0 && memory_type.property_flags.contains(properties) {
            return Some(i as u32);
        }
    }
    None
}

pub fn create_buffer(instance: Arc<ash::Instance>, pdevice: Arc<vk::PhysicalDevice>, device: Arc<ash::Device>, size: usize, usage: vk::BufferUsageFlags, properties: vk::MemoryPropertyFlags) -> GPUBuffer {
    assert!(size > 0, "Buffer size must be greater than zero");

    let buffer_info = vk::BufferCreateInfo {
        size: size as u64,
        usage,
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        ..Default::default()
    };

    let buffer = unsafe { device.create_buffer(&buffer_info, None) }
        .expect("Failed to create buffer");

    let mem_requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    
    // Find mem index
    let mem_index = find_mem_index(
        instance.clone(), 
        pdevice.clone(), 
        mem_requirements.memory_type_bits, 
        properties
    ).unwrap();

    let alloc_info = vk::MemoryAllocateInfo {
        allocation_size: mem_requirements.size,
        memory_type_index: mem_index,
        ..Default::default()
    };

    let buffer_memory = unsafe { device.allocate_memory(&alloc_info, None) }
        .expect("Failed to allocate buffer memory");

    unsafe { device.bind_buffer_memory(buffer, buffer_memory, 0) }
        .expect("Failed to bind buffer memory");

    let staging_mem_index = if properties.contains(vk::MemoryPropertyFlags::HOST_VISIBLE) {
        None
    } else {
        find_mem_index(
            instance.clone(), 
            pdevice.clone(), 
            mem_requirements.memory_type_bits,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT
        )
    };

    GPUBuffer {
        gpu_buffer: buffer,
        gpu_memory: buffer_memory,
        staging_mem_index,
        device,
    }
}

pub fn create_fixed_size_buffer<T: Sized + Copy, const N: usize>(instance: Arc<ash::Instance>, pdevice: Arc<vk::PhysicalDevice>, device: Arc<ash::Device>, usage: BufferUsage) -> FixedSizeBuffer<T, N> {
    let gpu_buffer = create_buffer(
        instance,
        pdevice,
        device,
        N * std::mem::size_of::<T>(), 
        usage.to_vk_usage() | vk::BufferUsageFlags::TRANSFER_DST,
        // change to device local later
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    );

    FixedSizeBuffer {
        data: PhantomData,
        usage,
        gpu_buffer,
    }
}

pub fn create_dynamic_buffer<T: Sized + Copy>(instance: Arc<ash::Instance>, pdevice: Arc<vk::PhysicalDevice>, device: Arc<ash::Device>, initial_size: usize, usage: BufferUsage) -> DynamicBuffer<T> {
    let gpu_buffer = create_buffer(
        instance,
        pdevice,
        device,
        initial_size * std::mem::size_of::<T>(), 
        usage.to_vk_usage() | vk::BufferUsageFlags::TRANSFER_DST,
        // change to device local later
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    );

    DynamicBuffer {
        data: PhantomData,
        size: initial_size,
        gpu_buffer,
    }
}

/// Buffer creation functions
impl Renderer {
    fn create_buffer(&self, size: usize, usage: vk::BufferUsageFlags, properties: vk::MemoryPropertyFlags) -> GPUBuffer {
        create_buffer(
            self.instance.clone(),
            self.pdevice.clone(),
            self.device.clone(),
            size, 
            usage,
            properties
        )
    }

    pub fn create_fixed_size_buffer<T: Sized + Copy, const N: usize>(&self, usage: BufferUsage) -> FixedSizeBuffer<T, N> {
        create_fixed_size_buffer(
            self.instance.clone(),
            self.pdevice.clone(),
            self.device.clone(),
            usage
        )
    }

    pub fn create_dynamic_buffer<T: Sized + Copy>(&self, initial_size: usize, usage: BufferUsage) -> DynamicBuffer<T> {
        create_dynamic_buffer(
            self.instance.clone(),
            self.pdevice.clone(),
            self.device.clone(),
            initial_size,
            usage
        )
    }
}