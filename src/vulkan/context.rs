use std::{fmt, sync::{Arc, Mutex}};
use anyhow::{Context as _, Result};
use ash::{vk};
use gpu_allocator::{
    vulkan::{Allocator, AllocatorCreateDesc},
    AllocatorDebugSettings,
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use crate::{engine::EngineFeatureValue, vulkan::{device::Device, instance::Instance, queue::Queue, surface::Surface}, CommandBuffer, CommandPool, EngineConfig, RayTracingContext};

#[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
use ash::extensions::khr::{DynamicRendering, Synchronization2};
use crate::vulkan::physical_device::PhysicalDevice;

use super::entry::Entry;

pub const DEBUG_GPU_ALLOCATOR: bool = false;

pub struct Context {
    pub allocator: Arc<Mutex<Allocator>>,
    pub command_pool: CommandPool,
    pub ray_tracing: Option<Arc<RayTracingContext>>,
    pub graphics_queue: Queue,
    pub present_queue: Queue,
    pub device: Arc<Device>,
    pub physical_device: PhysicalDevice,
    pub surface: Surface,
    pub instance: Instance,
    pub debug_printing: bool,
    pub shader_clock: bool,
    pub entry: Entry,

    #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
    pub(crate) synchronization2: Synchronization2,

    #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
    pub(crate) dynamic_rendering: DynamicRendering
}
impl Context {
    pub fn new<'a>(
        entry: Entry,
        window_handle: &'a dyn HasWindowHandle,
        display_handle: &'a dyn HasDisplayHandle,
        engine_config: &EngineConfig
    ) -> Result<Self> {

        // Vulkan instance
        let mut instance = Instance::new(&entry, display_handle, engine_config)
            .context("New Instance")?;

        // Vulkan surface
        let surface = Surface::new(&entry.inner, &instance, window_handle, display_handle)?;
        
        // Physical Device
        let mut required_extensions = engine_config.required_extensions.clone();
        let mut wanted_extensions = engine_config.wanted_extensions.clone();
        let mut required_device_features = engine_config.required_device_features.clone();
        let mut wanted_device_features = engine_config.wanted_device_features.clone();

        required_extensions.push("VK_KHR_swapchain".to_owned());
        
        if cfg!(any(vulkan_1_0, vulkan_1_1, vulkan_1_2)) {
            required_extensions.append(&mut vec![
                "VK_KHR_dynamic_rendering".to_owned(),
                "VK_KHR_synchronization2".to_owned(),
            ]);

            required_device_features.append(&mut vec![
                "dynamicRendering".to_owned(),
                "synchronization2".to_owned()
            ]);
        } else if cfg!(vulkan_1_3) {

            // The Engine does not use the extensions in vulkan 1.3 but the egui ash renderer dose so we still need to activate them.
            required_extensions.append(&mut vec![
                "VK_KHR_dynamic_rendering".to_owned(),
                "VK_KHR_synchronization2".to_owned(),
            ]);

            required_device_features.append(&mut vec![
                "dynamicRendering".to_owned(),
                "synchronization2".to_owned()
            ]);
        }

        // For Mac Support
        if cfg!(target_os = "macos") {
            required_extensions.push("VK_KHR_portability_subset".to_owned())
        }

        #[cfg(debug_assertions)]
        if engine_config.shader_debug_printing == EngineFeatureValue::Wanted {
            wanted_extensions.push("VK_KHR_shader_non_semantic_info".to_owned());

            wanted_device_features.append(&mut vec![
                "timelineSemaphore".to_owned(),
            ]);
        } else if engine_config.shader_debug_printing == EngineFeatureValue::Needed {
            required_extensions.push("VK_KHR_shader_non_semantic_info".to_owned());

            required_device_features.append(&mut vec![
                "timelineSemaphore".to_owned(),
            ]);
        }
        
        if engine_config.shader_debug_clock == EngineFeatureValue::Wanted {
            wanted_extensions.push("VK_KHR_shader_clock".to_owned());

            wanted_device_features.append(&mut vec![
                "deviceClock".to_owned(),
            ]);
        } else if engine_config.shader_debug_clock == EngineFeatureValue::Needed {
            required_extensions.push("VK_KHR_shader_clock".to_owned());

            wanted_device_features.append(&mut vec![
                "deviceClock".to_owned(),
            ]);

            required_device_features.append(&mut vec![
                "deviceClock".to_owned(),
            ]);
        };

        if engine_config.gl_ext_scalar_block_layout == EngineFeatureValue::Wanted {
            wanted_extensions.push("VK_EXT_scalar_block_layout".to_owned());
        } else if engine_config.gl_ext_scalar_block_layout == EngineFeatureValue::Needed {
            required_extensions.push("VK_EXT_scalar_block_layout".to_owned());
        };

        if engine_config.ray_tracing == EngineFeatureValue::Wanted {
            required_extensions.append(&mut vec![
                "VK_KHR_ray_tracing_pipeline".to_owned(),
                "VK_KHR_acceleration_structure".to_owned(),
                "VK_KHR_deferred_host_operations".to_owned(),
            ]);

            required_device_features.append(&mut vec![
                "rayTracingPipeline".to_owned(),
                "accelerationStructure".to_owned(),
                "runtimeDescriptorArray".to_owned(),
                "bufferDeviceAddress".to_owned(),
            ]);
        } else if engine_config.ray_tracing == EngineFeatureValue::Needed {
            wanted_extensions.append(&mut vec![
                "VK_KHR_ray_tracing_pipeline".to_owned(),
                "VK_KHR_acceleration_structure".to_owned(),
                "VK_KHR_deferred_host_operations".to_owned(),
            ]);

            wanted_device_features.append(&mut vec![
                "rayTracingPipeline".to_owned(),
                "accelerationStructure".to_owned(),
                "runtimeDescriptorArray".to_owned(),
                "bufferDeviceAddress".to_owned(),
            ]);
        }

        instance.load_possible_physical_devices_capabilities(
            &surface,
            &required_extensions,
            &wanted_extensions,
            &required_device_features,
            &wanted_device_features,
        )?;
        
        let render_storage_image_format_is_needed = engine_config.ray_tracing == EngineFeatureValue::Needed || engine_config.compute_rendering == EngineFeatureValue::Needed;
        let surface_formats_with_storage_bit_is_wanted = render_storage_image_format_is_needed || engine_config.ray_tracing == EngineFeatureValue::Wanted || engine_config.compute_rendering == EngineFeatureValue::Wanted;
        let physical_device = instance.select_suitable_physical_device(
            render_storage_image_format_is_needed,
            surface_formats_with_storage_bit_is_wanted,
        )?;
        
        let debug_printing = instance.debug_printing && *physical_device.wanted_extensions.get("VK_KHR_shader_non_semantic_info").unwrap_or(&false);
        let shader_clock = *physical_device.wanted_device_features.get("deviceClock").unwrap_or(&false)
            && *physical_device.wanted_extensions.get("VK_KHR_shader_clock").unwrap_or(&false);
        
        let possible_extensions = physical_device.wanted_extensions.iter().filter_map(|(name, b)| {
                if *b {
                    Some(name.to_owned())
                } else {
                    None
                }
            })
            .chain(required_extensions.into_iter())
            .collect();

        let possible_device_features = physical_device.wanted_device_features.iter().filter_map(|(name, b)| {
                if *b {
                    Some(name.to_owned())
                } else {
                    None
                }
            })
            .chain(required_device_features.into_iter())
            .collect();
        
        let device = Arc::new(Device::new(
            &instance,
            &physical_device,
            &possible_extensions,
            &possible_device_features,
        )?);

        #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
        let synchronization2 = Synchronization2::new(&instance.inner, &device.inner);

        #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
        let dynamic_rendering = DynamicRendering::new(&instance.inner, &device.inner);


        let graphics_queue = device.get_queue(
            physical_device.graphics_queue_family,
            0,

            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            synchronization2.to_owned()
        );
        let present_queue = device.get_queue(
            physical_device.present_queue_family,
            0,

            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            synchronization2.to_owned()
        );

        let ray_tracing = possible_extensions.contains(&"VK_KHR_ray_tracing_pipeline".to_owned()).then(|| {
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
            physical_device.graphics_queue_family,
            Some(vk::CommandPoolCreateFlags::TRANSIENT),

            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            synchronization2.to_owned(),

            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            dynamic_rendering.to_owned(),
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
            buffer_device_address: possible_device_features.contains(&"bufferDeviceAddress".to_owned()),
            allocation_sizes: Default::default(),
        })?;

        Ok(Self {
            allocator: Arc::new(Mutex::new(allocator)),
            command_pool,
            ray_tracing,
            present_queue,
            graphics_queue,
            device,
            physical_device,
            surface,
            instance,
            debug_printing,
            shader_clock,
            entry,

            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            synchronization2,

            #[cfg(any(vulkan_1_0, vulkan_1_1, vulkan_1_2))]
            dynamic_rendering
        })
    }
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
            .allocate_command_buffer(vk::CommandBufferLevel::PRIMARY, )?;

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

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("allocator", &self.allocator)
            .field("command_pool", &self.command_pool)
            .field("ray_tracing", &self.ray_tracing)
            .field("graphics_queue", &self.graphics_queue)
            .field("present_queue", &self.present_queue)
            .field("device", &self.device)
            .field("physical_device", &self.physical_device)
            .field("surface", &self.surface)
            .field("instance", &self.instance)
            .field("debug_printing", &self.debug_printing)
            .field("shader_clock", &self.shader_clock)
            .field("entry", &())
            //.field("synchronization2", &self.synchronization2)
            //.field("dynamic_rendering", &self.dynamic_rendering)
            .finish()
    }
}


