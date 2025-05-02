use std::fmt;
use std::sync::Arc;

use anyhow::Result;
use ash::vk;
use ash::vk::{ImageUsageFlags};
use glam::{UVec2, uvec2};
use gpu_allocator::MemoryLocation;

use crate::{vulkan::device::Device, vulkan::Queue, Context, Semaphore};
use crate::vulkan::Image;

use super::ImageAndView;

pub struct AcquiredImage {
    pub index: u32,
    pub is_suboptimal: bool,
}

pub struct Swapchain {
    device: Arc<Device>,
    inner: ash::khr::swapchain::Device,
    swapchain_khr: vk::SwapchainKHR,
    pub size: UVec2,
    pub format: vk::Format,
    pub depth_format: vk::Format,
    pub color_space: vk::ColorSpaceKHR,
    pub present_mode: vk::PresentModeKHR,
    pub images_and_views: Vec<ImageAndView>,
    pub depht_images_and_views: Vec<ImageAndView>,
}

impl Swapchain {
    pub fn new(context: &Context, width: u32, height: u32) -> Result<Self> {
        log::trace!("Creating vulkan swapchain");

        let device = context.device.clone();

        let capabilities = unsafe {
            context
                .surface
                .inner
                .get_physical_device_surface_capabilities(
                    context.physical_device.inner,
                    context.surface.surface_khr,
                )?
        };

        // Swapchain extent
        let extent = {
            if capabilities.current_extent.width != std::u32::MAX {
                capabilities.current_extent
            } else {
                let min = capabilities.min_image_extent;
                let max = capabilities.max_image_extent;
                let width = width.min(max.width).max(min.width);
                let height = height.min(max.height).max(min.height);
                vk::Extent2D { width, height }
            }
        };
        log::info!("Swapchain size: {}x{}", extent.width, extent.height);

        // Swapchain image count
        let image_count = capabilities.min_image_count + 1;
        log::info!("Swapchain image count: {image_count:?}");

        // Swapchain
        let families_indices = [
            context.physical_device.graphics_queue_family.index,
            context.physical_device.present_queue_family.index,
        ];

        let format = context.physical_device.surface_format;
        let depth_format = context.physical_device.depth_format;
        let present_mode = context.physical_device.present_mode;
        
        let create_info = {
            let mut builder = vk::SwapchainCreateInfoKHR::default()
                .surface(context.surface.surface_khr)
                .min_image_count(image_count)
                .image_format(format.format)
                .image_color_space(format.color_space)
                .image_extent(extent)
                .image_array_layers(1)
                .image_usage(
                    vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST,
                );

            builder = if context.physical_device.graphics_queue_family.index != context.physical_device.present_queue_family.index {
                builder
                    .image_sharing_mode(vk::SharingMode::CONCURRENT)
                    .queue_family_indices(&families_indices)
            } else {
                builder.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            };

            builder
                .pre_transform(capabilities.current_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(context.physical_device.present_mode)
                .clipped(true)
        };

        let inner = ash::khr::swapchain::Device::new(&context.instance.inner, &context.device.inner);
        let swapchain_khr = unsafe { inner.create_swapchain(&create_info, None)? };

        // Swapchain images and image views
        let images = unsafe { inner.get_swapchain_images(swapchain_khr)? };
        let images_and_views = images
            .into_iter()
            .map(|i| {
                let image = Image::from_swapchain_image(
                    device.clone(),
                    context.allocator.clone(),
                    i,
                    format.format,
                    extent,
                );
                let view = image.create_image_view(false).unwrap();
                ImageAndView{ view, image }
            })
            .collect::<Vec<_>>();

        let depht_images_and_views = (0..images_and_views.len())
            .into_iter()
            .map(|_| {
                let depth_image = context.create_image(
                    ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                    MemoryLocation::GpuOnly,
                    depth_format,
                    extent.width,
                    extent.height,
                ).unwrap();
                let depth_view = depth_image.create_image_view(true).unwrap();
                ImageAndView{ view: depth_view, image: depth_image }
            })
            .collect::<Vec<_>>();

        Ok(Self {
            device,
            inner,
            swapchain_khr,
            size: uvec2(extent.width, extent.height),
            format: format.format,
            depth_format,
            color_space: format.color_space,
            present_mode,
            images_and_views,
            depht_images_and_views,
        })
    }

    pub fn resize(&mut self, context: &Context, width: u32, height: u32) -> Result<()> {
        log::info!("Resizing vulkan swapchain to {width}x{height}");

        self.destroy();

        let capabilities = unsafe {
            context
                .surface
                .inner
                .get_physical_device_surface_capabilities(
                    context.physical_device.inner,
                    context.surface.surface_khr,
                )?
        };

        // Swapchain extent
        let extent = {
            if capabilities.current_extent.width != std::u32::MAX {
                capabilities.current_extent
            } else {
                let min = capabilities.min_image_extent;
                let max = capabilities.max_image_extent;
                let width = width.min(max.width).max(min.width);
                let height = height.min(max.height).max(min.height);
                vk::Extent2D { width, height }
            }
        };

        // Swapchain image count
        let image_count = capabilities.min_image_count;

        // Swapchain
        let families_indices = [
            context.physical_device.graphics_queue_family.index,
            context.physical_device.present_queue_family.index,
        ];

        let create_info = {
            let mut builder = vk::SwapchainCreateInfoKHR::default()
                .surface(context.surface.surface_khr)
                .min_image_count(image_count)
                .image_format(self.format)
                .image_color_space(self.color_space)
                .image_extent(extent)
                .image_array_layers(1)
                .image_usage(
                    vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST,
                );

            builder = if context.physical_device.graphics_queue_family.index != context.physical_device.present_queue_family.index {
                builder
                    .image_sharing_mode(vk::SharingMode::CONCURRENT)
                    .queue_family_indices(&families_indices)
            } else {
                builder.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            };

            builder
                .pre_transform(capabilities.current_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(self.present_mode)
                .clipped(true)
        };

        let swapchain_khr = unsafe { self.inner.create_swapchain(&create_info, None)? };

        // Swapchain images and image views
        let images = unsafe { self.inner.get_swapchain_images(swapchain_khr)? };
        let images_and_views = images
            .into_iter()
            .map(|i| {
                let image = Image::from_swapchain_image(
                    self.device.clone(),
                    context.allocator.clone(),
                    i,
                    self.format,
                    extent,
                );
                let view = image.create_image_view(false).unwrap();
                ImageAndView{ view, image }
            })
            .collect::<Vec<_>>();

        let depht_images_and_views = (0..images_and_views.len())
            .into_iter()
            .map(|_| {
                let depth_image = context.create_image(
                    ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                    MemoryLocation::GpuOnly,
                    self.depth_format,
                    extent.width,
                    extent.height,
                ).unwrap();
                let depth_view = depth_image.create_image_view(true).unwrap();
                ImageAndView{ view: depth_view, image: depth_image }
            })
            .collect::<Vec<_>>();

        self.swapchain_khr = swapchain_khr;
        self.size = uvec2(extent.width, extent.height);
        self.images_and_views = images_and_views;
        self.depht_images_and_views = depht_images_and_views;

        Ok(())
    }

    pub fn acquire_next_image(&self, timeout: u64, semaphore: &Semaphore) -> Result<AcquiredImage> {
        let (index, is_suboptimal) = unsafe {
            self.inner.acquire_next_image(
                self.swapchain_khr,
                timeout,
                semaphore.inner,
                vk::Fence::null(),
            )?
        };

        Ok(AcquiredImage {
            index,
            is_suboptimal,
        })
    }

    pub fn queue_present(
        &self,
        image_index: u32,
        wait_semaphores: &[&Semaphore],
        queue: &Queue,
    ) -> Result<bool> {
        let swapchains = [self.swapchain_khr];
        let images_indices = [image_index];
        let wait_semaphores = wait_semaphores.iter().map(|s| s.inner).collect::<Vec<_>>();

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&images_indices);

        let result = unsafe { self.inner.queue_present(queue.inner, &present_info)? };

        Ok(result)
    }

    fn destroy(&mut self) {
        unsafe {
            self.images_and_views.clear();
            self.depht_images_and_views.clear();
            self.inner.destroy_swapchain(self.swapchain_khr, None);
        }
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl fmt::Debug for Swapchain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Swapchain")
            .field("device", &self.device)
            .field("inner", &())
            .field("swapchain_khr", &self.swapchain_khr)
            .field("size", &self.size)
            .field("format", &self.format)
            .field("depth_format", &self.depth_format)
            .field("color_space", &self.color_space)
            .field("present_mode", &self.present_mode)
            .field("images_and_views", &self.images_and_views)
            .field("depht_images_and_views", &self.images_and_views).finish()
    }
}
