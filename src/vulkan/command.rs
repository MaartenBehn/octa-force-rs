use core::slice;
use std::{mem, sync::Arc};
use anyhow::Result;
use ash::vk::{self, Extent2D, IndexType, Offset2D};
use glam::{UVec2, Vec2};
use crate::{
    vulkan::device::Device, Buffer, ComputePipeline, Context, DescriptorSet, GraphicsPipeline,
    Image, ImageView, PipelineLayout, QueueFamily, RayTracingContext, RayTracingPipeline,
    ShaderBindingTable, TimestampQueryPool,
};

#[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
use ash::khr::{DynamicRendering, Synchronization2};

#[derive(Debug)]
pub struct CommandPool {
    device: Arc<Device>,
    ray_tracing: Option<Arc<RayTracingContext>>,
    pub inner: vk::CommandPool,

    #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
    synchronization2: Synchronization2,

    #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
    dynamic_rendering: DynamicRendering
}

impl CommandPool {
    pub(crate) fn new(
        device: Arc<Device>,
        ray_tracing: Option<Arc<RayTracingContext>>,
        queue_family: QueueFamily,
        flags: Option<vk::CommandPoolCreateFlags>,

        #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
        synchronization2: Synchronization2,

        #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
        dynamic_rendering: DynamicRendering
    ) -> Result<Self> {
        let flags = flags.unwrap_or_else(vk::CommandPoolCreateFlags::empty);

        let command_pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family.index)
            .flags(flags);
        let inner = unsafe { device.inner.create_command_pool(&command_pool_info, None)? };

        Ok(Self {
            device,
            ray_tracing,
            inner,

            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            synchronization2,

            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            dynamic_rendering
        })
    }

    pub fn allocate_command_buffers(
        &self,
        level: vk::CommandBufferLevel,
        count: u32,
    ) -> Result<Vec<CommandBuffer>> {
        let allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.inner)
            .level(level)
            .command_buffer_count(count);

        let buffers = unsafe { self.device.inner.allocate_command_buffers(&allocate_info)? };
        let buffers = buffers
            .into_iter()
            .map(|inner| CommandBuffer {
                device: self.device.clone(),
                ray_tracing: self.ray_tracing.clone(),
                inner,

                #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
                synchronization2: self.synchronization2.to_owned(),

                #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
                dynamic_rendering: self.dynamic_rendering.to_owned(),
            })
            .collect();

        Ok(buffers)
    }

    pub fn allocate_command_buffer(
        &self,
        level: vk::CommandBufferLevel,
    ) -> Result<CommandBuffer> {
        let buffers = self.allocate_command_buffers(level, 1, )?;
        let buffer = buffers.into_iter().next().unwrap();

        Ok(buffer)
    }

    pub fn free_command_buffers(&self, buffer: &[CommandBuffer]) {
        let buffs = buffer.iter().map(|b| b.inner).collect::<Vec<_>>();
        unsafe { self.device.inner.free_command_buffers(self.inner, &buffs) };
    }

    pub fn free_command_buffer(&self, buffer: &CommandBuffer) -> Result<()> {
        let buffs = [buffer.inner];
        unsafe { self.device.inner.free_command_buffers(self.inner, &buffs) };

        Ok(())
    }
}

impl Context {
    pub fn create_command_pool(
        &self,
        queue_family: QueueFamily,
        flags: Option<vk::CommandPoolCreateFlags>,

    ) -> Result<CommandPool> {
        CommandPool::new(
            self.device.clone(),
            self.ray_tracing.clone(),
            queue_family,
            flags,

            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            self.synchronization2.to_owned(),

            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            self.dynamic_rendering.to_owned(),
        )
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        unsafe { self.device.inner.destroy_command_pool(self.inner, None) };
    }
}

#[derive(Debug)]
pub struct CommandBuffer {
    device: Arc<Device>,
    ray_tracing: Option<Arc<RayTracingContext>>,
    pub inner: vk::CommandBuffer,

    #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
    synchronization2: Synchronization2,

    #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
    dynamic_rendering: DynamicRendering
}

impl CommandBuffer {
    pub fn begin(&self, flags: Option<vk::CommandBufferUsageFlags>) -> Result<()> {
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(flags.unwrap_or(vk::CommandBufferUsageFlags::empty()));
        unsafe {
            self.device
                .inner
                .begin_command_buffer(self.inner, &begin_info)?
        };

        Ok(())
    }

    pub fn end(&self) -> Result<()> {
        unsafe { self.device.inner.end_command_buffer(self.inner)? };

        Ok(())
    }

    pub fn reset(&self) -> Result<()> {
        unsafe {
            self.device
                .inner
                .reset_command_buffer(self.inner, vk::CommandBufferResetFlags::empty())?
        };

        Ok(())
    }

    pub fn bind_rt_pipeline(&self, pipeline: &RayTracingPipeline) {
        unsafe {
            self.device.inner.cmd_bind_pipeline(
                self.inner,
                vk::PipelineBindPoint::RAY_TRACING_KHR,
                pipeline.inner,
            )
        }
    }

    pub fn bind_graphics_pipeline(&self, pipeline: &GraphicsPipeline) {
        unsafe {
            self.device.inner.cmd_bind_pipeline(
                self.inner,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline.inner,
            )
        }
    }

    pub fn bind_compute_pipeline(&self, pipeline: &ComputePipeline) {
        unsafe {
            self.device.inner.cmd_bind_pipeline(
                self.inner,
                vk::PipelineBindPoint::COMPUTE,
                pipeline.inner,
            )
        }
    }

    pub fn bind_vertex_buffer(&self, vertex_buffer: &Buffer) {
        unsafe {
            self.device
                .inner
                .cmd_bind_vertex_buffers(self.inner, 0, &[vertex_buffer.inner], &[0])
        };
    }

    pub fn bind_index_buffer(&self, index_buffer: &Buffer) {
        self.bind_index_buffer_complex(index_buffer, 0, IndexType::UINT32)
    }

    pub fn bind_index_buffer_complex(&self, index_buffer: &Buffer, offset: vk::DeviceSize, index_type: IndexType) {
        unsafe {
            self.device.inner.cmd_bind_index_buffer(
                self.inner,
                index_buffer.inner,
                offset,
                index_type,
            )
        };
    }

    pub fn push_constant<T>(
        &self,
        layout: &PipelineLayout,
        stage_flags: vk::ShaderStageFlags,
        push_constant: &T,
    ) where
        T: Sized,
    {
        unsafe {
            let date = slice::from_raw_parts(
                (push_constant as *const T) as *const u8,
                mem::size_of::<T>(),
            );

            self.device
                .inner
                .cmd_push_constants(self.inner, layout.inner, stage_flags, 0, date)
        };
    }

    pub fn draw(&self, vertex_count: u32) {
        unsafe {
            self.device
                .inner
                .cmd_draw(self.inner, vertex_count, 1, 0, 0)
        };
    }

    pub fn draw_indexed(&self, index_count: u32) {
        unsafe {
            self.device
                .inner
                .cmd_draw_indexed(self.inner, index_count, 1, 0, 0, 0)
        };
    }

    pub fn draw_indexed_instanced(&self, index_count: u32, instance_count: u32) {
        unsafe {
            self.device
                .inner
                .cmd_draw_indexed(self.inner, index_count, instance_count, 0, 0, 0)
        };
    }

    pub fn dispatch(&self, group_count_x: u32, group_count_y: u32, group_count_z: u32) {
        unsafe {
            self.device
                .inner
                .cmd_dispatch(self.inner, group_count_x, group_count_y, group_count_z);
        }
    }

    pub fn bind_descriptor_sets(
        &self,
        bind_point: vk::PipelineBindPoint,
        layout: &PipelineLayout,
        first_set: u32,
        sets: &[&DescriptorSet],
    ) {
        let sets = sets.iter().map(|s| s.inner).collect::<Vec<_>>();
        unsafe {
            self.device.inner.cmd_bind_descriptor_sets(
                self.inner,
                bind_point,
                layout.inner,
                first_set,
                &sets,
                &[],
            )
        }
    }

    pub fn pipeline_buffer_barriers(&self, barriers: &[BufferBarrier]) {
        let barriers = barriers
            .iter()
            .map(|b| {
                vk::BufferMemoryBarrier2::default()
                    .src_stage_mask(b.src_stage_mask)
                    .src_access_mask(b.src_access_mask)
                    .dst_stage_mask(b.dst_stage_mask)
                    .dst_access_mask(b.dst_access_mask)
                    .buffer(b.buffer.inner)
                    .offset(0)
                    .size(vk::WHOLE_SIZE)
            })
            .collect::<Vec<_>>();

        let dependency_info = vk::DependencyInfo::default().buffer_memory_barriers(&barriers);

        unsafe {
            self.device
                .inner
                .cmd_pipeline_barrier2(self.inner, &dependency_info)
        };
    }

    pub fn pipeline_memory_barriers(&self, barriers: &[MemoryBarrier]) {
        let barriers = barriers
            .iter()
            .map(|b| {
                vk::MemoryBarrier2::default()
                    .src_stage_mask(b.src_stage_mask)
                    .src_access_mask(b.src_access_mask)
                    .dst_stage_mask(b.dst_stage_mask)
                    .dst_access_mask(b.dst_access_mask)
            })
            .collect::<Vec<_>>();

        let dependency_info = vk::DependencyInfo::default().memory_barriers(&barriers);

        unsafe {
            self.device
                .inner
                .cmd_pipeline_barrier2(self.inner, &dependency_info)
        };
    }

    pub fn copy_buffer(&self, src_buffer: &Buffer, dst_buffer: &Buffer) {
        unsafe {
            let region = vk::BufferCopy::default().size(src_buffer.size);
            self.device.inner.cmd_copy_buffer(
                self.inner,
                src_buffer.inner,
                dst_buffer.inner,
                std::slice::from_ref(&region),
            )
        };
    }

    pub fn pipeline_image_barriers(&self, barriers: &[ImageBarrier]) {
        let barriers = barriers
            .iter()
            .map(|b| {
                vk::ImageMemoryBarrier2::default()
                    .src_stage_mask(b.src_stage_mask)
                    .src_access_mask(b.src_access_mask)
                    .old_layout(b.old_layout)
                    .dst_stage_mask(b.dst_stage_mask)
                    .dst_access_mask(b.dst_access_mask)
                    .new_layout(b.new_layout)
                    .image(b.image.inner)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
            })
            .collect::<Vec<_>>();

        let dependency_info = vk::DependencyInfo::default().image_memory_barriers(&barriers);

        unsafe {
            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            self.synchronization2
                .cmd_pipeline_barrier2(self.inner, &dependency_info);

            #[cfg(vulkan_1_3)]
            self.device
                .inner
                .cmd_pipeline_barrier2(self.inner, &dependency_info)
        };
    }

    pub fn copy_image(
        &self,
        src_image: &Image,
        src_layout: vk::ImageLayout,
        dst_image: &Image,
        dst_layout: vk::ImageLayout,
    ) {
        let region = vk::ImageCopy::default()
            .src_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_array_layer: 0,
                mip_level: 0,
                layer_count: 1,
            })
            .dst_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_array_layer: 0,
                mip_level: 0,
                layer_count: 1,
            })
            .extent(vk::Extent3D {
                width: src_image.extent.width,
                height: src_image.extent.height,
                depth: 1,
            });

        unsafe {
            self.device.inner.cmd_copy_image(
                self.inner,
                src_image.inner,
                src_layout,
                dst_image.inner,
                dst_layout,
                std::slice::from_ref(&region),
            )
        };
    }

    pub fn copy_buffer_to_image(&self, src: &Buffer, dst: &Image, layout: vk::ImageLayout) {
        let region = vk::BufferImageCopy::default()
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_extent(dst.extent);

        unsafe {
            self.device.inner.cmd_copy_buffer_to_image(
                self.inner,
                src.inner,
                dst.inner,
                layout,
                std::slice::from_ref(&region),
            );
        };
    }

    pub fn build_acceleration_structures(
        &self,
        as_build_geo_info: &vk::AccelerationStructureBuildGeometryInfoKHR,
        as_build_range_info: &[vk::AccelerationStructureBuildRangeInfoKHR],
    ) {
        let ray_tracing = self.ray_tracing.as_ref().expect(
            "Cannot call CommandBuffer::build_acceleration_structures when ray tracing is not enabled",
        );

        unsafe {
            ray_tracing
                .acceleration_structure_fn
                .cmd_build_acceleration_structures(
                    self.inner,
                    std::slice::from_ref(as_build_geo_info),
                    std::slice::from_ref(&as_build_range_info),
                )
        };
    }

    pub fn trace_rays(&self, shader_binding_table: &ShaderBindingTable, width: u32, height: u32) {
        let ray_tracing = self
            .ray_tracing
            .as_ref()
            .expect("Cannot call CommandBuffer::trace_rays when ray tracing is not enabled");

        unsafe {
            ray_tracing.pipeline_fn.cmd_trace_rays(
                self.inner,
                &shader_binding_table.raygen_region,
                &shader_binding_table.miss_region,
                &shader_binding_table.hit_region,
                &vk::StridedDeviceAddressRegionKHR::default(),
                width,
                height,
                1,
            )
        };
    }

    pub fn begin_rendering(
        &self,
        image_view: &ImageView,
        depth_view: &ImageView,
        size: UVec2,
        load_op: vk::AttachmentLoadOp,
        clear_color: Option<[f32; 4]>,
    ) {
        let color_attachment_info = vk::RenderingAttachmentInfo::default()
            .image_view(image_view.inner)
            .image_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
            .load_op(load_op)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: clear_color.unwrap_or([1.0; 4]),
                },
            });

        let mut rendering_info = vk::RenderingInfo::default()
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: Extent2D{ width: size.x, height: size.y },
            })
            .layer_count(1)
            .color_attachments(std::slice::from_ref(&color_attachment_info));

        let depth_attachment_info = vk::RenderingAttachmentInfo::default()
            .image_view(depth_view.inner)
            .image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .clear_value(vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: f32::MAX,
                    stencil: 0,
                },
            });

        rendering_info = rendering_info.depth_attachment(&depth_attachment_info);

        unsafe {
            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            self.dynamic_rendering
                .cmd_begin_rendering(self.inner, &rendering_info);

            #[cfg(vulkan_1_3)]
            self.device
                .inner
                .cmd_begin_rendering(self.inner, &rendering_info)
        };
    }

    pub fn end_rendering(&self) {
        unsafe {
            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            self.dynamic_rendering.cmd_end_rendering(self.inner);

            #[cfg(vulkan_1_3)]
            self.device.inner.cmd_end_rendering(self.inner);
        };
    }

    pub fn set_viewport(&self, pos: Vec2, size: Vec2) {
        unsafe {
            self.device.inner.cmd_set_viewport(
                self.inner,
                0,
                &[vk::Viewport {
                    x: pos.x,
                    y: pos.y,
                    width: size.x,
                    height: size.y,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            )
        };
    }

    pub fn set_viewport_size(&self, size: Vec2) {
        self.set_viewport(Vec2::ZERO, size)
    }

    pub fn set_scissor(&self, pos: Vec2, size: Vec2) {
        unsafe {
            self.device.inner.cmd_set_scissor(
                self.inner,
                0,
                &[vk::Rect2D {
                    offset: Offset2D {x: pos.x as i32, y: pos.y as i32},
                    extent: Extent2D {width: size.x as u32, height: size.y as u32 },
                }],
            )
        };
    }

    pub fn set_scissor_size(&self, size: Vec2) {
        self.set_scissor(Vec2::ZERO, size)
    }

    pub fn reset_all_timestamp_queries_from_pool<const C: usize>(
        &self,
        pool: &TimestampQueryPool<C>,
    ) {
        unsafe {
            self.device
                .inner
                .cmd_reset_query_pool(self.inner, pool.inner, 0, C as _);
        }
    }

    pub fn write_timestamp<const C: usize>(
        &self,
        stage: vk::PipelineStageFlags2,
        pool: &TimestampQueryPool<C>,
        query_index: u32,
    ) {
        assert!(query_index < C as u32, "Query index must be < {C}");

        unsafe {
            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            self.synchronization2
                .cmd_write_timestamp2(self.inner, stage, pool.inner, query_index);


            #[cfg(vulkan_1_3)]
            self.device
                .inner
                .cmd_write_timestamp2(self.inner, stage, pool.inner, query_index);
        }
    }
}

#[derive(Clone, Copy)]
pub struct BufferBarrier<'a> {
    pub buffer: &'a Buffer,
    pub src_access_mask: vk::AccessFlags2,
    pub dst_access_mask: vk::AccessFlags2,
    pub src_stage_mask: vk::PipelineStageFlags2,
    pub dst_stage_mask: vk::PipelineStageFlags2,
}

pub struct MemoryBarrier {
    pub src_access_mask: vk::AccessFlags2,
    pub dst_access_mask: vk::AccessFlags2,
    pub src_stage_mask: vk::PipelineStageFlags2,
    pub dst_stage_mask: vk::PipelineStageFlags2,
}

#[derive(Clone, Copy)]
pub struct ImageBarrier<'a> {
    pub image: &'a Image,
    pub old_layout: vk::ImageLayout,
    pub new_layout: vk::ImageLayout,
    pub src_access_mask: vk::AccessFlags2,
    pub dst_access_mask: vk::AccessFlags2,
    pub src_stage_mask: vk::PipelineStageFlags2,
    pub dst_stage_mask: vk::PipelineStageFlags2,
}
