use std::{cell::RefCell, rc::Rc, sync::Arc, usize};
use ash::vk;

use crate::OctaResult;

use super::{Context, DescriptorPool, DescriptorSet, DescriptorSetLayout, Device, ImageView};

#[derive(Debug)]
pub struct DescriptorHeap {
    device: Arc<Device>,
    pub heap_types: Vec<vk::DescriptorPoolSize>,
    pub pool: DescriptorPool,
    pub layout: DescriptorSetLayout,
    pub set: DescriptorSet,
    allocator: Rc<RefCell<DescriptorHeapHandleAllocator>>,
}

pub type DescriptorHandleValue = u32;

#[derive(Debug)]
pub struct DescriptorHandle {
    pub value: DescriptorHandleValue,
    allocator: Rc<RefCell<DescriptorHeapHandleAllocator>>
}

impl DescriptorHeap {
    pub(crate) fn new(device: Arc<Device>, heap_types: Vec<vk::DescriptorPoolSize>) -> OctaResult<Self> {
        
        let pool = DescriptorPool::new(
            device.clone(), 
            1,
            &heap_types,
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
        
        let layout = DescriptorSetLayout::new(
            device.clone(), 
            &bindings, 
            vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL,
            &flags)?;

        let set = pool.allocate_set(&layout)?;
      
        let allocator = Rc::new(RefCell::new(DescriptorHeapHandleAllocator::new()));

        Ok(Self {
            device,
            heap_types,
            pool,
            layout,
            set,
            allocator,
        })
    }

    pub fn create_image_handle(&mut self, view: &ImageView, usage: vk::ImageUsageFlags) -> OctaResult<DescriptorHandle> {
        let handle = self.allocator.borrow_mut().alloc();

        let img_info = vk::DescriptorImageInfo::default()
            .image_view(view.inner);

        let wds = vk::WriteDescriptorSet::default()
            .dst_set(self.set.inner)
            .dst_array_element(handle)
            .descriptor_count(1);

        if usage.contains(vk::ImageUsageFlags::SAMPLED) {
            let img_info = img_info.image_layout(
                if usage.contains(vk::ImageUsageFlags::STORAGE) { 
                    vk::ImageLayout::GENERAL 
                } else { 
                    vk::ImageLayout::READ_ONLY_OPTIMAL
                }
            );

            let binding_index = self.heap_types.iter()
                .position(|t| t.ty == vk::DescriptorType::SAMPLED_IMAGE )
                .ok_or(anyhow::anyhow!("Descriptor heap must have sampled image type!"))?;

            let wds = wds.descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .dst_binding(binding_index as _)
                .image_info(std::slice::from_ref(&img_info));

            unsafe { self.device.inner.update_descriptor_sets(&[wds], &[]) };
        } 

        if usage.contains(vk::ImageUsageFlags::STORAGE) {
            let img_info = img_info.image_layout(vk::ImageLayout::GENERAL);

            let binding_index = self.heap_types.iter()
                .position(|t| t.ty == vk::DescriptorType::STORAGE_IMAGE )
                .ok_or(anyhow::anyhow!("Descriptor heap must have storage image type!"))?;

            let wds = wds.descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .dst_binding(binding_index as _)
                .image_info(std::slice::from_ref(&img_info));
            
            unsafe { self.device.inner.update_descriptor_sets(&[wds], &[]) };
        } 

        Ok(DescriptorHandle { value: handle, allocator: self.allocator.clone() })
    }
}

impl Context {
    pub fn create_descriptor_heap(
        &self,
        heap_types: Vec<vk::DescriptorPoolSize>,
    ) -> OctaResult<DescriptorHeap> {
        DescriptorHeap::new(self.device.clone(), heap_types)
    }
}


#[derive(Debug)]
pub(crate) struct DescriptorHeapHandleAllocator {
    next_free_word_id: usize,
    used_map: Vec<u64>,
}

impl DescriptorHeapHandleAllocator {

    pub fn new() -> Self {
        let capacity = 1024 * 64;
        
        Self {
            next_free_word_id: 0,
            used_map: vec![0; (capacity + 63) / 64],
        }
    }

    pub fn alloc(&mut self) -> u32 {
        for i in 0..self.used_map.len() {
            let wi = (i + self.next_free_word_id) % self.used_map.len();
            if self.used_map[wi] != !0 {
                let j = self.used_map[wi].count_ones();
                self.used_map[wi] |= 1 << j;
                self.next_free_word_id = wi;
            
                return wi as u32 * 64 + j;
            }
        }
        return !0;
    }
    
    pub fn free(&mut self, addr: u32) {
        self.used_map[(addr / 64) as usize] &= !(1 << (addr & 63));
    }
}


impl Drop for DescriptorHandle {
    fn drop(&mut self) {
        self.allocator.borrow_mut().free(self.value);
    }
}
