use std::time::Duration;

use ash::vk;

use crate::{vulkan::{Context, Fence, Semaphore, TimestampQueryPool}, OctaResult};

#[derive(Debug)]
pub struct InFlightFrames {
    per_in_flight_frame: Vec<PerInFlightFrame>,
    per_frame: Vec<PerFrame>,
    pub in_flight_index: usize,
    pub num_frames_in_flight: usize,
    pub frame_index: usize,
    pub num_frames: usize,
}

#[derive(Debug)]
struct PerInFlightFrame {
    image_available_semaphore: Semaphore,
    fence: Fence,
    timing_query_pool: TimestampQueryPool<2>,
}

#[derive(Debug)]
struct PerFrame {
    render_finished_semaphore: Semaphore,
}

impl InFlightFrames {
    pub(crate) fn new(context: &Context, frame_count: usize, in_flight_count: usize) -> OctaResult<Self> {
        let per_in_flight_frame = (0..in_flight_count)
            .map(|_i| {
                let image_available_semaphore = context.create_semaphore()?;
                let fence = context.create_fence(Some(vk::FenceCreateFlags::SIGNALED))?;
                let timing_query_pool = context.create_timestamp_query_pool()?;

                Ok(PerInFlightFrame {
                    image_available_semaphore,
                    fence,
                    timing_query_pool,
                })
            })
            .collect::<OctaResult<Vec<_>>>()?;

        let per_frame = (0..frame_count)
            .map(|_i| {
                let render_finished_semaphore = context.create_semaphore()?;

                Ok(PerFrame {
                    render_finished_semaphore,
                })
            })
            .collect::<OctaResult<Vec<_>>>()?;

        Ok(Self {
            per_in_flight_frame,
            per_frame,
            in_flight_index: 0,
            num_frames_in_flight: in_flight_count,
            frame_index: 0,
            num_frames: frame_count,
        })
    }

    pub(crate) fn next(&mut self) {
        self.in_flight_index = (self.in_flight_index + 1) % self.per_in_flight_frame.len();
    }

    pub(crate) fn set_frame_index(&mut self, frame_index: usize) {
        self.frame_index = frame_index; 
    }

    pub(crate) fn image_available_semaphore(&self) -> &Semaphore {
        &self.per_in_flight_frame[self.in_flight_index].image_available_semaphore
    }

    pub(crate) fn render_finished_semaphore(&self) -> &Semaphore {
        &self.per_frame[self.frame_index].render_finished_semaphore
    }

    pub(crate) fn fence(&self) -> &Fence {
        &self.per_in_flight_frame[self.in_flight_index].fence
    }

    pub(crate) fn timing_query_pool(&self) -> &TimestampQueryPool<2> {
        &self.per_in_flight_frame[self.in_flight_index].timing_query_pool
    }

    pub(crate) fn gpu_frame_time_ms(&self) -> OctaResult<Duration> {
        let result = self.timing_query_pool().wait_for_all_results()?;
        let time = Duration::from_nanos(result[1].saturating_sub(result[0]));

        Ok(time)
    }
}
