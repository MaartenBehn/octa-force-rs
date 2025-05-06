use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use convert_case::{Case, Casing};

use anyhow::{bail, Result};
use ash::{vk};
use ash::vk::{Format, FormatFeatureFlags, PhysicalDevice8BitStorageFeatures, PhysicalDeviceAccelerationStructureFeaturesKHR, PhysicalDeviceFeatures2, PhysicalDeviceRayTracingPipelineFeaturesKHR, PhysicalDeviceShaderClockFeaturesKHR, PhysicalDeviceType, PhysicalDeviceVulkan11Features, PhysicalDeviceVulkan12Features, PhysicalDeviceVulkan13Features, PresentModeKHR, SurfaceFormatKHR};
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
//#[allow(unused)]
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

                log::info!(" -- Device Type: {:?}", device_capabilities.device_type);
                if device_capabilities.device_type == PhysicalDeviceType::VIRTUAL_GPU {
                    log::info!(" ---- -150 Points");
                    minus_points -= 150;
                } else if device_capabilities.device_type == PhysicalDeviceType::CPU {
                    log::info!(" ---- -100 Points");
                    minus_points -= 100;
                } else if device_capabilities.device_type == PhysicalDeviceType::INTEGRATED_GPU {
                    log::info!(" ---- -50 Points");
                    minus_points -= 50;
                } else if device_capabilities.device_type == PhysicalDeviceType::OTHER {
                    log::info!(" ---- -10 Points");
                    minus_points -= 10;
                }

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
            minus_points2.cmp(minus_points1).then(c1.limits.max_memory_allocation_count.cmp(&c2.limits.max_memory_allocation_count))
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
            Format::D24_UNORM_S8_UINT,
            Format::D16_UNORM,
            Format::D16_UNORM_S8_UINT,
            Format::D32_SFLOAT,
            Format::D32_SFLOAT_S8_UINT,
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
        let required_features = PhysicalDeviceFeatures::new(required_device_features);
        let mut res_required_features = required_features.to_owned();
        unsafe { instance.inner.get_physical_device_features2(inner, &mut res_required_features.vulkan_features()) };
        let (required_device_features_ok, required_device_features) = res_required_features.get_mask_result(&required_features);

        let wanted_features = PhysicalDeviceFeatures::new(wanted_device_features);
        let mut res_wanted_features = required_features.to_owned();
        unsafe { instance.inner.get_physical_device_features2(inner, &mut res_wanted_features.vulkan_features()) };
        let (wanted_device_features_ok, wanted_device_features) = res_wanted_features.get_mask_result(&wanted_features);

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

#[derive(Default, Clone, Copy)]
pub struct PhysicalDeviceFeatures<'a> {
    pub features: vk::PhysicalDeviceFeatures,
    pub ray_tracing_feature: PhysicalDeviceRayTracingPipelineFeaturesKHR<'a>,
    pub acceleration_struct_feature: PhysicalDeviceAccelerationStructureFeaturesKHR<'a>,
    pub features11: PhysicalDeviceVulkan11Features<'a>,
    pub features12: PhysicalDeviceVulkan12Features<'a>,
    pub features13: PhysicalDeviceVulkan13Features<'a>,
    pub clock_feature: PhysicalDeviceShaderClockFeaturesKHR<'a>,
    pub storage8_features: PhysicalDevice8BitStorageFeatures<'a>
}

#[macro_export]
macro_rules! any_used {
    ( $self:ident, $feature:ident, $( pub $x:ident : Bool32 ),* $(,)? ) => {
        {
            let mut used = false;
            $(
                used |= ($self.$feature.$x == ash::vk::TRUE);
            )*
            used
        }
    };
}

#[macro_export]
macro_rules! get_mask_result {
    ( $self:ident, $other:ident : $( $feature:ident, $( pub $x:ident : Bool32 ),* $(,)? ):* $(:)? ) => {
        {
            let mut res: HashMap<String, bool> = HashMap::new(); 
            let mut ok = true;
            $(
                $(
                    if $other.$feature.$x == ash::vk::TRUE {
                        res.insert(format!("{}",  stringify!($x).to_case(Case::Camel)), $self.$feature.$x == ash::vk::TRUE);
                        ok &= $self.$feature.$x == ash::vk::TRUE;
                    }
                )*
            )*
            (ok, res)
        }
    };
}

#[macro_export]
macro_rules! set_from_list {
    ( $self:ident, $list:ident : $( $feature:ident, $( pub $x:ident : Bool32 ),* $(,)? ):* $(:)? ) => {
        {
            $(
                $(
                   $self.$feature = $self.$feature.$x($list.take(&format!("{}", stringify!($x).to_case(Case::Camel))).is_some());
                )*
            )*
        }
    }
}

impl<'a> PhysicalDeviceFeatures<'a> {

    pub(crate) fn vulkan_features(&mut self) -> PhysicalDeviceFeatures2{
        let mut res = PhysicalDeviceFeatures2::default()
            .features(self.features);

        if any_used!(self, features11,
    pub storage_buffer16_bit_access: Bool32,
    pub uniform_and_storage_buffer16_bit_access: Bool32,
    pub storage_push_constant16: Bool32,
    pub storage_input_output16: Bool32,
    pub multiview: Bool32,
    pub multiview_geometry_shader: Bool32,
    pub multiview_tessellation_shader: Bool32,
    pub variable_pointers_storage_buffer: Bool32,
    pub variable_pointers: Bool32,
    pub protected_memory: Bool32,
    pub sampler_ycbcr_conversion: Bool32,
    pub shader_draw_parameters: Bool32,
        ) {
            res = res.push_next(&mut self.features11);
        }

        if any_used!(self, features12,
    pub sampler_mirror_clamp_to_edge: Bool32,
    pub draw_indirect_count: Bool32,
    pub storage_buffer8_bit_access: Bool32,
    pub uniform_and_storage_buffer8_bit_access: Bool32,
    pub storage_push_constant8: Bool32,
    pub shader_buffer_int64_atomics: Bool32,
    pub shader_shared_int64_atomics: Bool32,
    pub shader_float16: Bool32,
    pub shader_int8: Bool32,
    pub descriptor_indexing: Bool32,
    pub shader_input_attachment_array_dynamic_indexing: Bool32,
    pub shader_uniform_texel_buffer_array_dynamic_indexing: Bool32,
    pub shader_storage_texel_buffer_array_dynamic_indexing: Bool32,
    pub shader_uniform_buffer_array_non_uniform_indexing: Bool32,
    pub shader_sampled_image_array_non_uniform_indexing: Bool32,
    pub shader_storage_buffer_array_non_uniform_indexing: Bool32,
    pub shader_storage_image_array_non_uniform_indexing: Bool32,
    pub shader_input_attachment_array_non_uniform_indexing: Bool32,
    pub shader_uniform_texel_buffer_array_non_uniform_indexing: Bool32,
    pub shader_storage_texel_buffer_array_non_uniform_indexing: Bool32,
    pub descriptor_binding_uniform_buffer_update_after_bind: Bool32,
    pub descriptor_binding_sampled_image_update_after_bind: Bool32,
    pub descriptor_binding_storage_image_update_after_bind: Bool32,
    pub descriptor_binding_storage_buffer_update_after_bind: Bool32,
    pub descriptor_binding_uniform_texel_buffer_update_after_bind: Bool32,
    pub descriptor_binding_storage_texel_buffer_update_after_bind: Bool32,
    pub descriptor_binding_update_unused_while_pending: Bool32,
    pub descriptor_binding_partially_bound: Bool32,
    pub descriptor_binding_variable_descriptor_count: Bool32,
    pub runtime_descriptor_array: Bool32,
    pub sampler_filter_minmax: Bool32,
    pub scalar_block_layout: Bool32,
    pub imageless_framebuffer: Bool32,
    pub uniform_buffer_standard_layout: Bool32,
    pub shader_subgroup_extended_types: Bool32,
    pub separate_depth_stencil_layouts: Bool32,
    pub host_query_reset: Bool32,
    pub timeline_semaphore: Bool32,
    pub buffer_device_address: Bool32,
    pub buffer_device_address_capture_replay: Bool32,
    pub buffer_device_address_multi_device: Bool32,
    pub vulkan_memory_model: Bool32,
    pub vulkan_memory_model_device_scope: Bool32,
    pub vulkan_memory_model_availability_visibility_chains: Bool32,
    pub shader_output_viewport_index: Bool32,
    pub shader_output_layer: Bool32,
    pub subgroup_broadcast_dynamic_id: Bool32, 
        ) {
            res = res.push_next(&mut self.features12);
        }

        if any_used!(self, features13,
    pub robust_image_access: Bool32,
    pub inline_uniform_block: Bool32,
    pub descriptor_binding_inline_uniform_block_update_after_bind: Bool32,
    pub pipeline_creation_cache_control: Bool32,
    pub private_data: Bool32,
    pub shader_demote_to_helper_invocation: Bool32,
    pub shader_terminate_invocation: Bool32,
    pub subgroup_size_control: Bool32,
    pub compute_full_subgroups: Bool32,
    pub synchronization2: Bool32,
    pub texture_compression_astc_hdr: Bool32,
    pub shader_zero_initialize_workgroup_memory: Bool32,
    pub dynamic_rendering: Bool32,
    pub shader_integer_dot_product: Bool32,
    pub maintenance4: Bool32,
        ) {
            res = res.push_next(&mut self.features13);
        }
        
        if any_used!(self, clock_feature,
    pub shader_subgroup_clock: Bool32,
    pub shader_device_clock: Bool32,
        ) {
            res = res.push_next(&mut self.clock_feature);
        }

        if any_used!(self, ray_tracing_feature,
    pub ray_tracing_pipeline: Bool32,
    pub ray_tracing_pipeline_shader_group_handle_capture_replay: Bool32,
    pub ray_tracing_pipeline_shader_group_handle_capture_replay_mixed: Bool32,
    pub ray_tracing_pipeline_trace_rays_indirect: Bool32,
    pub ray_traversal_primitive_culling: Bool32,
        ) {
            res = res.push_next(&mut self.ray_tracing_feature);
        }
        
        if any_used!(self, acceleration_struct_feature,
    pub acceleration_structure: Bool32,
    pub acceleration_structure_capture_replay: Bool32,
    pub acceleration_structure_indirect_build: Bool32,
    pub acceleration_structure_host_commands: Bool32,
    pub descriptor_binding_acceleration_structure_update_after_bind: Bool32,
        ) {
            res = res.push_next(&mut self.acceleration_struct_feature);
        }

        if any_used!(self, storage8_features,
    pub storage_buffer8_bit_access: Bool32,
    pub uniform_and_storage_buffer8_bit_access: Bool32,
    pub storage_push_constant8: Bool32,
        ) {
            res = res.push_next(&mut self.storage8_features);
        }

        res
    }

    pub fn new(list: &[String]) -> Self {
        
        let mut set: HashSet<_> = list.into_iter().map(|s| s.to_owned()).collect();
        let mut res = Self::default();

        set_from_list!(res, set
            :features,
    pub robust_buffer_access: Bool32,
    pub full_draw_index_uint32: Bool32,
    pub image_cube_array: Bool32,
    pub independent_blend: Bool32,
    pub geometry_shader: Bool32,
    pub tessellation_shader: Bool32,
    pub sample_rate_shading: Bool32,
    pub dual_src_blend: Bool32,
    pub logic_op: Bool32,
    pub multi_draw_indirect: Bool32,
    pub draw_indirect_first_instance: Bool32,
    pub depth_clamp: Bool32,
    pub depth_bias_clamp: Bool32,
    pub fill_mode_non_solid: Bool32,
    pub depth_bounds: Bool32,
    pub wide_lines: Bool32,
    pub large_points: Bool32,
    pub alpha_to_one: Bool32,
    pub multi_viewport: Bool32,
    pub sampler_anisotropy: Bool32,
    pub texture_compression_etc2: Bool32,
    pub texture_compression_astc_ldr: Bool32,
    pub texture_compression_bc: Bool32,
    pub occlusion_query_precise: Bool32,
    pub pipeline_statistics_query: Bool32,
    pub vertex_pipeline_stores_and_atomics: Bool32,
    pub fragment_stores_and_atomics: Bool32,
    pub shader_tessellation_and_geometry_point_size: Bool32,
    pub shader_image_gather_extended: Bool32,
    pub shader_storage_image_extended_formats: Bool32,
    pub shader_storage_image_multisample: Bool32,
    pub shader_storage_image_read_without_format: Bool32,
    pub shader_storage_image_write_without_format: Bool32,
    pub shader_uniform_buffer_array_dynamic_indexing: Bool32,
    pub shader_sampled_image_array_dynamic_indexing: Bool32,
    pub shader_storage_buffer_array_dynamic_indexing: Bool32,
    pub shader_storage_image_array_dynamic_indexing: Bool32,
    pub shader_clip_distance: Bool32,
    pub shader_cull_distance: Bool32,
    pub shader_float64: Bool32,
    pub shader_int64: Bool32,
    pub shader_int16: Bool32,
    pub shader_resource_residency: Bool32,
    pub shader_resource_min_lod: Bool32,
    pub sparse_binding: Bool32,
    pub sparse_residency_buffer: Bool32,
    pub sparse_residency_image2_d: Bool32,
    pub sparse_residency_image3_d: Bool32,
    pub sparse_residency2_samples: Bool32,
    pub sparse_residency4_samples: Bool32,
    pub sparse_residency8_samples: Bool32,
    pub sparse_residency16_samples: Bool32,
    pub sparse_residency_aliased: Bool32,
    pub variable_multisample_rate: Bool32,
    pub inherited_queries: Bool32,
            :features11,
    pub storage_buffer16_bit_access: Bool32,
    pub uniform_and_storage_buffer16_bit_access: Bool32,
    pub storage_push_constant16: Bool32,
    pub storage_input_output16: Bool32,
    pub multiview: Bool32,
    pub multiview_geometry_shader: Bool32,
    pub multiview_tessellation_shader: Bool32,
    pub variable_pointers_storage_buffer: Bool32,
    pub variable_pointers: Bool32,
    pub protected_memory: Bool32,
    pub sampler_ycbcr_conversion: Bool32,
    pub shader_draw_parameters: Bool32,
            :features12,
    pub sampler_mirror_clamp_to_edge: Bool32,
    pub draw_indirect_count: Bool32,
    pub storage_buffer8_bit_access: Bool32,
    pub uniform_and_storage_buffer8_bit_access: Bool32,
    pub storage_push_constant8: Bool32,
    pub shader_buffer_int64_atomics: Bool32,
    pub shader_shared_int64_atomics: Bool32,
    pub shader_float16: Bool32,
    pub shader_int8: Bool32,
    pub descriptor_indexing: Bool32,
    pub shader_input_attachment_array_dynamic_indexing: Bool32,
    pub shader_uniform_texel_buffer_array_dynamic_indexing: Bool32,
    pub shader_storage_texel_buffer_array_dynamic_indexing: Bool32,
    pub shader_uniform_buffer_array_non_uniform_indexing: Bool32,
    pub shader_sampled_image_array_non_uniform_indexing: Bool32,
    pub shader_storage_buffer_array_non_uniform_indexing: Bool32,
    pub shader_storage_image_array_non_uniform_indexing: Bool32,
    pub shader_input_attachment_array_non_uniform_indexing: Bool32,
    pub shader_uniform_texel_buffer_array_non_uniform_indexing: Bool32,
    pub shader_storage_texel_buffer_array_non_uniform_indexing: Bool32,
    pub descriptor_binding_uniform_buffer_update_after_bind: Bool32,
    pub descriptor_binding_sampled_image_update_after_bind: Bool32,
    pub descriptor_binding_storage_image_update_after_bind: Bool32,
    pub descriptor_binding_storage_buffer_update_after_bind: Bool32,
    pub descriptor_binding_uniform_texel_buffer_update_after_bind: Bool32,
    pub descriptor_binding_storage_texel_buffer_update_after_bind: Bool32,
    pub descriptor_binding_update_unused_while_pending: Bool32,
    pub descriptor_binding_partially_bound: Bool32,
    pub descriptor_binding_variable_descriptor_count: Bool32,
    pub runtime_descriptor_array: Bool32,
    pub sampler_filter_minmax: Bool32,
    pub scalar_block_layout: Bool32,
    pub imageless_framebuffer: Bool32,
    pub uniform_buffer_standard_layout: Bool32,
    pub shader_subgroup_extended_types: Bool32,
    pub separate_depth_stencil_layouts: Bool32,
    pub host_query_reset: Bool32,
    pub timeline_semaphore: Bool32,
    pub buffer_device_address: Bool32,
    pub buffer_device_address_capture_replay: Bool32,
    pub buffer_device_address_multi_device: Bool32,
    pub vulkan_memory_model: Bool32,
    pub vulkan_memory_model_device_scope: Bool32,
    pub vulkan_memory_model_availability_visibility_chains: Bool32,
    pub shader_output_viewport_index: Bool32,
    pub shader_output_layer: Bool32,
    pub subgroup_broadcast_dynamic_id: Bool32,
            :features13,
    pub robust_image_access: Bool32,
    pub inline_uniform_block: Bool32,
    pub descriptor_binding_inline_uniform_block_update_after_bind: Bool32,
    pub pipeline_creation_cache_control: Bool32,
    pub private_data: Bool32,
    pub shader_demote_to_helper_invocation: Bool32,
    pub shader_terminate_invocation: Bool32,
    pub subgroup_size_control: Bool32,
    pub compute_full_subgroups: Bool32,
    pub synchronization2: Bool32,
    pub texture_compression_astc_hdr: Bool32,
    pub shader_zero_initialize_workgroup_memory: Bool32,
    pub dynamic_rendering: Bool32,
    pub shader_integer_dot_product: Bool32,
    pub maintenance4: Bool32,
            :ray_tracing_feature,
    pub ray_tracing_pipeline: Bool32,
    pub ray_tracing_pipeline_shader_group_handle_capture_replay: Bool32,
    pub ray_tracing_pipeline_shader_group_handle_capture_replay_mixed: Bool32,
    pub ray_tracing_pipeline_trace_rays_indirect: Bool32,
    pub ray_traversal_primitive_culling: Bool32,
            :acceleration_struct_feature,
    pub acceleration_structure: Bool32,
    pub acceleration_structure_capture_replay: Bool32,
    pub acceleration_structure_indirect_build: Bool32,
    pub acceleration_structure_host_commands: Bool32,
    pub descriptor_binding_acceleration_structure_update_after_bind: Bool32,
            :clock_feature,
    pub shader_subgroup_clock: Bool32,
    pub shader_device_clock: Bool32,
            :storage8_features,
    pub storage_buffer8_bit_access: Bool32,
    pub uniform_and_storage_buffer8_bit_access: Bool32,
    pub storage_push_constant8: Bool32,
);  
        if !set.is_empty() {
            error!("Device Feature Check: {:?}, not known!", set);
            panic!()
        }
        res
    }

    pub fn get_mask_result(&self, mask: &PhysicalDeviceFeatures) -> (bool, HashMap<String, bool>) {

        get_mask_result!(self, mask
            :features,
    pub robust_buffer_access: Bool32,
    pub full_draw_index_uint32: Bool32,
    pub image_cube_array: Bool32,
    pub independent_blend: Bool32,
    pub geometry_shader: Bool32,
    pub tessellation_shader: Bool32,
    pub sample_rate_shading: Bool32,
    pub dual_src_blend: Bool32,
    pub logic_op: Bool32,
    pub multi_draw_indirect: Bool32,
    pub draw_indirect_first_instance: Bool32,
    pub depth_clamp: Bool32,
    pub depth_bias_clamp: Bool32,
    pub fill_mode_non_solid: Bool32,
    pub depth_bounds: Bool32,
    pub wide_lines: Bool32,
    pub large_points: Bool32,
    pub alpha_to_one: Bool32,
    pub multi_viewport: Bool32,
    pub sampler_anisotropy: Bool32,
    pub texture_compression_etc2: Bool32,
    pub texture_compression_astc_ldr: Bool32,
    pub texture_compression_bc: Bool32,
    pub occlusion_query_precise: Bool32,
    pub pipeline_statistics_query: Bool32,
    pub vertex_pipeline_stores_and_atomics: Bool32,
    pub fragment_stores_and_atomics: Bool32,
    pub shader_tessellation_and_geometry_point_size: Bool32,
    pub shader_image_gather_extended: Bool32,
    pub shader_storage_image_extended_formats: Bool32,
    pub shader_storage_image_multisample: Bool32,
    pub shader_storage_image_read_without_format: Bool32,
    pub shader_storage_image_write_without_format: Bool32,
    pub shader_uniform_buffer_array_dynamic_indexing: Bool32,
    pub shader_sampled_image_array_dynamic_indexing: Bool32,
    pub shader_storage_buffer_array_dynamic_indexing: Bool32,
    pub shader_storage_image_array_dynamic_indexing: Bool32,
    pub shader_clip_distance: Bool32,
    pub shader_cull_distance: Bool32,
    pub shader_float64: Bool32,
    pub shader_int64: Bool32,
    pub shader_int16: Bool32,
    pub shader_resource_residency: Bool32,
    pub shader_resource_min_lod: Bool32,
    pub sparse_binding: Bool32,
    pub sparse_residency_buffer: Bool32,
    pub sparse_residency_image2_d: Bool32,
    pub sparse_residency_image3_d: Bool32,
    pub sparse_residency2_samples: Bool32,
    pub sparse_residency4_samples: Bool32,
    pub sparse_residency8_samples: Bool32,
    pub sparse_residency16_samples: Bool32,
    pub sparse_residency_aliased: Bool32,
    pub variable_multisample_rate: Bool32,
    pub inherited_queries: Bool32,
            :features11,
    pub storage_buffer16_bit_access: Bool32,
    pub uniform_and_storage_buffer16_bit_access: Bool32,
    pub storage_push_constant16: Bool32,
    pub storage_input_output16: Bool32,
    pub multiview: Bool32,
    pub multiview_geometry_shader: Bool32,
    pub multiview_tessellation_shader: Bool32,
    pub variable_pointers_storage_buffer: Bool32,
    pub variable_pointers: Bool32,
    pub protected_memory: Bool32,
    pub sampler_ycbcr_conversion: Bool32,
    pub shader_draw_parameters: Bool32,
            :features12,
    pub sampler_mirror_clamp_to_edge: Bool32,
    pub draw_indirect_count: Bool32,
    pub storage_buffer8_bit_access: Bool32,
    pub uniform_and_storage_buffer8_bit_access: Bool32,
    pub storage_push_constant8: Bool32,
    pub shader_buffer_int64_atomics: Bool32,
    pub shader_shared_int64_atomics: Bool32,
    pub shader_float16: Bool32,
    pub shader_int8: Bool32,
    pub descriptor_indexing: Bool32,
    pub shader_input_attachment_array_dynamic_indexing: Bool32,
    pub shader_uniform_texel_buffer_array_dynamic_indexing: Bool32,
    pub shader_storage_texel_buffer_array_dynamic_indexing: Bool32,
    pub shader_uniform_buffer_array_non_uniform_indexing: Bool32,
    pub shader_sampled_image_array_non_uniform_indexing: Bool32,
    pub shader_storage_buffer_array_non_uniform_indexing: Bool32,
    pub shader_storage_image_array_non_uniform_indexing: Bool32,
    pub shader_input_attachment_array_non_uniform_indexing: Bool32,
    pub shader_uniform_texel_buffer_array_non_uniform_indexing: Bool32,
    pub shader_storage_texel_buffer_array_non_uniform_indexing: Bool32,
    pub descriptor_binding_uniform_buffer_update_after_bind: Bool32,
    pub descriptor_binding_sampled_image_update_after_bind: Bool32,
    pub descriptor_binding_storage_image_update_after_bind: Bool32,
    pub descriptor_binding_storage_buffer_update_after_bind: Bool32,
    pub descriptor_binding_uniform_texel_buffer_update_after_bind: Bool32,
    pub descriptor_binding_storage_texel_buffer_update_after_bind: Bool32,
    pub descriptor_binding_update_unused_while_pending: Bool32,
    pub descriptor_binding_partially_bound: Bool32,
    pub descriptor_binding_variable_descriptor_count: Bool32,
    pub runtime_descriptor_array: Bool32,
    pub sampler_filter_minmax: Bool32,
    pub scalar_block_layout: Bool32,
    pub imageless_framebuffer: Bool32,
    pub uniform_buffer_standard_layout: Bool32,
    pub shader_subgroup_extended_types: Bool32,
    pub separate_depth_stencil_layouts: Bool32,
    pub host_query_reset: Bool32,
    pub timeline_semaphore: Bool32,
    pub buffer_device_address: Bool32,
    pub buffer_device_address_capture_replay: Bool32,
    pub buffer_device_address_multi_device: Bool32,
    pub vulkan_memory_model: Bool32,
    pub vulkan_memory_model_device_scope: Bool32,
    pub vulkan_memory_model_availability_visibility_chains: Bool32,
    pub shader_output_viewport_index: Bool32,
    pub shader_output_layer: Bool32,
    pub subgroup_broadcast_dynamic_id: Bool32,
            :features13,
    pub robust_image_access: Bool32,
    pub inline_uniform_block: Bool32,
    pub descriptor_binding_inline_uniform_block_update_after_bind: Bool32,
    pub pipeline_creation_cache_control: Bool32,
    pub private_data: Bool32,
    pub shader_demote_to_helper_invocation: Bool32,
    pub shader_terminate_invocation: Bool32,
    pub subgroup_size_control: Bool32,
    pub compute_full_subgroups: Bool32,
    pub synchronization2: Bool32,
    pub texture_compression_astc_hdr: Bool32,
    pub shader_zero_initialize_workgroup_memory: Bool32,
    pub dynamic_rendering: Bool32,
    pub shader_integer_dot_product: Bool32,
    pub maintenance4: Bool32,
            :ray_tracing_feature,
    pub ray_tracing_pipeline: Bool32,
    pub ray_tracing_pipeline_shader_group_handle_capture_replay: Bool32,
    pub ray_tracing_pipeline_shader_group_handle_capture_replay_mixed: Bool32,
    pub ray_tracing_pipeline_trace_rays_indirect: Bool32,
    pub ray_traversal_primitive_culling: Bool32,
            :acceleration_struct_feature,
    pub acceleration_structure: Bool32,
    pub acceleration_structure_capture_replay: Bool32,
    pub acceleration_structure_indirect_build: Bool32,
    pub acceleration_structure_host_commands: Bool32,
    pub descriptor_binding_acceleration_structure_update_after_bind: Bool32,
            :clock_feature,
    pub shader_subgroup_clock: Bool32,
    pub shader_device_clock: Bool32,
            :storage8_features,
    pub storage_buffer8_bit_access: Bool32,
    pub uniform_and_storage_buffer8_bit_access: Bool32,
    pub storage_push_constant8: Bool32,
)
    }
}
