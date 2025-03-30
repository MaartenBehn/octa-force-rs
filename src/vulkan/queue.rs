use anyhow::Result;
use crate::{CommandBuffer, Fence, Semaphore};
use ash::vk;

#[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
use ash::khr::synchronization2::Device as Synchronization2;

#[cfg(vulkan_1_3)]
use std::sync::Arc;
#[cfg(vulkan_1_3)]
use crate::vulkan::Device;

#[derive(Debug, Clone, Copy)]
pub struct QueueFamily {
    pub index: u32,
    pub(crate) inner: vk::QueueFamilyProperties,
    supports_present: bool,
}

impl QueueFamily {
    pub(crate) fn new(
        index: u32,
        inner: vk::QueueFamilyProperties,
        supports_present: bool,
    ) -> Self {
        Self {
            index,
            inner,
            supports_present,
        }
    }

    pub fn supports_compute(&self) -> bool {
        self.inner.queue_flags.contains(vk::QueueFlags::COMPUTE)
    }

    pub fn supports_graphics(&self) -> bool {
        self.inner.queue_flags.contains(vk::QueueFlags::GRAPHICS)
    }

    pub fn supports_present(&self) -> bool {
        self.supports_present
    }

    pub fn has_queues(&self) -> bool {
        self.inner.queue_count > 0
    }

    pub fn supports_timestamp_queries(&self) -> bool {
        self.inner.timestamp_valid_bits > 0
    }
}

pub struct Queue {
    pub inner: vk::Queue,

    #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
    synchronization2: Synchronization2,

    #[cfg(vulkan_1_3)]
    device: Arc<Device>,
}

impl Queue {
    pub(crate) fn new(
        inner: vk::Queue,

        #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
        synchronization2: Synchronization2,

        #[cfg(vulkan_1_3)]
        device: Arc<Device>
    ) -> Self {
        Self {
            inner,

            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            synchronization2,

            #[cfg(vulkan_1_3)]
            device,
        }
    }

    pub fn submit(
        &self,
        command_buffer: &CommandBuffer,
        wait_semaphore: Option<SemaphoreSubmitInfo>,
        signal_semaphore: Option<SemaphoreSubmitInfo>,
        fence: &Fence,
    ) -> Result<()> {
        let wait_semaphore_submit_info = wait_semaphore.map(|s| {
            vk::SemaphoreSubmitInfo::default()
                .semaphore(s.semaphore.inner)
                .stage_mask(s.stage_mask)
        });

        let signal_semaphore_submit_info = signal_semaphore.map(|s| {
            vk::SemaphoreSubmitInfo::default()
                .semaphore(s.semaphore.inner)
                .stage_mask(s.stage_mask)
        });

        let cmd_buffer_submit_info =
            vk::CommandBufferSubmitInfo::default().command_buffer(command_buffer.inner);

        let submit_info = vk::SubmitInfo2::default()
            .command_buffer_infos(std::slice::from_ref(&cmd_buffer_submit_info));

        let submit_info = match wait_semaphore_submit_info.as_ref() {
            Some(info) => submit_info.wait_semaphore_infos(std::slice::from_ref(info)),
            None => submit_info,
        };

        let submit_info = match signal_semaphore_submit_info.as_ref() {
            Some(info) => submit_info.signal_semaphore_infos(std::slice::from_ref(info)),
            None => submit_info,
        };

        unsafe {
            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            self.synchronization2.queue_submit2(
                self.inner,
                std::slice::from_ref(&submit_info),
                fence.inner
            )?;

            #[cfg(vulkan_1_3)]
            self.device.inner.queue_submit2(
                self.inner,
                std::slice::from_ref(&submit_info),
                fence.inner,
            )?
        };

        Ok(())
    }
}

pub struct SemaphoreSubmitInfo<'a> {
    pub semaphore: &'a Semaphore,
    pub stage_mask: vk::PipelineStageFlags2,
}
