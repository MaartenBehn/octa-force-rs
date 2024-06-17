use std::ffi::{c_char, c_void, CStr, CString};

use anyhow::{bail, Result};
use ash::{
    extensions::ext::DebugUtils,
    vk::{self, DebugUtilsMessengerEXT},
    Entry, Instance as AshInstance,
};
use ash::vk::{Format, SurfaceFormatKHR};
use log::info;
use raw_window_handle::HasRawDisplayHandle;

use crate::{vulkan::physical_device::PhysicalDevice, vulkan::surface::Surface, EngineConfig};
use crate::EngineFeatureValue::{Needed, NotUsed};

#[allow(dead_code)]
pub struct Instance {
    pub(crate) inner: AshInstance,
    debug_report_callback: Option<(DebugUtils, DebugUtilsMessengerEXT)>,
    physical_devices: Vec<PhysicalDevice>,
    pub(crate) validation_layers: bool,
    pub(crate) debug_printing: bool,
}

impl Instance {
    pub(crate) fn new(
        entry: &Entry,
        display_handle: &dyn HasRawDisplayHandle,
        engine_config: &EngineConfig,
    ) -> Result<Self> {

        #[cfg(vulkan_1_0)] let version = crate::vulkan::Version::VK_1_0;
        #[cfg(vulkan_1_1)] let version = crate::vulkan::Version::VK_1_1;
        #[cfg(vulkan_1_2)] let version = crate::vulkan::Version::VK_1_2;
        #[cfg(vulkan_1_3)] let version = crate::vulkan::Version::VK_1_3;

        info!("Using Vulkan Version {:?}", version);

        // Vulkan instance
        let app_name = CString::new(engine_config.name.as_bytes())?;
        let app_info = vk::ApplicationInfo::builder()
            .application_name(app_name.as_c_str())
            .api_version(version.make_api_version());

        let mut extension_names =
            ash_window::enumerate_required_extensions(display_handle.raw_display_handle())?
                .to_vec();

        let mut instance_create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info);

        // Validation Layers
        let mut validation_layers = false;
        #[cfg(debug_assertions)]
        let (_layer_names, layer_names_ptrs) = get_validation_layer_names_and_pointers();
        #[cfg(debug_assertions)]
        if engine_config.validation_layers != NotUsed {
            if check_validation_layer_support(&entry) {
                extension_names.push(DebugUtils::name().as_ptr());
                instance_create_info = instance_create_info.enabled_layer_names(&layer_names_ptrs);
                validation_layers = true;
            } else if engine_config.validation_layers == Needed {
                bail!("Validation Layers are needed but not supported by hardware.")
            }
        }

        // Debug Printing
        let mut debug_printing = false;
        #[cfg(debug_assertions)]
        let mut validation_features = vk::ValidationFeaturesEXT::builder();
        #[cfg(debug_assertions)]
        if engine_config.shader_debug_printing != NotUsed && validation_layers {
            if validation_layers {
                validation_features = validation_features.enabled_validation_features(&[vk::ValidationFeatureEnableEXT::DEBUG_PRINTF]);
                instance_create_info = instance_create_info.push_next(&mut validation_features);
                debug_printing = true;
            } else if engine_config.shader_debug_printing == Needed {
                bail!("Debug Printing is needed but not supported by hardware.")
            }
        }

        // For Mac Support
        if cfg!(target_os = "macos") {
            extension_names.push(vk::KhrPortabilityEnumerationFn::name().as_ptr());
            extension_names.push(vk::KhrGetPhysicalDeviceProperties2Fn::name().as_ptr());

            instance_create_info.flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
        }



        instance_create_info = instance_create_info.enabled_extension_names(&extension_names);

        // Creating Instance
        let inner = unsafe { entry.create_instance(&instance_create_info, None)? };

        // Validation
        let debug_report_callback = if validation_layers {
            Some(setup_debug_messenger(&entry, &inner))
        } else {
            None
        };

        Ok(Self {
            inner,
            debug_report_callback,
            physical_devices: vec![],
            validation_layers,
            debug_printing,
        })
    }

    pub(crate) fn enumerate_physical_devices(
        &mut self,
        surface: &Surface,
        required_extensions: &[String],
        wanted_extensions: &[String],
        wanted_surface_formats: &[SurfaceFormatKHR],
        wanted_depth_formats: &[Format],
        required_device_features: &[String],
        wanted_device_features: &[String],
    ) -> Result<&[PhysicalDevice]> {
        if self.physical_devices.is_empty() {
            let physical_devices = unsafe { self.inner.enumerate_physical_devices()? };

            let physical_devices = physical_devices
                .into_iter()
                .map(|pd| PhysicalDevice::new(
                    &self.inner,
                    surface,
                    pd,
                    required_extensions,
                    wanted_extensions,
                    wanted_surface_formats,
                    wanted_depth_formats,
                    required_device_features,
                    wanted_device_features))
                .collect::<Result<Vec<_>>>()?;

            self.physical_devices = physical_devices;
        }

        Ok(&self.physical_devices)
    }
}


const REQUIRED_DEBUG_LAYERS: [&str; 1] = ["VK_LAYER_KHRONOS_validation"];

/// Get the pointers to the validation layers names.
/// Also return the corresponding `CString` to avoid dangling pointers.
#[allow(dead_code)]
pub fn get_validation_layer_names_and_pointers() -> (Vec<CString>, Vec<*const c_char>) {
    let layer_names = REQUIRED_DEBUG_LAYERS
        .iter()
        .map(|name| CString::new(*name).unwrap())
        .collect::<Vec<_>>();
    let layer_names_ptrs = layer_names
        .iter()
        .map(|name| name.as_ptr())
        .collect::<Vec<_>>();
    (layer_names, layer_names_ptrs)
}

/// Check if the required validation set in `REQUIRED_LAYERS`
/// are supported by the Vulkan instance.
#[allow(dead_code)]
pub fn check_validation_layer_support(entry: &Entry) -> bool {

    let mut found = false;
    for required in REQUIRED_DEBUG_LAYERS.iter() {
        found |= entry
            .enumerate_instance_layer_properties()
            .unwrap()
            .iter()
            .any(|layer| {
                let name = unsafe { CStr::from_ptr(layer.layer_name.as_ptr()) };
                let name = name.to_str().expect("Failed to get layer name pointer");
                required == &name
            });
    }

    if !found {
        log::warn!("Validation layer not supported: {:?}", REQUIRED_DEBUG_LAYERS);
    }

    found
}

#[allow(dead_code)]
unsafe extern "system" fn vulkan_debug_callback(
    flag: vk::DebugUtilsMessageSeverityFlagsEXT,
    typ: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _: *mut c_void,
) -> vk::Bool32 {
    use vk::DebugUtilsMessageSeverityFlagsEXT as Flag;

    let message = CStr::from_ptr((*p_callback_data).p_message);
    match flag {
        Flag::VERBOSE => log::trace!("{:?} - {:?}", typ, message),
        Flag::INFO => {
            //log::info!("{:?} - {:?}", typ, message)
        },
        Flag::WARNING => log::warn!("{:?} - {:?}", typ, message),
        _ => log::error!("{:?} - {:?}", typ, message),
    }
    vk::FALSE
}

/// Setup the debug message if validation layers are enabled.
#[allow(dead_code)]
pub fn setup_debug_messenger(
    _entry: &Entry,
    _instance: &AshInstance,
) -> (DebugUtils, vk::DebugUtilsMessengerEXT) {
    let create_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
        )
        .pfn_user_callback(Some(vulkan_debug_callback));

    let debug_utils = DebugUtils::new(_entry, _instance);
    let debug_utils_messenger = unsafe {
        debug_utils
            .create_debug_utils_messenger(&create_info, None)
            .unwrap()
    };

    (debug_utils, debug_utils_messenger)
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            if let Some((utils, messenger)) = self.debug_report_callback.take() {
                utils.destroy_debug_utils_messenger(messenger, None);
            }
            self.inner.destroy_instance(None);
        }
    }
}
