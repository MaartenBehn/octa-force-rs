use std::mem::{size_of_val, align_of};

use anyhow::Result;
use ash::vk;
use gpu_allocator::MemoryLocation;

use crate::{Buffer, Context, ImageAndView};
use crate::vulkan::{CommandBuffer, Image, ImageBarrier};

pub fn compute_aligned_size(size: u32, alignment: u32) -> u32 {
    (size + (alignment - 1)) & !(alignment - 1)
}

pub fn read_shader_from_bytes(bytes: &[u8]) -> Result<Vec<u32>> {
    let mut cursor = std::io::Cursor::new(bytes);
    Ok(ash::util::read_spv(&mut cursor)?)
}


impl Context {
    pub fn create_gpu_only_buffer_from_data<T: Copy>(
        &self,
        usage: vk::BufferUsageFlags,
        data: &[T],
    ) -> Result<Buffer> {
        self.create_gpu_only_buffer_from_data_complex(usage, data, align_of::<T>())
    }


    pub fn create_gpu_only_buffer_from_data_complex<T: Copy>(
        &self,
        usage: vk::BufferUsageFlags,
        data: &[T],
        alignment: usize,
    ) -> Result<Buffer> {
        let size = size_of_val(data) as _;
        let staging_buffer = self.create_buffer(
            vk::BufferUsageFlags::TRANSFER_SRC,
            MemoryLocation::CpuToGpu,
            size,
        )?;
        staging_buffer.copy_data_to_buffer_complex(data, 0, alignment)?;

        let buffer = self.create_buffer(
            usage | vk::BufferUsageFlags::TRANSFER_DST,
            MemoryLocation::GpuOnly,
            size,
        )?;

        self.execute_one_time_commands(|cmd_buffer| {
            cmd_buffer.copy_buffer(&staging_buffer, &buffer);
        })?;

        Ok(buffer)
    }

    pub fn create_storage_images(
        &mut self,
        format: vk::Format,
        extent: vk::Extent2D,
        count: usize,
    ) -> Result<Vec<ImageAndView>> {
        let mut images = Vec::with_capacity(count);

        for _ in 0..count {
            let image = self.create_image(
                vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::STORAGE,
                MemoryLocation::GpuOnly,
                format,
                extent.width,
                extent.height,
            )?;

            let view = image.create_image_view(false)?;

            self.execute_one_time_commands(|cmd_buffer| {
                cmd_buffer.pipeline_image_barriers(&[ImageBarrier {
                    image: &image,
                    old_layout: vk::ImageLayout::UNDEFINED,
                    new_layout: vk::ImageLayout::GENERAL,
                    src_access_mask: vk::AccessFlags2::NONE,
                    dst_access_mask: vk::AccessFlags2::NONE,
                    src_stage_mask: vk::PipelineStageFlags2::NONE,
                    dst_stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
                }]);
            })?;

            images.push(ImageAndView { image, view })
        }

        Ok(images)
    }

    pub fn recreate_storage_images(
        &mut self,
        format: vk::Format,
        extent: vk::Extent2D,
        storage_images: &mut Vec<ImageAndView>
    ) -> Result<()> {
        let new_storage_images = self.create_storage_images(
            format,
            extent,
            storage_images.len(),
        )?;

        let _ = std::mem::replace(storage_images, new_storage_images);

        Ok(())
    }
}


impl CommandBuffer {
    pub fn ready_swapchain_image(
        &self,
        swapchain_image: &Image,
    ) -> Result<()> {
        self.pipeline_image_barriers(&[ImageBarrier {
            image: swapchain_image,
            old_layout: vk::ImageLayout::UNDEFINED,
            new_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            src_access_mask: vk::AccessFlags2::NONE,
            dst_access_mask: vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
            src_stage_mask: vk::PipelineStageFlags2::NONE,
            dst_stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        }]);

        Ok(())
    }

    // Used with raytracing or compute rendering
    pub fn ready_swapchain_image_form_storage_image(
        &self,
        storage_image: &Image,
        swapchain_image: &Image,
    ) -> Result<()> {
        self.pipeline_image_barriers(&[
            ImageBarrier {
                image: swapchain_image,
                old_layout: vk::ImageLayout::UNDEFINED,
                new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                src_access_mask: vk::AccessFlags2::NONE,
                dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
                src_stage_mask: vk::PipelineStageFlags2::NONE,
                dst_stage_mask: vk::PipelineStageFlags2::TRANSFER,
            },
            ImageBarrier {
                image: storage_image,
                old_layout: vk::ImageLayout::GENERAL,
                new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                src_access_mask: vk::AccessFlags2::SHADER_WRITE,
                dst_access_mask: vk::AccessFlags2::TRANSFER_READ,
                src_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER
                    | vk::PipelineStageFlags2::RAY_TRACING_SHADER_KHR,
                dst_stage_mask: vk::PipelineStageFlags2::TRANSFER,
            },
        ]);

        self.copy_image(
            storage_image,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            swapchain_image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );

        self.pipeline_image_barriers(&[
            ImageBarrier {
                image: swapchain_image,
                old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                new_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
                dst_access_mask: vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
                src_stage_mask: vk::PipelineStageFlags2::TRANSFER,
                dst_stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            },
            ImageBarrier {
                image: storage_image,
                old_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                new_layout: vk::ImageLayout::GENERAL,
                src_access_mask: vk::AccessFlags2::TRANSFER_READ,
                dst_access_mask: vk::AccessFlags2::NONE,
                src_stage_mask: vk::PipelineStageFlags2::TRANSFER,
                dst_stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
            },
        ]);

        Ok(())
    }

    pub fn finish_swapchain_image(
        &self,
        swapchain_image: &Image,
    ) -> Result<()> {
        self.pipeline_image_barriers(&[ImageBarrier {
            image: swapchain_image,
            old_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            new_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            src_access_mask: vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
            dst_access_mask: vk::AccessFlags2::COLOR_ATTACHMENT_READ,
            src_stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            dst_stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        }]);
    }
}

