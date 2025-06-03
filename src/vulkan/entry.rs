use std::ffi::CStr;

use anyhow::Context as _;
use ash::khr::{surface, wayland_surface};
use itertools::Itertools;

use crate::OctaResult;

#[derive(Clone)]
pub struct Entry {
    pub inner: ash::Entry,
}

impl Entry {
    pub fn new() -> Self {
        let inner = ash::Entry::linked();

        Self {
            inner,
        }
    }

    pub fn supports_surface(&self) -> OctaResult<bool> {
        self.check_extension_support(&[surface::NAME.to_str()?])
    }

    pub fn supports_wayland(&self) -> OctaResult<bool> {
        self.check_extension_support(&[wayland_surface::NAME.to_str()?])
    }

    pub fn check_extension_support(&self, extensions: &[&str]) -> OctaResult<bool> {
        extensions.iter()
            .map(|wanted_extension|  {
                let found = unsafe{ 
                    self.inner.enumerate_instance_extension_properties(None)?
                        .iter()
                        .map::<OctaResult<bool>, _>(|extension| {
                            let name = extension.extension_name_as_c_str()?;
                            let name = name.to_str().context("Failed to get layer name pointer")?;
                            Ok(wanted_extension == &name)
                        })
                        .process_results(|mut iter | iter
                            .any(|b| b)
                        )?
                };

                if !found {
                    log::warn!("Extension not supported: {:?}", wanted_extension);
                }

                Ok(found)
            })
            .process_results(|mut iter| iter 
                .all(|b| b)
            ) 
    }


    pub fn check_layer_support(&self, layers: &[&str]) -> OctaResult<bool> {

        layers.iter()
            .map(|wanted_layer|  {
                let found = unsafe{ 
                    self.inner.enumerate_instance_layer_properties()?
                        .iter()
                        .map::<OctaResult<bool>, _>(|layer| {
                            let name = CStr::from_ptr(layer.layer_name.as_ptr());
                            let name = name.to_str().context("Failed to get layer name pointer")?;
                            Ok(wanted_layer == &name)
                        })
                        .process_results(|mut iter | iter
                            .any(|b| b)
                        )?
                };

                if !found {
                    log::warn!("Layer not supported: {:?}", wanted_layer);
                }

                Ok(found)
            })
            .process_results(|mut iter| iter 
                .all(|b| b)
            ) 
    }
}

