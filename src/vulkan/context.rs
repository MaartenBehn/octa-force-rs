use std::cmp::Ordering;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Result};
use ash::{vk, Entry};
use ash::vk::{PhysicalDeviceType, SurfaceFormatKHR};
use gpu_allocator::{
    vulkan::{Allocator, AllocatorCreateDesc},
    AllocatorDebugSettings,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use crate::{
    vulkan::device::{Device},
    vulkan::instance::Instance,
    vulkan::physical_device::PhysicalDevice,
    vulkan::queue::{Queue, QueueFamily},
    vulkan::surface::Surface,
    CommandBuffer, CommandPool, RayTracingContext, Version, VERSION_1_0,
};

pub const DEBUG_GPU_ALLOCATOR: bool = false;

pub struct Context {
    pub allocator: Arc<Mutex<Allocator>>,
    pub command_pool: CommandPool,
    pub ray_tracing: Option<Arc<RayTracingContext>>,
    pub graphics_queue: Queue,
    pub present_queue: Queue,
    pub device: Arc<Device>,
    pub present_queue_family: QueueFamily,
    pub graphics_queue_family: QueueFamily,
    pub physical_device: PhysicalDevice,
    pub surface: Surface,
    pub instance: Instance,
    pub debug_printing: bool,
    _entry: Entry,
}

pub struct ContextBuilder<'a> {
    window_handle: &'a dyn HasRawWindowHandle,
    display_handle: &'a dyn HasRawDisplayHandle,
    vulkan_version: Version,
    app_name: &'a str,
    required_extensions: Vec<String>,
    wanted_extensions: Vec<String>,
    wanted_surface_formats: Vec<SurfaceFormatKHR>,
    required_device_features: Vec<String>,
    wanted_device_features: Vec<String>,
}

impl<'a> ContextBuilder<'a> {
    pub fn new(
        window_handle: &'a dyn HasRawWindowHandle,
        display_handle: &'a dyn HasRawDisplayHandle,
    ) -> Self {

        let required_extensions = vec![
            "VK_KHR_swapchain".to_owned(),
            "VK_KHR_dynamic_rendering".to_owned()];
        
        let mut wanted_extensions = vec![];
        #[cfg(debug_assertions)]
        wanted_extensions.push("VK_KHR_shader_non_semantic_info".to_owned());

        let required_device_features = vec![
            "dynamicRendering".to_owned(),
            "synchronization2".to_owned()
        ];

        Self {
            window_handle,
            display_handle,
            vulkan_version: VERSION_1_0,
            app_name: "",
            required_extensions,
            wanted_extensions,
            wanted_surface_formats: Vec::new(),
            required_device_features,
            wanted_device_features: Vec::new(),
        }
    }

    pub fn vulkan_version(self, vulkan_version: Version) -> Self {
        Self {
            vulkan_version,
            ..self
        }
    }

    pub fn app_name(self, app_name: &'a str) -> Self {
        Self { app_name, ..self }
    }

    pub fn required_extensions(self, required_extensions: Vec<&str>) -> Self {
        let mut extensions = self.required_extensions;
        extensions.extend(required_extensions.into_iter().map(|a| a.to_owned()));
        extensions.sort_unstable();
        extensions.dedup();

        Self {
            required_extensions: extensions,
            ..self
        }
    }

    pub fn wanted_extensions(self, wanted_extensions: Vec<&str>) -> Self {
        let mut extensions = self.wanted_extensions;
        extensions.extend(wanted_extensions.into_iter().map(|a| a.to_owned()));
        extensions.sort_unstable();
        extensions.dedup();

        Self {
            wanted_extensions: extensions,
            ..self
        }
    }

    pub fn wanted_surface_formats(self, wanted_surface_formats: Vec<SurfaceFormatKHR>) -> Self {
        Self {
            wanted_surface_formats,
            ..self
        }
    }

    pub fn required_device_features(self, required_device_features: Vec<&str>) -> Self {
        let mut features = self.required_device_features;
        features.extend(required_device_features.into_iter().map(|a| a.to_owned()));
        features.sort_unstable();
        features.dedup();

        Self {
            required_device_features: features,
            ..self
        }
    }

    pub fn wanted_device_features(self, wanted_device_features: Vec<&str>) -> Self {
        let mut features = self.wanted_device_features;
        features.extend(wanted_device_features.into_iter().map(|a| a.to_owned()));
        features.sort_unstable();
        features.dedup();

        Self {
            wanted_device_features: features,
            ..self
        }
    }

    pub fn build(self) -> Result<Context> {
        Context::new(self)
    }
}

impl Context {
    fn new(
        ContextBuilder {
            window_handle,
            display_handle,
            vulkan_version,
            app_name,
            required_extensions,
            wanted_extensions,
            wanted_surface_formats,
            required_device_features,
            wanted_device_features
        }: ContextBuilder,
    ) -> Result<Self> {
        // Vulkan instance
        let entry = Entry::linked();
        let mut instance = Instance::new(&entry, display_handle, vulkan_version, app_name)?;

        // Vulkan surface
        let surface = Surface::new(&entry, &instance, window_handle, display_handle)?;

        let physical_devices = instance.enumerate_physical_devices(
            &surface,
            &required_extensions,
            &wanted_extensions,
            &wanted_surface_formats,
            &required_device_features,
            &wanted_device_features,
        )?;
        let physical_device = select_suitable_physical_device(physical_devices)?;
        let debug_printing = instance.debug_printing && physical_device.wanted_extensions["VK_KHR_shader_non_semantic_info"];

        log::info!("Selected physical device: {:?}", physical_device.name);

        let graphics_queue_family = physical_device.graphics_queue.unwrap();
        let present_queue_family = physical_device.present_queue.unwrap();
        let queue_families = [graphics_queue_family, present_queue_family];
        let device = Arc::new(Device::new(
            &instance,
            &physical_device,
            &queue_families,
            &required_extensions,
            &required_device_features,
        )?);
        let graphics_queue = device.get_queue(graphics_queue_family, 0);
        let present_queue = device.get_queue(present_queue_family, 0);

        let ray_tracing = required_extensions.contains(&"VK_KHR_ray_tracing_pipeline".to_owned()).then(|| {
            let ray_tracing =
                Arc::new(RayTracingContext::new(&instance, &physical_device, &device));
            log::debug!(
                "Ray tracing pipeline properties {:#?}",
                ray_tracing.pipeline_properties
            );
            log::debug!(
                "Acceleration structure properties {:#?}",
                ray_tracing.acceleration_structure_properties
            );
            ray_tracing
        });

        let command_pool = CommandPool::new(
            device.clone(),
            ray_tracing.clone(),
            graphics_queue_family,
            Some(vk::CommandPoolCreateFlags::TRANSIENT),
        )?;

        // Gpu allocator
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.inner.clone(),
            device: device.inner.clone(),
            physical_device: physical_device.inner,
            debug_settings: AllocatorDebugSettings {
                log_allocations: DEBUG_GPU_ALLOCATOR,
                log_frees: DEBUG_GPU_ALLOCATOR,
                log_memory_information: DEBUG_GPU_ALLOCATOR,
                log_leaks_on_shutdown: true,
                store_stack_traces: false,
                log_stack_traces: false,
            },
            buffer_device_address: required_device_features.contains(&"bufferDeviceAddress".to_owned()),
            allocation_sizes: Default::default(),
        })?;

        Ok(Self {
            allocator: Arc::new(Mutex::new(allocator)),
            command_pool,
            ray_tracing,
            present_queue,
            graphics_queue,
            device,
            present_queue_family,
            graphics_queue_family,
            physical_device,
            surface,
            instance,
            debug_printing,
            _entry: entry,
        })
    }
}

fn select_suitable_physical_device(
    devices: &[PhysicalDevice],
) -> Result<PhysicalDevice> {
    log::debug!("Choosing Vulkan physical device.");

    let mut seen_names = Vec::new();

    let mut supported_devices: Vec<_> = devices
        .iter()
        .filter(|device| {
            let name = &device.name;

            if seen_names.contains(name){
                return false;
            }
            seen_names.push(name.to_owned());
            log::debug!("Possible Device: {name}");

            let mut ok = true;

            if device.graphics_queue.is_none() {
                ok = false;
                log::debug!(" -- No Graphics Queue");
            }

            if device.present_queue.is_none() {
                ok = false;
                log::debug!(" -- No Present Queue");
            }

            if device.surface_format.is_none() {
                ok = false;
                log::debug!(" -- No Surface Format");
                for format in device.supported_surface_formats.iter() {
                    log::debug!(" ---- {format:?} supported.");
                }
            }

            if device.present_mode.is_none() {
                ok = false;
                log::debug!(" -- No Present Mode");
                for format in device.supported_present_modes.iter() {
                    log::debug!(" ---- {format:?} supported.");
                }
            }

            if !device.limits_ok {
                ok = false;
                log::debug!(" -- Limits not ok");
            }

            if !device.required_extensions_ok {
                ok = false;
                log::debug!(" -- Extensions not ok");
                for (n, b) in device.required_extensions.iter() {
                    if !b {
                        log::debug!(" ---- {n} missing.");
                    }
                }
            }

            if !device.wanted_device_features_ok {
                log::debug!(" -- Not all wanted Extensions");
                for (n, b) in device.wanted_extensions.iter() {
                    if !b {
                        log::debug!(" ---- {n} missing.");
                    }
                }
            }

            if !device.required_device_features_ok {
                ok = false;
                log::debug!(" -- Device Features not ok");
                for (n, b) in device.required_device_features.iter() {
                    if !b {
                        log::debug!(" ---- {n} missing.");
                    }
                }
            }

            if !device.wanted_device_features_ok {
                ok = false;
                log::debug!(" -- Not all wanted Device Features");
                for (n, b) in device.wanted_device_features.iter() {
                    if !b {
                        log::debug!(" ---- {n} missing.");
                    }
                }
            }

            if ok {
                log::debug!(" -- Ok");
            }

            return ok
        }).collect();

    if supported_devices.is_empty() {
        bail!("No suitable Device found.")
    }

    supported_devices.sort_by(|a, b| {
        a.limits.max_memory_allocation_count.cmp(&b.limits.max_memory_allocation_count)
    });

    supported_devices.sort_by(|a, b| {
        if a.device_type == PhysicalDeviceType::DISCRETE_GPU && b.device_type == PhysicalDeviceType::INTEGRATED_GPU {
            return Ordering::Greater
        }
        if a.device_type == PhysicalDeviceType::INTEGRATED_GPU && b.device_type == PhysicalDeviceType::DISCRETE_GPU {
            return Ordering::Less
        }
        Ordering::Equal
    });



    Ok( supported_devices[0].clone())
}

impl Context {
    pub fn device_wait_idle(&self) -> Result<()> {
        unsafe { self.device.inner.device_wait_idle()? };

        Ok(())
    }

    pub fn execute_one_time_commands<R, F: FnOnce(&CommandBuffer) -> R>(
        &self,
        executor: F,
    ) -> Result<R> {
        let command_buffer = self
            .command_pool
            .allocate_command_buffer(vk::CommandBufferLevel::PRIMARY)?;

        // Begin recording
        command_buffer.begin(Some(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;

        // Execute user function
        let executor_result = executor(&command_buffer);

        // End recording
        command_buffer.end()?;

        // Submit and wait
        let fence = self.create_fence(None)?;
        self.graphics_queue
            .submit(&command_buffer, None, None, &fence)?;
        fence.wait(None)?;

        // Free
        self.command_pool.free_command_buffer(&command_buffer)?;

        Ok(executor_result)
    }
}
