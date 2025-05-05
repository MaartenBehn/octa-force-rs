use std::{cell::RefCell, collections::HashMap, fmt::{self, Octal}, rc::Rc, sync::Arc};

use ash::vk::{self, DescriptorPoolSize};
use egui::emath::OrderedFloat;
use log::error;

use crate::OctaResult;

use super::{Context, DescriptorPool, DescriptorSet, DescriptorSetLayout, Device};

#[derive(Debug)]
pub struct SamplerPool {
    device: Arc<Device>,
    pool: Rc<DescriptorPool>,
    samplers: HashMap<SamplerConfig, vk::Sampler>
}

#[derive(Eq, Hash, PartialEq, PartialOrd, Ord)]
struct SamplerConfig {
    pub flags: vk::SamplerCreateFlags,
    pub mag_filter: vk::Filter,
    pub min_filter: vk::Filter,
    pub mipmap_mode: vk::SamplerMipmapMode,
    pub address_mode_u: vk::SamplerAddressMode,
    pub address_mode_v: vk::SamplerAddressMode,
    pub address_mode_w: vk::SamplerAddressMode,
    pub mip_lod_bias: OrderedFloat<f32>,
    pub anisotropy_enable: bool,
    pub max_anisotropy: OrderedFloat<f32>,
    pub compare_enable: bool,
    pub compare_op: vk::CompareOp,
    pub min_lod: OrderedFloat<f32>,
    pub max_lod: OrderedFloat<f32>,
    pub border_color: vk::BorderColor,
    pub unnormalized_coordinates: bool,
}

#[derive(Debug)]
pub struct SamplerSetHandle {
    pub set: DescriptorSet,
    pub layout: DescriptorSetLayout,
    pool: Rc<DescriptorPool>,
}

impl SamplerPool {
    pub(crate) fn new(device: Arc<Device>, max_num: usize) -> OctaResult<Self> {
        let pool = DescriptorPool::new(
            device.clone(), 
            max_num as _,
            &[
                DescriptorPoolSize {
                    ty: vk::DescriptorType::SAMPLER,
                    descriptor_count: max_num as _, 
                }
            ],
            vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET
        )?;

        Ok(Self {
            device: device.clone(),
            pool: Rc::new(pool),
            samplers: Default::default()
        })
    }

    pub(crate) fn get_sampler(&mut self, info: &vk::SamplerCreateInfo) -> OctaResult<vk::Sampler> {
        
        let config =SamplerConfig { 
            flags: info.flags, 
            mag_filter: info.mag_filter, 
            min_filter: info.min_filter, 
            mipmap_mode: info.mipmap_mode, 
            address_mode_u: info.address_mode_u, 
            address_mode_v: info.address_mode_v, 
            address_mode_w: info.address_mode_w, 
            mip_lod_bias: info.mip_lod_bias.into(), 
            anisotropy_enable: info.anisotropy_enable == 1, 
            max_anisotropy: info.max_anisotropy.into(), 
            compare_enable: info.compare_enable == 1, 
            compare_op: info.compare_op, 
            min_lod: info.min_lod.into(), 
            max_lod: info.max_lod.into(), 
            border_color: info.border_color, 
            unnormalized_coordinates: info.unnormalized_coordinates == 1, 
        };

        if let Some(sampler) = self.samplers.get(&config) {
            Ok(sampler.clone())
        } else {
            let inner = unsafe { self.device.inner.create_sampler(info, None)? };
            self.samplers.insert(config, inner.clone());

            Ok(inner)
        }
    }

    pub fn get_set(&mut self, infos: &[vk::SamplerCreateInfo]) -> OctaResult<SamplerSetHandle> {
      
        let samplers = infos.iter()
            .map(|info| {
                let sampler = self.get_sampler(info)?;
                Ok(vec![sampler])
            })
            .collect::<OctaResult<Vec<_>>>()?;

        let bindings = samplers.iter()
            .enumerate()
            .map(|(i, sampler)| {
                let binding = vk::DescriptorSetLayoutBinding::default()
                    .stage_flags(vk::ShaderStageFlags::ALL)
                    .descriptor_type(vk::DescriptorType::SAMPLER)
                    .descriptor_count(1)
                    .binding(i as _)
                    .immutable_samplers(sampler);
                binding
            })
            .collect::<Vec<_>>();

        let layout = DescriptorSetLayout::new(
            self.device.clone(), 
            &bindings, 
            vk::DescriptorSetLayoutCreateFlags::empty(),
            &[])?;

        let set = self.pool.allocate_set(&layout)?;

        Ok(SamplerSetHandle { 
            set, 
            layout, 
            pool: self.pool.clone()
        })
    }
}

impl Context {
    pub fn create_sampler_pool(&self, max_num: usize) -> OctaResult<SamplerPool> {
       SamplerPool::new(self.device.clone(), max_num) 
    }
}

impl Drop for SamplerPool {
    fn drop(&mut self) {
        for (_, sampler) in self.samplers.iter() {
            unsafe {
                self.device.inner.destroy_sampler(*sampler, None);
            }
        }
    }
}

impl Drop for SamplerSetHandle {
    fn drop(&mut self) {
        let res = self.pool.free_set(&self.set);
        if res.is_err() {
            error!("Failed to free Descriptor Set: {}", res.unwrap_err());
        }
    }
}

impl fmt::Debug for SamplerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SamplerConfig")
            .field("flags", &self.flags)
            .field("mag_filter", &self.mag_filter)
            .field("min_filter", &self.min_filter)
            .field("mipmap_mode", &self.mipmap_mode)
            .field("address_mode_u", &self.address_mode_u)
            .field("address_mode_v", &self.address_mode_v)
            .field("address_mode_w", &self.address_mode_w)
            .field("mip_lod_bias", &self.mip_lod_bias.into_inner())
            .field("anisotropy_enable", &self.anisotropy_enable)
            .field("max_anisotropy", &self.max_anisotropy.into_inner())
            .field("compare_enable", &self.compare_enable)
            .field("compare_op", &self.compare_op)
            .field("min_lod", &self.min_lod.into_inner())
            .field("max_lod", &self.max_lod.into_inner())
            .field("border_color", &self.border_color)
            .field("unnormalized_coordinates", &self.unnormalized_coordinates)
            .finish()
    }
}
