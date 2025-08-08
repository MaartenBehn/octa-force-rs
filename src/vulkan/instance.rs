use std::{ffi::{c_char, c_void, CStr, CString}, fmt};
use anyhow::{Context, Result};
use ash::{
    ext::debug_utils::Instance as DebugUtils,
    vk::{self, DebugUtilsMessengerEXT},
    Instance as AshInstance,
};
use log::{debug, info};
use raw_window_handle::HasDisplayHandle;
use crate::{vulkan::physical_device::PhysicalDeviceCapabilities, EngineConfig};
use super::entry::Entry;

#[allow(deprecated)]
use raw_window_handle::HasRawDisplayHandle;

#[cfg(debug_assertions)]
use anyhow::bail;

#[cfg(debug_assertions)]
use crate::{engine::EngineFeatureValue};


const REQUIRED_DEBUG_LAYERS: [&str; 1] = ["VK_LAYER_KHRONOS_validation"];

#[allow(dead_code)]
pub struct Instance {
    pub(crate) inner: AshInstance,
    debug_report_callback: Option<(DebugUtils, DebugUtilsMessengerEXT)>,
    pub(crate) physical_devices_capabilities: Vec<PhysicalDeviceCapabilities>,
    pub(crate) validation_layers: bool,
    pub(crate) debug_printing: bool,
}

impl Instance {
    pub(crate) fn new(
        entry: &Entry,
        display_handle: &dyn HasDisplayHandle,
        engine_config: &EngineConfig,
    ) -> Result<Self> {

        #[cfg(vulkan_1_0)] let version = crate::vulkan::Version::Vk1_0;
        #[cfg(vulkan_1_1)] let version = crate::vulkan::Version::Vk1_1;
        #[cfg(vulkan_1_2)] let version = crate::vulkan::Version::Vk1_2;
        #[cfg(vulkan_1_3)] let version = crate::vulkan::Version::Vk1_3;

        info!("Using Vulkan Version {:?}", version);

        // Vulkan instance
        let app_name = CString::new(engine_config.name.as_bytes())?;
        let app_info = vk::ApplicationInfo::default()
            .application_name(app_name.as_c_str())
            .api_version(version.make_api_version());

        #[allow(deprecated)]
        let mut extension_names =
            ash_window::enumerate_required_extensions(display_handle.raw_display_handle()?)?
                .to_vec();

        let mut instance_create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info);

        // Validation Layers
        #[cfg(not(debug_assertions))]
        let validation_layers = false;
        #[cfg(debug_assertions)]
        let mut validation_layers = false;
        #[cfg(debug_assertions)]
        let (_layer_names, layer_names_ptrs) = get_validation_layer_names_and_pointers();
        #[cfg(debug_assertions)]
        if engine_config.validation_layers != EngineFeatureValue::NotUsed {
            
            if entry.check_layer_support(&REQUIRED_DEBUG_LAYERS)? {
                extension_names.push(ash::ext::debug_utils::NAME.as_ptr());
                instance_create_info = instance_create_info.enabled_layer_names(&layer_names_ptrs);
                validation_layers = true;

            } else if engine_config.validation_layers == EngineFeatureValue::Needed {
                bail!("Validation Layers are needed but not supported by hardware.")
            }
        }

        // Debug Printing
        #[cfg(not(debug_assertions))]
        let debug_printing = false;
        #[cfg(debug_assertions)]
        let mut debug_printing = false;
        #[cfg(debug_assertions)]
        let mut validation_features = vk::ValidationFeaturesEXT::default();
        #[cfg(debug_assertions)]
        if engine_config.shader_debug_printing != EngineFeatureValue::NotUsed {
            if validation_layers {
                validation_features = validation_features.enabled_validation_features(&[vk::ValidationFeatureEnableEXT::DEBUG_PRINTF]);
                instance_create_info = instance_create_info.push_next(&mut validation_features);
                debug_printing = true;
            } else if engine_config.shader_debug_printing == EngineFeatureValue::Needed {
                bail!("Debug Printing is needed but not supported by hardware.")
            }
        }

        // For Mac Support
        if cfg!(target_os = "macos") {
            extension_names.push(ash::khr::portability_enumeration::NAME.as_ptr());
            extension_names.push(ash::khr::get_physical_device_properties2::NAME.as_ptr());

            instance_create_info.flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
        }

        instance_create_info = instance_create_info.enabled_extension_names(&extension_names);

        let extensions = extension_names.iter()
            .map(|ptr| unsafe{ CStr::from_ptr(*ptr)})
            .collect::<Vec<_>>();
        
        debug!("Extensions: {extensions:?}");

        // Creating Instance
        let inner = unsafe { 
            entry.inner.create_instance(&instance_create_info, None)
                .context("Creating Instance")?
        };

        // Validation
        let debug_report_callback = if validation_layers {
            Some(setup_debug_messenger(&entry, &inner))
        } else {
            None
        };

        Ok(Self {
            inner,
            debug_report_callback,
            physical_devices_capabilities: vec![],
            validation_layers,
            debug_printing,
        })
    }
}



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

#[allow(dead_code)]
unsafe extern "system" fn vulkan_debug_callback(
    flag: vk::DebugUtilsMessageSeverityFlagsEXT,
    typ: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _: *mut c_void,
) -> vk::Bool32 { unsafe {
    use vk::DebugUtilsMessageSeverityFlagsEXT as Flag;

    let message = CStr::from_ptr((*p_callback_data).p_message);
    match flag {
        Flag::VERBOSE => log::trace!("{:?} - {}", typ, message.to_str().unwrap()),
        Flag::INFO => { log::info!("{:?} - {}", typ, message.to_str().unwrap()) },
        Flag::WARNING => log::warn!("{:?} - {}", typ, message.to_str().unwrap()),
        _ => log::error!("{:?} - {} \n", typ, message.to_str().unwrap()),
    }
    vk::FALSE
}}

/// Setup the debug message if validation layers are enabled.
#[allow(dead_code)]
pub fn setup_debug_messenger(
    _entry: &Entry,
    _instance: &AshInstance,
) -> (DebugUtils, vk::DebugUtilsMessengerEXT) {
    let create_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
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

    let debug_utils = DebugUtils::new(&_entry.inner, _instance);
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

impl fmt::Debug for Instance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Instance")
            .field("inner", &())
            .field("debug_report_callback", &())
            .field("physical_devices_capabilities", &self.physical_devices_capabilities)
            .field("validation_layers", &self.validation_layers).field("debug_printing", &self.debug_printing)
            .finish()
    }
}
