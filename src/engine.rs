use glam::UVec2;
use log::{debug, info};
use winit::window::Window;

use crate::{AcquiredImage, Fence, OctaResult, Semaphore, SemaphoreSubmitInfo, TimestampQueryPool};
use crate::{controls::Controls, gui::Gui, hot_reloading::HotReloadConfig, stats::FrameStats, CommandBuffer, CommandPool, Context, Swapchain};

use crate::stats::{StatsDisplayMode};
use ash::vk::{self};
use winit::{
    dpi::PhysicalSize, event_loop::ActiveEventLoop, window::{WindowAttributes}
};

#[cfg(debug_assertions)]
use puffin_egui::puffin;
#[cfg(debug_assertions)]
use std::time::Duration;

use crate::binding::Binding;
use crate::binding::r#trait::BindingTrait;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub enum EngineFeatureValue {
    NotUsed,
    Wanted,
    Needed,
}

#[derive(Clone, Debug)]
pub struct EngineConfig {
    pub name: String,
    pub start_size: UVec2,

    pub ray_tracing: EngineFeatureValue,
    pub compute_rendering: EngineFeatureValue,
    pub validation_layers: EngineFeatureValue,
    pub shader_debug_printing: EngineFeatureValue,
    pub shader_debug_clock: EngineFeatureValue,
    pub gl_ext_scalar_block_layout: EngineFeatureValue,

    pub hot_reload_config: Option<HotReloadConfig>
}

pub struct Engine {
    pub num_frames_in_flight: usize,
    pub num_frames: usize,

    pub(crate) frame_stats: FrameStats,
    pub(crate) stats_gui: Gui,
    pub window: Window,

    pub controls: Controls,
    
    pub swapchain: Swapchain,
    pub command_pool: CommandPool,
    pub command_buffers: Vec<CommandBuffer>,
    pub(crate) in_flight_frames: InFlightFrames,
    pub context: Context,
}

impl Engine {
    pub fn new(
        event_loop: &ActiveEventLoop,
        engine_config: &EngineConfig
    ) -> OctaResult<Self> {
        info!("Creating Engine");

        let window = event_loop.create_window(WindowAttributes::default()
            .with_title(&engine_config.name)
            .with_inner_size(PhysicalSize::new(engine_config.start_size.x, engine_config.start_size.y))
            .with_resizable(true))?;

        // Vulkan context
        let context = Context::new(&window, &window, engine_config)?;

        let command_pool = context.create_command_pool(
            context.physical_device.graphics_queue_family,
            Some(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
        )?;

        let swapchain = Swapchain::new(
            &context,
            window.inner_size().width,
            window.inner_size().height,
        )?;
        let num_frames = swapchain.images_and_views.len();
        let num_frames_in_flight = 2;

        let command_buffers = create_command_buffers(&command_pool, &swapchain)?;

        let in_flight_frames = InFlightFrames::new(&context, num_frames_in_flight)?;

        let controls = Controls::default();
        
        let frame_stats = FrameStats::new();
        let stats_gui = Gui::new(&context, swapchain.format, swapchain.depth_format,  &window, num_frames)?;
        

        Ok(Self {
            num_frames_in_flight,
            num_frames,
            window,
            context,
            command_pool,
            swapchain,
            command_buffers,
            in_flight_frames,
            controls,
            frame_stats,
            stats_gui,
        })
    }

    pub fn recreate_swapchain(&mut self, width: u32, height: u32) -> OctaResult<()> {
        debug!("Recreating the swapchain");

        self.wait_for_gpu()?;

        // Swapchain and dependent resources
        self.swapchain.resize(&self.context, width, height)?;

        Ok(())
    }

    pub fn wait_for_gpu(&self) -> OctaResult<()> {
        self.context.device_wait_idle()
    }

    pub fn draw<B:BindingTrait>(
        &mut self, 
        binding: &mut Binding<B>, 
        render_state: &mut B::RenderState, 
        logic_state: &mut B::LogicState,
    ) -> OctaResult<bool> {
        #[cfg(debug_assertions)]
        puffin::profile_function!();

        // Drawing the frame
        self.in_flight_frames.next();
        self.in_flight_frames.fence().wait(None)?;

        // Can't get for gpu time on the first frames or vkGetQueryPoolResults gets stuck
        // due to VK_QUERY_RESULT_WAIT_BIT
        let gpu_time = (self.frame_stats.total_frame_count >= self.num_frames_in_flight)
            .then(|| self.in_flight_frames.gpu_frame_time_ms())
            .transpose()?
            .unwrap_or_default();
        self.frame_stats.set_gpu_time(gpu_time);
        self.frame_stats.tick();

        let next_frame_result = self.swapchain.acquire_next_image(
            std::u64::MAX,
            self.in_flight_frames.image_available_semaphore(),
        );
        let frame_index = match next_frame_result {
            Ok(AcquiredImage { index, .. }) => index as usize,
            Err(err) => match err.downcast_ref::<vk::Result>() {
                Some(&vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok(true),
                _ => panic!("Error while acquiring next image. Cause: {}", err),
            },
        };
        self.in_flight_frames.fence().reset()?;
        
        // debug!("Frame Index: {frame_index}", );
        
        {
            #[cfg(debug_assertions)]
            puffin::profile_scope!("update app");
            binding.update(render_state, logic_state, self, frame_index, self.frame_stats.frame_time)?;
        }

        self.record_command_buffer(frame_index, binding, render_state, logic_state)?;

        self.context.graphics_queue.submit(
            &self.command_buffers[frame_index],
            Some(SemaphoreSubmitInfo {
                semaphore: self.in_flight_frames.image_available_semaphore(),
                stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            }),
            Some(SemaphoreSubmitInfo {
                semaphore: self.in_flight_frames.render_finished_semaphore(),
                stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
            }),
            self.in_flight_frames.fence(),
        )?;

        let signal_semaphores = [self.in_flight_frames.render_finished_semaphore()];
        let present_result = self.swapchain.queue_present(
            frame_index as _,
            &signal_semaphores,
            &self.context.present_queue,
        );
        match present_result {
            Ok(true) => return Ok(true),
            Err(err) => match err.downcast_ref::<vk::Result>() {
                Some(&vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok(true),
                _ => panic!("Failed to present queue. Cause: {}", err),
            },
            _ => {}
        }

        Ok(false)
    }

    pub fn record_command_buffer<B: BindingTrait>(&mut self, image_index: usize, binding: &Binding<B>, render_state: &mut B::RenderState, logic_state: &mut B::LogicState) -> OctaResult<()> {
        #[cfg(debug_assertions)]
        puffin::profile_function!();

        let buffer = &self.command_buffers[image_index];
        buffer.reset()?;
        buffer.begin(None)?;
        buffer.reset_all_timestamp_queries_from_pool(self.in_flight_frames.timing_query_pool());
        buffer.write_timestamp(
            vk::PipelineStageFlags2::NONE,
            self.in_flight_frames.timing_query_pool(),
            0,
        );

        {
            #[cfg(debug_assertions)]
            puffin::profile_scope!("render app");

            binding.record_render_commands(render_state, logic_state, self, image_index)?;
        }

        let buffer = &self.command_buffers[image_index];

        if self.frame_stats.stats_display_mode != StatsDisplayMode::None {
            #[cfg(debug_assertions)]
            puffin::profile_scope!("render stats");

            buffer.begin_rendering(
                &self.swapchain.images_and_views[image_index].view,
                &self.swapchain.depht_images_and_views[image_index].view,
                self.swapchain.size,
                vk::AttachmentLoadOp::DONT_CARE,
                None,
            );

            self.stats_gui.cmd_draw(
                buffer, 
                self.swapchain.size,
                image_index,
                &self.window, 
                &self.context,
                |ctx| {
                    self.frame_stats.build_perf_ui(ctx);
                }
            )?;
            
            buffer.end_rendering();
        }

        buffer.swapchain_image_present_barrier(&self.swapchain.images_and_views[image_index].image)?;
        buffer.write_timestamp(
            vk::PipelineStageFlags2::ALL_COMMANDS,
            self.in_flight_frames.timing_query_pool(),
            1,
        );
        buffer.end()?;

        Ok(())
    }
}

fn create_command_buffers(pool: &CommandPool, swapchain: &Swapchain) -> OctaResult<Vec<CommandBuffer>> {
    pool.allocate_command_buffers(vk::CommandBufferLevel::PRIMARY, swapchain.images_and_views.len() as _)
}



pub(crate) struct InFlightFrames {
    per_frames: Vec<PerFrame>,
    current_frame: usize,
}

struct PerFrame {
    image_available_semaphore: Semaphore,
    render_finished_semaphore: Semaphore,
    fence: Fence,
    timing_query_pool: TimestampQueryPool<2>,
}

impl InFlightFrames {
    fn new(context: &Context, frame_count: usize) -> OctaResult<Self> {
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
            current_frame: 0,
        })
    }

    fn next(&mut self) {
        self.current_frame = (self.current_frame + 1) % self.per_frames.len();
    }

    fn image_available_semaphore(&self) -> &Semaphore {
        &self.per_frames[self.current_frame].image_available_semaphore
    }

    fn render_finished_semaphore(&self) -> &Semaphore {
        &self.per_frames[self.current_frame].render_finished_semaphore
    }

    fn fence(&self) -> &Fence {
        &self.per_frames[self.current_frame].fence
    }

    fn timing_query_pool(&self) -> &TimestampQueryPool<2> {
        &self.per_frames[self.current_frame].timing_query_pool
    }

    fn gpu_frame_time_ms(&self) -> OctaResult<Duration> {
        let result = self.timing_query_pool().wait_for_all_results()?;
        let time = Duration::from_nanos(result[1].saturating_sub(result[0]));

        Ok(time)
    }
}



