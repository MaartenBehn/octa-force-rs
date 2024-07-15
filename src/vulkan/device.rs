use std::{ffi::CString, sync::Arc};

use anyhow::Result;
use ash::{vk, Device as AshDevice};
use crate::{
    vulkan::instance::Instance,
    vulkan::physical_device::PhysicalDeviceCapabilities,
    vulkan::queue::{Queue, QueueFamily},
};
use crate::vulkan::physical_device::{PhysicalDevice, PhysicalDeviceFeatures};

#[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
use ash::extensions::khr::Synchronization2;

pub struct Device {
    pub inner: AshDevice,
}

impl Device {
    pub(crate) fn new(
        instance: &Instance,
        physical_device: &PhysicalDevice,
        extensions: &Vec<String>,
        device_features: &Vec<String>,
    ) -> Result<Self> {
        let queue_priorities = [1.0f32];
        
        let  queue_families = [physical_device.graphics_queue_family, physical_device.present_queue_family];
        let queue_create_infos = {
            let mut indices = queue_families.iter().map(|f| f.index).collect::<Vec<_>>();
            indices.dedup();

            indices
                .iter()
                .map(|index| {
                    vk::DeviceQueueCreateInfo::builder()
                        .queue_family_index(*index)
                        .queue_priorities(&queue_priorities)
                        .build()
                })
                .collect::<Vec<_>>()
        };

        let device_extensions = extensions
            .iter()
            .map(|e| CString::new(e.to_owned()))
            .collect::<Result<Vec<_>, _>>()?;

        let device_extensions_ptrs = device_extensions
            .iter()
            .map(|e| e.as_ptr())
            .collect::<Vec<_>>();

        let mut features = PhysicalDeviceFeatures::new(device_features);
        let mut vulkan_features = features.vulkan_features();

        let device_create_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&device_extensions_ptrs)
            .push_next(&mut vulkan_features);

        let inner = unsafe {
            instance
                .inner
                .create_device(physical_device.inner, &device_create_info, None)?
        };

        Ok(Self {
            inner
        })
    }

    pub fn get_queue(
        self: &Arc<Self>,
        queue_family: QueueFamily,
        queue_index: u32,

        #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
        synchronization2: Synchronization2,
    ) -> Queue {
        let inner = unsafe { self.inner.get_device_queue(queue_family.index, queue_index) };

        Queue::new(
            inner,

            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            synchronization2,

            #[cfg(vulkan_1_3)]
            self.clone(),
        )
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            self.inner.destroy_device(None);
        }
    }
}


