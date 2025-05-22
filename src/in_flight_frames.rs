use std::time::Duration;

use ash::vk;

use crate::{vulkan::{Context, Fence, Semaphore, TimestampQueryPool}, OctaResult};

#[derive(Debug)]
pub struct InFlightFrames {
    per_frames: Vec<PerFrame>,
    pub current_index: usize,
    pub num_frames: usize,
}

#[derive(Debug)]
struct PerFrame {
    image_available_semaphore: Semaphore,
    render_finished_semaphore: Semaphore,
    fence: Fence,
    timing_query_pool: TimestampQueryPool<2>,
}

impl InFlightFrames {
    pub(crate) fn new(context: &Context, frame_count: usize) -> OctaResult<Self> {
        let sync_objects = (0..frame_count)
            .map(|_i| {
                let image_available_semaphore = context.create_semaphore()?;
                let render_finished_semaphore = context.create_semaphore()?;
                let fence = context.create_fence(Some(vk::FenceCreateFlags::SIGNALED))?;

                let timing_query_pool = context.create_timestamp_query_pool()?;

                Ok(PerFrame {
                    image_available_semaphore,
                    render_finished_semaphore,
                    fence,
                    timing_query_pool,
                })
            })
            .collect::<OctaResult<Vec<_>>>()?;

        Ok(Self {
            per_frames: sync_objects,
            current_index: 0,
            num_frames: frame_count,
        })
    }

    pub(crate) fn next(&mut self) {
        self.current_index = (self.current_index + 1) % self.per_frames.len();
    }

    pub(crate) fn image_available_semaphore(&self) -> &Semaphore {
        &self.per_frames[self.current_index].image_available_semaphore
    }

    pub(crate) fn render_finished_semaphore(&self) -> &Semaphore {
        &self.per_frames[self.current_index].render_finished_semaphore
    }

    pub(crate) fn fence(&self) -> &Fence {
        &self.per_frames[self.current_index].fence
    }

    pub(crate) fn timing_query_pool(&self) -> &TimestampQueryPool<2> {
        &self.per_frames[self.current_index].timing_query_pool
    }

    pub(crate) fn gpu_frame_time_ms(&self) -> OctaResult<Duration> {
        let result = self.timing_query_pool().wait_for_all_results()?;
        let time = Duration::from_nanos(result[1].saturating_sub(result[0]));

        Ok(time)
    }
}
