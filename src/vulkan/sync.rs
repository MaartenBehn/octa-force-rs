use anyhow::Result;
use ash::vk;
use std::sync::Arc;

use crate::{vulkan::device::Device, Context};

#[derive(Debug)]
pub struct Semaphore {
    device: Arc<Device>,
    pub(crate) inner: vk::Semaphore,
}

impl Semaphore {
    pub(crate) fn new(device: Arc<Device>) -> Result<Self> {
        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let inner = unsafe { device.inner.create_semaphore(&semaphore_info, None)? };

        Ok(Self { device, inner })
    }
}

impl Context {
    pub fn create_semaphore(&self) -> Result<Semaphore> {
        Semaphore::new(self.device.clone())
    }
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        unsafe {
            self.device.inner.destroy_semaphore(self.inner, None);
        }
    }
}

#[derive(Debug)]
pub struct Fence {
    device: Arc<Device>,
    pub(crate) inner: vk::Fence,
}

impl Fence {
    pub(crate) fn new(device: Arc<Device>, flags: Option<vk::FenceCreateFlags>) -> Result<Self> {
        let flags = flags.unwrap_or_else(vk::FenceCreateFlags::empty);

        let fence_info = vk::FenceCreateInfo::default().flags(flags);
        let inner = unsafe { device.inner.create_fence(&fence_info, None)? };

        Ok(Self { device, inner })
    }

    pub fn wait(&self, timeout: Option<u64>) -> Result<()> {
        let timeout = timeout.unwrap_or(std::u64::MAX);

        unsafe {
            self.device
                .inner
                .wait_for_fences(&[self.inner], true, timeout)?
        };

        Ok(())
    }

    pub fn reset(&self) -> Result<()> {
        unsafe { self.device.inner.reset_fences(&[self.inner])? };

        Ok(())
    }
}

impl Context {
    pub fn create_fence(&self, flags: Option<vk::FenceCreateFlags>) -> Result<Fence> {
        Fence::new(self.device.clone(), flags)
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        unsafe {
            self.device.inner.destroy_fence(self.inner, None);
        }
    }
}
