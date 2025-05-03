use std::sync::Arc;
use ash::vk;

use crate::OctaResult;

use super::{Context, DescriptorPool, DescriptorSet, DescriptorSetLayout, Device};

#[derive(Debug)]
pub struct DescriptorHeap {
    pub pool: DescriptorPool,
    pub layout: DescriptorSetLayout,
    pub set: DescriptorSet,
} 

impl DescriptorHeap {
    pub(crate) fn new(device: Arc<Device>, heap_types: &[vk::DescriptorPoolSize]) -> OctaResult<Self> {
        
        let pool = DescriptorPool::new(
            device.clone(), 
            1,
            heap_types,
            vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND
        )?;

        let (bindings, flags) = heap_types.iter()
            .enumerate()
            .map(|(i, t)| {
                (vk::DescriptorSetLayoutBinding {
                    binding: i as _,
                    descriptor_count: t.descriptor_count,
                    descriptor_type: t.ty,
                    stage_flags: vk::ShaderStageFlags::ALL,
                    ..Default::default()
                },
                vk::DescriptorBindingFlags::PARTIALLY_BOUND 
                    | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND
                    | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING
                )
            })
            .collect::<(Vec<_>, Vec<_>)>();
        
        let layout = DescriptorSetLayout::new(device.clone(), &bindings, &flags)?;

        let set = pool.allocate_set(&layout)?;
       
        Ok(Self {
            pool,
            layout,
            set,
        })
    }
}

impl Context {
    pub fn create_descriptor_heap(
        &self,
        heap_types: &[vk::DescriptorPoolSize],
    ) -> OctaResult<DescriptorHeap> {
        DescriptorHeap::new(self.device.clone(), heap_types)
    }
}
