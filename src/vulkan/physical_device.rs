use std::collections::{HashMap, HashSet};
use std::ffi::CStr;

use anyhow::Result;
use ash::{vk, Instance};
use ash::vk::{PhysicalDeviceAccelerationStructureFeaturesKHR, PhysicalDeviceFeatures2, PhysicalDeviceRayTracingPipelineFeaturesKHR, PhysicalDeviceVulkan12Features, PhysicalDeviceVulkan13Features, PresentModeKHR, SurfaceFormatKHR};

use crate::{vulkan::queue::QueueFamily, vulkan::surface::Surface};

#[derive(Debug, Clone)]
pub struct PhysicalDevice {
    pub inner: vk::PhysicalDevice,
    pub name: String,
    pub device_type: vk::PhysicalDeviceType,

    pub limits: vk::PhysicalDeviceLimits,
    pub limits_ok: bool,

    pub queues: Vec<QueueFamily>,
    pub graphics_queue: Option<QueueFamily>,
    pub present_queue: Option<QueueFamily>,

    pub required_extensions: HashMap<String, bool>,
    pub required_extensions_ok: bool,

    pub wanted_extensions: HashMap<String, bool>,
    pub wanted_extensions_ok: bool,

    pub supported_surface_formats: Vec<SurfaceFormatKHR>,
    pub surface_format: Option<SurfaceFormatKHR>,

    pub supported_present_modes: Vec<PresentModeKHR>,
    pub present_mode: Option<PresentModeKHR>,

    pub required_device_features: HashMap<String, bool>,
    pub required_device_features_ok: bool,

    pub wanted_device_features: HashMap<String, bool>,
    pub wanted_device_features_ok: bool,
}

impl PhysicalDevice {
    pub(crate) fn new(
        instance: &Instance,
        surface: &Surface,
        inner: vk::PhysicalDevice,
        required_extensions: &Vec<String>,
        wanted_extensions: &Vec<String>,
        wanted_surface_formats: &Vec<SurfaceFormatKHR>,
        required_device_features: &Vec<String>,
        wanted_device_features: &Vec<String>,
    ) -> Result<Self> {
        let props = unsafe { instance.get_physical_device_properties(inner) };

        // Name
        let name = unsafe {
            CStr::from_ptr(props.device_name.as_ptr())
                .to_str()
                .unwrap()
                .to_owned()
        };

        // Type
        let device_type = props.device_type;

        // Limits
        let limits = props.limits;
        let limits_ok = true;

        // Queues
        let queue_family_properties =
            unsafe { instance.get_physical_device_queue_family_properties(inner) };
        let queue_families: Vec<_> = queue_family_properties
            .into_iter()
            .enumerate()
            .map(|(index, p)| {
                let present_support = unsafe {
                    surface.inner.get_physical_device_surface_support(
                        inner,
                        index as _,
                        surface.surface_khr,
                    )?
                };

                Ok(QueueFamily::new(index as _, p, present_support))
            })
            .collect::<Result<_>>()?;


        // Choose Graphics and Present Queue
        let mut graphics = None;
        let mut present = None;

        for family in queue_families.iter().filter(|f| f.has_queues()) {
            if family.supports_graphics()
                && family.supports_compute()
                && family.supports_timestamp_queries()
                && graphics.is_none()
            {
                graphics = Some(*family);
            }

            if family.supports_present() && present.is_none() {
                present = Some(*family);
            }

            if graphics.is_some() && present.is_some() {
                break;
            }
        }

        // Extensions
        let extension_properties =
            unsafe { instance.enumerate_device_extension_properties(inner)? };
        let supported_extensions: Vec<_> = extension_properties
            .into_iter()
            .map(|p| {
                let name = unsafe { CStr::from_ptr(p.extension_name.as_ptr()) };
                name.to_str().unwrap().to_owned()
            })
            .collect();

        let mut required_extensions_ok = true;
        let required_extensions = required_extensions.iter().map(|name| {
            let found = supported_extensions.contains(name);
            required_extensions_ok &= found;
            (name.to_owned(), found)
        }).collect();

        let mut wanted_extensions_ok = true;
        let wanted_extensions = wanted_extensions.iter().map(|name| {
            let found = supported_extensions.contains(name);
            wanted_extensions_ok &= found;
            (name.to_owned(), found)
        }).collect();


        // Surface Formats
        let supported_surface_formats = unsafe {
            surface
                .inner
                .get_physical_device_surface_formats(inner, surface.surface_khr)?
        };

        // Choose Surface Format to use
        let surface_format = if supported_surface_formats.len() == 0 {
            None
        } else {
            Some(wanted_surface_formats.iter().find(|wanted_format| {
                supported_surface_formats.iter().find(|format| {
                    format.format == wanted_format.format && format.color_space == wanted_format.color_space
                    // Base: B8G8R8A8_UNORM SRGB_NONLINEAR
                }).is_some()
            }).unwrap_or(&supported_surface_formats[0]).to_owned())
        };

        // Present Mode
        let supported_present_modes = unsafe {
            surface
                .inner
                .get_physical_device_surface_present_modes(inner, surface.surface_khr)?
        };

        // Choose Present mode to use
        // https://www.reddit.com/r/vulkan/comments/4sbpnz/trying_to_understand_presentation_modes/
        let present_mode_prio = [
            PresentModeKHR::FIFO_RELAXED,
            PresentModeKHR::FIFO,
            PresentModeKHR::MAILBOX,
            PresentModeKHR::SHARED_CONTINUOUS_REFRESH,
            PresentModeKHR::SHARED_DEMAND_REFRESH,
            PresentModeKHR::IMMEDIATE,
        ];
        let mut present_mode = None;
        for wanted in present_mode_prio {
            if supported_present_modes.contains(&wanted) {
                present_mode = Some(wanted);
                break;
            }
        }

        // Device Features
        let mut required_features = PhysicalDeviceFeatures::new(&required_device_features);
        unsafe { instance.get_physical_device_features2(inner, &mut required_features.vulkan_features()) };
        let (required_device_features_ok, required_device_features) = required_features.result(&required_device_features);

        let mut wanted_features = PhysicalDeviceFeatures::new(&wanted_device_features);
        unsafe { instance.get_physical_device_features2(inner, &mut wanted_features.vulkan_features()) };
        let (wanted_device_features_ok, wanted_device_features) = wanted_features.result(&wanted_device_features);


        Ok(Self {
            inner,
            name,
            device_type,

            limits,
            limits_ok,

            queues: queue_families,
            graphics_queue: graphics,
            present_queue: present,

            required_extensions,
            required_extensions_ok,

            wanted_extensions,
            wanted_extensions_ok,

            supported_surface_formats,
            surface_format,

            supported_present_modes,
            present_mode,

            required_device_features,
            required_device_features_ok,

            wanted_device_features,
            wanted_device_features_ok,
        })
    }
}

pub(crate) struct PhysicalDeviceFeatures {
    pub(crate) ray_tracing_feature: PhysicalDeviceRayTracingPipelineFeaturesKHR,
    pub(crate) acceleration_struct_feature: PhysicalDeviceAccelerationStructureFeaturesKHR,
    pub(crate) features12: PhysicalDeviceVulkan12Features,
    pub(crate) features13: PhysicalDeviceVulkan13Features,
}

impl PhysicalDeviceFeatures {
    pub(crate) fn new(required_device_features: &Vec<String>) -> PhysicalDeviceFeatures {
        let mut required_features: HashSet<_> = required_device_features.iter().collect();
        let ray_tracing_feature = vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::builder()
            .ray_tracing_pipeline(required_features.take(&"rayTracingPipeline".to_owned()).is_some())
            .build();

        let acceleration_struct_feature = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::builder()
            .acceleration_structure(required_features.take(&"accelerationStructure".to_owned()).is_some())
            .build();

        let features12 = vk::PhysicalDeviceVulkan12Features::builder()
            .runtime_descriptor_array(required_features.take(&"runtimeDescriptorArray".to_owned()).is_some())
            .buffer_device_address(required_features.take(&"bufferDeviceAddress".to_owned()).is_some())
            .build();

        let features13 = vk::PhysicalDeviceVulkan13Features::builder()
            .dynamic_rendering(required_features.take(&"dynamicRendering".to_owned()).is_some())
            .synchronization2(required_features.take(&"synchronization2".to_owned()).is_some())
            .build();

        for feature in required_features {
            log::warn!("Device Feature: {feature} not implemented.");
        }

        PhysicalDeviceFeatures {ray_tracing_feature, acceleration_struct_feature, features12, features13}
    }

    pub(crate) fn result(&self, required_device_features: &Vec<String>) -> (bool, HashMap<String, bool>) {
        let mut required_features: HashSet<_> = required_device_features.iter().collect();
        let mut result = HashMap::new();
        let mut all = true;

        if required_features.take(&"rayTracingPipeline".to_owned()).is_some() {
            let found = self.ray_tracing_feature.ray_tracing_pipeline == vk::TRUE;
            result.insert("rayTracingPipeline".to_owned(), found);
            all &= found;
        }
        if required_features.take(&"accelerationStructure".to_owned()).is_some() {
            let found = self.acceleration_struct_feature.acceleration_structure == vk::TRUE;
            result.insert("accelerationStructure".to_owned(), found);
            all &= found;
        }
        if required_features.take(&"runtimeDescriptorArray".to_owned()).is_some() {
            let found = self.features12.runtime_descriptor_array == vk::TRUE;
            result.insert("runtimeDescriptorArray".to_owned(), found);
            all &= found;
        }
        if required_features.take(&"bufferDeviceAddress".to_owned()).is_some() {
            let found = self.features12.buffer_device_address == vk::TRUE;
            result.insert("bufferDeviceAddress".to_owned(), found);
            all &= found;
        }
        if required_features.take(&"dynamicRendering".to_owned()).is_some() {
            let found = self.features13.dynamic_rendering == vk::TRUE;
            result.insert("dynamicRendering".to_owned(), found);
            all &= found;
        }
        if required_features.take(&"synchronization2".to_owned()).is_some() {
            let found = self.features13.synchronization2 == vk::TRUE;
            result.insert("synchronization2".to_owned(), found);
            all &= found;
        }

        (all, result)
    }

    pub(crate) fn vulkan_features(&mut self) -> PhysicalDeviceFeatures2{
        let mut builder = PhysicalDeviceFeatures2::builder();
        if self.ray_tracing_feature.ray_tracing_pipeline == vk::TRUE {
            builder = builder.push_next(&mut self.ray_tracing_feature);
        }
        if self.acceleration_struct_feature.acceleration_structure == vk::TRUE {
            builder = builder.push_next(&mut self.acceleration_struct_feature);
        }
        if (self.features12.runtime_descriptor_array | self.features12.runtime_descriptor_array) == vk::TRUE {
            builder = builder.push_next(&mut self.features12);
        }
        if (self.features13.dynamic_rendering | self.features13.synchronization2) == vk::TRUE {
            builder = builder.push_next(&mut self.features13);
        }

        builder.build()
    }
}



