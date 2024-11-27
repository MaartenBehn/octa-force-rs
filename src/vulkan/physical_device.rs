use std::collections::{HashMap, HashSet};
use std::ffi::CStr;

use anyhow::{bail, Result};
use ash::{vk};
use ash::vk::{Format, FormatFeatureFlags, PhysicalDeviceAccelerationStructureFeaturesKHR, PhysicalDeviceFeatures2, PhysicalDeviceRayTracingPipelineFeaturesKHR, PhysicalDeviceVulkan12Features, PhysicalDeviceVulkan13Features, PresentModeKHR, SurfaceFormatKHR};
use log::error;
use crate::{vulkan::queue::QueueFamily, vulkan::surface::Surface};
use crate::vulkan::instance::Instance;

#[derive(Debug, Clone)]
pub struct PhysicalDevice {
    pub inner: vk::PhysicalDevice,
    pub name: String,
    pub device_type: vk::PhysicalDeviceType,

    pub limits: vk::PhysicalDeviceLimits,

    pub graphics_queue_family: QueueFamily,
    pub present_queue_family: QueueFamily,

    pub wanted_extensions: HashMap<String, bool>,

    pub surface_format: SurfaceFormatKHR,
    pub render_storage_image_format: Format,

    pub depth_format: Format,

    pub present_mode: PresentModeKHR,

    pub wanted_device_features: HashMap<String, bool>,
}


#[derive(Debug, Clone)]
#[allow(unused)]
pub struct PhysicalDeviceCapabilities {
    pub inner: vk::PhysicalDevice,
    pub name: String,
    pub device_type: vk::PhysicalDeviceType,

    pub limits: vk::PhysicalDeviceLimits,
    pub limits_ok: bool,

    pub queues: Vec<QueueFamily>,
    pub graphics_queues: Vec<QueueFamily>,
    pub present_queues: Vec<QueueFamily>,

    pub required_extensions: HashMap<String, bool>,
    pub required_extensions_ok: bool,

    pub wanted_extensions: HashMap<String, bool>,
    pub wanted_extensions_ok: bool,

    pub supported_surface_formats: Vec<SurfaceFormatKHR>,
    pub supported_surface_formats_with_storage_bit: Vec<SurfaceFormatKHR>,
    pub render_storage_image_formats: Vec<Format>,

    pub supported_depth_formats: Vec<Format>,

    pub supported_present_modes: Vec<PresentModeKHR>,

    pub required_device_features: HashMap<String, bool>,
    pub required_device_features_ok: bool,

    pub wanted_device_features: HashMap<String, bool>,
    pub wanted_device_features_ok: bool,
}



impl Instance {
    pub(crate) fn select_suitable_physical_device(
        &mut self,
        render_storage_image_format_is_needed: bool,
        surface_formats_with_storage_bit_is_wanted: bool
    ) -> Result<PhysicalDevice> {
        let mut seen_names = Vec::new();

        let mut devices_capabilities: Vec<_> = self.physical_devices_capabilities
            .iter()
            .filter_map(|device_capabilities| {
                let name = &device_capabilities.name;

                if seen_names.contains(name){
                    return None;
                }
                seen_names.push(name.to_owned());
                log::info!("Possible Device: {name}");

                let mut ok = true;
                let mut minus_points = 0;

                if !device_capabilities.limits_ok {
                    ok = false;
                    log::info!(" -- Limits not ok");
                }
                
                if device_capabilities.graphics_queues.is_empty() {
                    ok = false;
                    log::info!(" -- No Graphics Queue");
                }

                if device_capabilities.present_queues.is_empty() {
                    ok = false;
                    log::info!(" -- No Present Queue");
                }

                if device_capabilities.supported_surface_formats.is_empty() {
                    ok = false;
                    log::info!(" -- No Supported Surface Format");
                }
                
                if device_capabilities.supported_surface_formats_with_storage_bit.is_empty() {
                    log::info!(" -- No Supported Surface Format with Storage Bit");
                    if surface_formats_with_storage_bit_is_wanted {
                        minus_points -= 10;
                        log::info!(" ---- Wanted => -10 Points");
                    } else {
                        log::info!(" ---- Not Wanted");
                    }
                    
                    if render_storage_image_format_is_needed && device_capabilities.render_storage_image_formats.is_empty() {
                        ok = false;
                        log::info!(" -- No Supported Render Storage Image Format");
                    }
                }

                if device_capabilities.supported_present_modes.is_empty() {
                    ok = false;
                    log::info!(" -- No Present Mode");
                }

                if !device_capabilities.required_extensions_ok {
                    ok = false;
                    log::info!(" -- Extensions not ok");
                    for (n, b) in device_capabilities.required_extensions.iter() {
                        if !b {
                            log::info!(" ---- {n} missing.");
                        }
                    }
                }

                if !device_capabilities.wanted_device_features_ok {
                    log::info!(" -- Not all wanted Extensions");
                    for (n, b) in device_capabilities.wanted_extensions.iter() {
                        if !b {
                            minus_points -= 5;
                            log::info!(" ---- {n} missing => -5 Points");
                        }
                    }
                }

                if !device_capabilities.required_device_features_ok {
                    ok = false;
                    log::info!(" -- Device Features not ok");
                    for (n, b) in device_capabilities.required_device_features.iter() {
                        if !b {
                            log::info!(" ---- {n} missing.");
                        }
                    }
                }

                if !device_capabilities.wanted_device_features_ok {
                    ok = false;
                    log::info!(" -- Not all wanted Device Features");
                    for (n, b) in device_capabilities.wanted_device_features.iter() {
                        if !b {
                            minus_points -= 5;
                            log::info!(" ---- {n} missing => -5 Points");
                        }
                    }
                }

                if ok {
                    log::info!(" -- Ok");
                    return Some((device_capabilities, minus_points));
                }

                None
            }).collect();
        
        if devices_capabilities.is_empty() {
            bail!("No suitable Device found!")
        }
        
        devices_capabilities.sort_by(|(c1, minus_points1), (c2, minus_points2)| {
            minus_points1.cmp(minus_points2).then(c1.limits.max_memory_allocation_count.cmp(&c2.limits.max_memory_allocation_count))
        });

        log::info!("Sorted suitable Devices: ");
        for (c, _) in devices_capabilities.iter() {
            log::info!(" -- {}", c.name);
        }
        
        let selected_device_capabilities = devices_capabilities[0].0;
        log::info!("Selected Physical Device: {}", selected_device_capabilities.name);
        
        let device_type = selected_device_capabilities.device_type;
        log::info!(" -- Device Type: {device_type:?}");
        
        let (surface_format, surface_format_storage_image_support) = if !selected_device_capabilities.supported_surface_formats_with_storage_bit.is_empty() {
            (selected_device_capabilities.supported_surface_formats_with_storage_bit[0], true)
        } else {
            (selected_device_capabilities.supported_surface_formats[0], false)
        };
        log::info!(" -- Surface format: {:?}  {:?}", surface_format.format, surface_format.color_space);
        log::info!(" ---- {} storage image support", if surface_format_storage_image_support {"✔"} else {"❌"});

        let render_storage_image_format = if surface_format_storage_image_support {
            selected_device_capabilities.supported_surface_formats_with_storage_bit[0].format
        } else {
            selected_device_capabilities.render_storage_image_formats[0]
        };
        log::info!(" -- Render storage image format: {:?}", render_storage_image_format);

        let depth_format = selected_device_capabilities.supported_depth_formats[0];
        log::info!(" -- Depth format: {:?} ", depth_format);
        
        let present_mode = selected_device_capabilities.supported_present_modes[0];
        log::info!(" -- Present Mode: {:?} ", present_mode);

        let wanted_extensions = selected_device_capabilities.wanted_extensions.to_owned();
        log::info!(" -- Wanted Extensions:");
        for (name, ok) in wanted_extensions.iter() {
            log::info!(" ---- {} {name}", if *ok {"✔"} else {"❌"});
        }

        let wanted_device_features = selected_device_capabilities.wanted_device_features.to_owned();
        log::info!(" -- Wanted Device Features:");
        for (name, ok) in wanted_device_features.iter() {
            log::info!(" ---- {} {name}", if *ok {"✔"} else {"❌"});
        }
        
        Ok(PhysicalDevice {
            inner: selected_device_capabilities.inner,
            name: selected_device_capabilities.name.to_owned(),
            device_type,
            limits: selected_device_capabilities.limits,
            graphics_queue_family: selected_device_capabilities.graphics_queues[0],
            present_queue_family: selected_device_capabilities.present_queues[0],
            wanted_extensions,
            surface_format,
            render_storage_image_format,
            depth_format,
            present_mode,
            wanted_device_features,
        })
    }

    pub(crate)  fn load_possible_physical_devices_capabilities(
        &mut self,
        surface: &Surface,
        required_extensions: &[String],
        wanted_extensions: &[String],
        required_device_features: &[String],
        wanted_device_features: &[String]
    ) -> Result<()> {
        if !self.physical_devices_capabilities.is_empty() {
            return Ok(())
        }

        let physical_devices = unsafe { self.inner.enumerate_physical_devices()? };

        self.physical_devices_capabilities = physical_devices
            .into_iter()
            .map(|pd| PhysicalDeviceCapabilities::new(
                &self,
                surface,
                pd,
                required_extensions,
                wanted_extensions,
                required_device_features,
                wanted_device_features))
            .collect::<Result<Vec<_>>>()?;

        Ok(())
    }
}


impl PhysicalDeviceCapabilities {
    pub(crate) fn new(
        instance: &Instance,
        surface: &Surface,
        inner: vk::PhysicalDevice,
        required_extensions: &[String],
        wanted_extensions: &[String],
        required_device_features: &[String],
        wanted_device_features: &[String],
    ) -> Result<Self> {
        let props = unsafe { instance.inner.get_physical_device_properties(inner) };

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
            unsafe { instance.inner.get_physical_device_queue_family_properties(inner) };
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
        let mut graphics = vec![];
        let mut present = vec![];
        for family in queue_families.iter().filter(|f| f.has_queues()) {
            if family.supports_graphics()
                && family.supports_compute()
                && family.supports_timestamp_queries()
            {
                graphics.push(*family);
            }

            if family.supports_present() {
                present.push(*family);
            }
        }

        // Extensions
        let extension_properties =
            unsafe { instance.inner.enumerate_device_extension_properties(inner)? };
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
        let surface_formats = unsafe {
            surface
                .inner
                .get_physical_device_surface_formats(inner, surface.surface_khr)?
        };

        let surface_formats_with_storage_bit: Vec<_> = surface_formats.to_owned().into_iter().filter(|format| {
            unsafe {
                let property = instance.inner.get_physical_device_format_properties(inner, format.format);
                property.optimal_tiling_features.contains(FormatFeatureFlags::STORAGE_IMAGE)
            }
        }).collect();

        // See https://stackoverflow.com/questions/75094730/why-prefer-non-srgb-format-for-vulkan-swapchain
        let rgba_formats = [
            Format::R8G8B8A8_SRGB,
            Format::R8G8B8A8_UNORM,
            Format::R8G8B8A8_SINT,
            Format::R8G8B8A8_SNORM,
            Format::R8G8B8A8_SSCALED,
            Format::R8G8B8A8_UINT,
        ];
        
        let render_storage_image_formats: Vec<_> = rgba_formats.into_iter().filter(|format| {
            unsafe {
                let property = instance.inner.get_physical_device_format_properties(inner, *format);
                property.optimal_tiling_features.contains(FormatFeatureFlags::STORAGE_IMAGE)
            }
        }).collect();
        
        // Depth Formats
        let all_depth_formats = [
            Format::D32_SFLOAT,
            Format::D32_SFLOAT_S8_UINT,
            Format::D16_UNORM,
            Format::D16_UNORM_S8_UINT,
            Format::D24_UNORM_S8_UINT
        ];

        let supported_depth_formats = all_depth_formats.into_iter().filter(|format| {
            unsafe {
                let property = instance.inner.get_physical_device_format_properties(inner, *format);
                property.optimal_tiling_features.contains(FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT)
            }
        }).collect::<Vec<_>>();


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
        let mut sorted_supported_present_modes = vec![];
        for wanted in present_mode_prio {
            if supported_present_modes.contains(&wanted) {
                sorted_supported_present_modes.push(wanted);
            }
        }
        
        // Device Features
        let mut required_features = PhysicalDeviceFeatures::new(required_device_features);
        unsafe { instance.inner.get_physical_device_features2(inner, &mut required_features.vulkan_features()) };
        let (required_device_features_ok, required_device_features) = required_features.result(required_device_features);

        let mut wanted_features = PhysicalDeviceFeatures::new(wanted_device_features);
        unsafe { instance.inner.get_physical_device_features2(inner, &mut wanted_features.vulkan_features()) };
        let (wanted_device_features_ok, wanted_device_features) = wanted_features.result(wanted_device_features);

        Ok(Self {
            inner,
            name,
            device_type,

            limits,
            limits_ok,

            queues: queue_families,
            graphics_queues: graphics, 
            present_queues: present,

            required_extensions,
            required_extensions_ok,

            wanted_extensions,
            wanted_extensions_ok,

            supported_surface_formats: surface_formats,
            supported_surface_formats_with_storage_bit: surface_formats_with_storage_bit,
            render_storage_image_formats,
            supported_depth_formats,

            supported_present_modes: sorted_supported_present_modes,

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
    pub(crate) fn new(required_device_features: &[String]) -> PhysicalDeviceFeatures {
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

    pub(crate) fn result(&self, required_device_features: &[String]) -> (bool, HashMap<String, bool>) {
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
        
        if !required_features.is_empty() {
            error!("Device Feature Check: {:?}, not implemented!", required_features);
            unimplemented!()
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



