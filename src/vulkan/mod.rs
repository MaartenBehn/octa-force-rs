pub extern crate ash;
pub extern crate ash_window;
pub extern crate gpu_allocator;

mod align;
mod buffer;
mod command;
mod context;
mod descriptor;
pub mod descriptor_heap;
mod device;
mod image;
mod instance;
pub mod physical_device;
mod pipeline;
mod query;
mod queue;
mod ray_tracing;
mod sampler;
pub mod sampler_pool;
mod surface;
mod swapchain;
mod sync;
mod extensions;

pub mod push_constant;
pub mod utils;

use std::fmt::{Debug, Formatter};
pub use buffer::*;
pub use command::*;
pub use context::*;
pub use descriptor::*;
pub use device::*;
pub use image::*;
pub use pipeline::*;
pub use query::*;
pub use queue::*;
pub use ray_tracing::*;
pub use sampler::*;
pub use swapchain::*;
pub use sync::*;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Version {
    Vk1_0,
    Vk1_1,
    Vk1_2,
    Vk1_3,
}

impl Version {
    pub(crate) fn make_api_version(&self) -> u32 {
        match self {
            Version::Vk1_0 => {ash::vk::make_api_version(0, 1, 0, 0)}
            Version::Vk1_1 => {ash::vk::make_api_version(0, 1, 1, 0)}
            Version::Vk1_2 => {ash::vk::make_api_version(0, 1, 2, 0)}
            Version::Vk1_3 => {ash::vk::make_api_version(0, 1, 3, 0)}
        }
    }
}

impl Debug for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Version::Vk1_0 => {f.write_str("1.0.")}
            Version::Vk1_1 => {f.write_str("1.1.")}
            Version::Vk1_2 => {f.write_str("1.2.")}
            Version::Vk1_3 => {f.write_str("1.3.")}
        }
    }
}
