mod acceleration_structure;
mod pipeline;
mod shader_binding_table;

pub use acceleration_structure::*;
pub use pipeline::*;
pub use shader_binding_table::*;

use ash::{
    khr::{
        acceleration_structure::Device as AshAccelerationStructure,
        ray_tracing_pipeline::Device as AshRayTracingPipeline,
    },
    vk,
};

use crate::{
    vulkan::device::Device, vulkan::instance::Instance,
};
use crate::vulkan::physical_device::PhysicalDevice;

pub struct RayTracingContext<'a> {
    pub pipeline: AshRayTracingPipeline,
    pub acceleration_structure: AshAccelerationStructure,
}

impl<'a> RayTracingContext<'a> {
    pub(crate) fn new(instance: &Instance, pdevice: &PhysicalDevice, device: &Device) -> Self {
        let pipeline_properties =
            unsafe { AshRayTracingPipeline::get_properties(&instance.inner, pdevice.inner) };
        let pipeline_fn = AshRayTracingPipeline::new(&instance.inner, &device.inner);

        let acceleration_structure_properties =
            unsafe { AshAccelerationStructure::get_properties(&instance.inner, pdevice.inner) };
        let acceleration_structure_fn =
            AshAccelerationStructure::new(&instance.inner, &device.inner);

        Self {
            pipeline_properties,
            pipeline: pipeline_fn,
            acceleration_structure_properties,
            acceleration_structure: acceleration_structure_fn,
        }
    }
}
