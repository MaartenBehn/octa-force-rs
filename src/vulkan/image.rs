use std::mem::align_of;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use ash::vk::{self};
use gpu_allocator::vulkan::AllocationScheme;
use gpu_allocator::{
    vulkan::{Allocation, AllocationCreateDesc, Allocator},
    MemoryLocation,
};
use log::trace;

use crate::{vulkan::device::Device, Context};
use crate::vulkan::align::Align;

#[derive(Debug)]
pub struct Image {
    device: Arc<Device>,
    allocator: Arc<Mutex<Allocator>>,
    pub(crate) inner: vk::Image,
    allocation: Option<Allocation>,
    pub format: vk::Format,
    pub extent: vk::Extent3D,
    is_swapchain: bool, // if set, image should not be destroyed
}

#[derive(Debug)]
pub struct ImageView {
    device: Arc<Device>,
    pub(crate) inner: vk::ImageView,
}

impl Image {
    pub(crate) fn new_2d(
        device: Arc<Device>,
        allocator: Arc<Mutex<Allocator>>,
        usage: vk::ImageUsageFlags,
        memory_location: MemoryLocation,
        format: vk::Format,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        trace!("Creating Image: {width}x{height} with usage flags {usage:?} at memory location {memory_location:?}");

        let extent = vk::Extent3D {
            width,
            height,
            depth: 1,
        };

        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(extent)
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let inner = unsafe { device.inner.create_image(&image_info, None)? };
        let requirements = unsafe { device.inner.get_image_memory_requirements(inner) };

        let allocation = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: "image",
            requirements,
            location: memory_location,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;

        unsafe {
            device
                .inner
                .bind_image_memory(inner, allocation.memory(), allocation.offset())?
        };

        Ok(Self {
            device,
            allocator,
            inner,
            allocation: Some(allocation),
            format,
            extent,
            is_swapchain: false,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn new_cube(
        device: Arc<Device>,
        allocator: Arc<Mutex<Allocator>>,
        usage: vk::ImageUsageFlags,
        memory_location: MemoryLocation,
        format: vk::Format,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let extent = vk::Extent3D {
            width,
            height,
            depth: 1,
        };

        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(extent)
            .array_layers(6)
            .mip_levels(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let inner = unsafe { device.inner.create_image(&image_info, None)? };
        let requirements = unsafe { device.inner.get_image_memory_requirements(inner) };

        let allocation = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: "image",
            requirements,
            location: memory_location,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;

        unsafe {
            device
                .inner
                .bind_image_memory(inner, allocation.memory(), allocation.offset())?
        };

        Ok(Self {
            device,
            allocator,
            inner,
            allocation: Some(allocation),
            format,
            extent,
            is_swapchain: false,
        })
    }


    pub(crate) fn from_swapchain_image(
        device: Arc<Device>,
        allocator: Arc<Mutex<Allocator>>,
        swapchain_image: vk::Image,
        format: vk::Format,
        extent: vk::Extent2D,
    ) -> Self {
        let extent = vk::Extent3D {
            width: extent.width,
            height: extent.height,
            depth: 1,
        };

        Self {
            device,
            allocator,
            inner: swapchain_image,
            allocation: None,
            format,
            extent,
            is_swapchain: true,
        }
    }

    pub fn create_image_view(&self, is_depth: bool) -> Result<ImageView> {
        let aspect_mask = if !is_depth {
            vk::ImageAspectFlags::COLOR
        } else {
            vk::ImageAspectFlags::DEPTH
        };

        let view_info = vk::ImageViewCreateInfo::default()
            .image(self.inner)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(self.format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        let inner = unsafe { self.device.inner.create_image_view(&view_info, None)? };

        Ok(ImageView {
            device: self.device.clone(),
            inner,
        })
    }

    pub fn copy_data_to_image<T: Copy>(&self, data: &[T]) -> Result<()> {
        self.copy_data_to_image_complex(data, 0, align_of::<T>())
    }

    pub fn copy_data_to_image_complex<T: Copy>(
        &self,
        data: &[T],
        offset: usize,
        alignment: usize,
    ) -> Result<()> {
        unsafe {
            let data_ptr = self
                .allocation
                .as_ref()
                .unwrap()
                .mapped_ptr()
                .unwrap()
                .as_ptr();

            let mut align: Align<T> = Align::new(data_ptr, alignment as _, data.len(), offset);
            align.copy_from_slice(data);
        };

        Ok(())
    }
}

impl Context {
    pub fn create_image(
        &self,
        usage: vk::ImageUsageFlags,
        memory_location: MemoryLocation,
        format: vk::Format,
        width: u32,
        height: u32,
    ) -> Result<Image> {
        Image::new_2d(
            self.device.clone(),
            self.allocator.clone(),
            usage,
            memory_location,
            format,
            width,
            height,
        )
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        if self.is_swapchain {
            return;
        }

        unsafe { self.device.inner.destroy_image(self.inner, None) };
        self.allocator
            .lock()
            .unwrap()
            .free(self.allocation.take().unwrap())
            .unwrap();
    }
}

impl Drop for ImageView {
    fn drop(&mut self) {
        unsafe { self.device.inner.destroy_image_view(self.inner, None) };
    }
}

#[derive(Debug)]
pub struct ImageAndView {
    pub view: ImageView,
    pub image: Image,
}

