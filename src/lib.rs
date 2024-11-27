pub extern crate anyhow;
pub extern crate glam;
pub extern crate log;
pub extern crate egui;
pub extern crate egui_ash_renderer;
pub extern crate egui_winit;
pub extern crate egui_extras;
pub extern crate puffin_egui;

pub mod camera;
pub mod controls;
pub mod gui;
pub mod logger;
pub mod stats;
pub mod vulkan;
pub mod utils;
pub mod hot_reloading;
pub mod binding;

use crate::stats::{FrameStats, StatsDisplayMode};
use ash::vk::{self};
use controls::Controls;
use glam::UVec2;
use std::{thread, time::{Duration, Instant}};
use log::{debug, info};
use vulkan::*;
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};
use winit::event::KeyEvent;
use winit::keyboard::{KeyCode, PhysicalKey};

#[cfg(debug_assertions)]
use puffin_egui::puffin;
#[cfg(debug_assertions)]
use std::mem;

use crate::binding::{get_binding, Binding};
use crate::binding::r#trait::BindingTrait;
use crate::gui::Gui;
use crate::hot_reloading::HotReloadConfig;
use crate::logger::{log_init};

pub type OctaResult<V> = anyhow::Result<V>;

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

    pub hot_reload_config: Option<HotReloadConfig>
}

pub struct Engine {
    pub num_frames_in_flight: usize,
    pub num_frames: usize,

    frame_stats: FrameStats,
    stats_gui: Gui,
    pub window: Window,

    pub controls: Controls,
    
    pub swapchain: Swapchain,
    pub command_pool: CommandPool,
    pub command_buffers: Vec<CommandBuffer>,
    in_flight_frames: InFlightFrames,
    pub context: Context,
}

pub fn run<B: BindingTrait>(engine_config: EngineConfig) -> OctaResult<()> {
    log_init()?;
    
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    
    let mut binding = get_binding::<B>(&engine_config)?;
    
    let mut engine = Engine::new(&event_loop, &engine_config)?;
    let mut render_state = binding.new_render_state(&mut engine)?;
    #[cfg(debug_assertions)]
    let mut dropped_render_states: Vec<(B::RenderState, usize)> = vec![];
    
    let mut logic_state = binding.new_logic_state(&mut engine)?;

    let mut is_swapchain_dirty = false;

    let mut last_frame = Instant::now();
    let mut last_frame_start = Instant::now();

    let fps_as_duration = Duration::from_secs_f64(1.0 / 60.0);

    event_loop.run(move |event, elwt| {

        // Make sure it is dropped before engine so it properly stops
        let logic_state = &mut logic_state; 
        let render_state = &mut render_state;
        #[cfg(debug_assertions)]
        let dropped_render_states = &mut dropped_render_states; 
        
        // Send Event to Controls Struct
        engine.controls.handle_event(&event);
        
        match event {
            Event::NewEvents(_) => {
                let frame_start = Instant::now();
                let frame_time = frame_start - last_frame;
                let compute_time = frame_start - last_frame_start;
                last_frame = frame_start;

                if fps_as_duration > compute_time {
                    thread::sleep(fps_as_duration - compute_time)
                };
                last_frame_start = Instant::now();
                
                engine.frame_stats.set_cpu_time(frame_time, compute_time);

                engine.controls.reset();
            }
            
            Event::WindowEvent { event, .. } => {
                engine.stats_gui.handle_event(&engine.window, &event);
                binding.on_window_event(render_state, logic_state, &mut engine, &event)
                    .expect("Failed in On Window Event");

                match event {
                    // On resize
                    WindowEvent::Resized(..) => {
                        log::debug!("Window has been resized");
                        is_swapchain_dirty = true;
                    }
                    // Keyboard
                    WindowEvent::KeyboardInput {
                        event:
                        KeyEvent {
                            state,
                            physical_key,
                            ..
                        },
                        ..
                    } => {
                        if matches!(physical_key, PhysicalKey::Code(KeyCode::F1))
                            && state == ElementState::Pressed
                        {
                            engine.frame_stats.toggle_stats();
                        }
                    }
                    // Mouse
                    WindowEvent::MouseInput { state, button, .. } => {
                        if button == MouseButton::Right {
                            if state == ElementState::Pressed {
                                engine.window.set_cursor_visible(false);
                            } else {
                                engine.window.set_cursor_visible(true);
                            }
                        }
                    }
                    // Exit app on request to close window
                    WindowEvent::CloseRequested => elwt.exit(),
                    _ => (),
                }
            }
            
            // Draw
            Event::AboutToWait => {
                if is_swapchain_dirty {
                    let dim = engine.window.inner_size();
                    if dim.width > 0 && dim.height > 0 {
                        engine
                            .recreate_swapchain(dim.width, dim.height)
                            .expect("Failed to recreate swapchain");
                        binding.on_recreate_swapchain(render_state, logic_state, &mut engine)
                            .expect("Error on recreate swapchain callback");
                    } else {
                        return;
                    }
                }

                is_swapchain_dirty = engine.draw(
                    &mut binding,
                    render_state,
                    logic_state,
                    #[cfg(debug_assertions)]
                    dropped_render_states
                ).expect("Failed to tick");
            }
            
            // Wait for gpu to finish pending work before closing app
            Event::LoopExiting => {engine
                .wait_for_gpu()
                .expect("Failed to wait for gpu to finish work");
                
                info!("Stopping")
            
            },
            _ => (),
        }
    })?;
    
    Ok(())
}

impl Engine {
    fn new(
        event_loop: &EventLoop<()>,
        engine_config: &EngineConfig
    ) -> OctaResult<Self> {
        info!("Creating Engine");

        let window = WindowBuilder::new()
            .with_title(&engine_config.name)
            .with_inner_size(PhysicalSize::new(engine_config.start_size.x, engine_config.start_size.y))
            .with_resizable(true)
            .build(&event_loop)
            .unwrap();

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

    fn recreate_swapchain(&mut self, width: u32, height: u32) -> OctaResult<()> {
        debug!("Recreating the swapchain");

        self.wait_for_gpu()?;

        // Swapchain and dependent resources
        self.swapchain.resize(&self.context, width, height)?;

        Ok(())
    }

    pub fn wait_for_gpu(&self) -> OctaResult<()> {
        self.context.device_wait_idle()
    }

    fn draw<B:BindingTrait>(
        &mut self, 
        binding: &mut Binding<B>, 
        render_state: &mut B::RenderState, 
        logic_state: &mut B::LogicState,
        #[cfg(debug_assertions)]
        dropped_render_state: &mut Vec<(B::RenderState, usize)>
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

        let next_image_result = self.swapchain.acquire_next_image(
            std::u64::MAX,
            self.in_flight_frames.image_available_semaphore(),
        );
        let image_index = match next_image_result {
            Ok(AcquiredImage { index, .. }) => index as usize,
            Err(err) => match err.downcast_ref::<vk::Result>() {
                Some(&vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok(true),
                _ => panic!("Error while acquiring next image. Cause: {}", err),
            },
        };
        self.in_flight_frames.fence().reset()?;


        #[cfg(debug_assertions)]
        if let Binding::HotReload(b) = binding {
            for i in (0..dropped_render_state.len()).rev() {
                if dropped_render_state[i].1 == image_index {
                    // Dosen't work 
                    dropped_render_state.remove(i);
                }
            }
            
            if b.lib_reloader.can_update() {
                debug!("Hot reload");
                b.active = true;
                
                b.lib_reloader.update()?;

                binding.init_hot_reload()?;
                
                let mut new_render_state = binding.new_render_state(self)?;
                mem::swap(render_state, &mut new_render_state);
                
                dropped_render_state.push((new_render_state, image_index));
                
                debug!("Hot reload done");
            }
        }
        
        {
            #[cfg(debug_assertions)]
            puffin::profile_scope!("update app");
            binding.update(render_state, logic_state, self, image_index, self.frame_stats.frame_time)?;
        }

        self.record_command_buffer(image_index, binding, render_state, logic_state)?;

        self.context.graphics_queue.submit(
            &self.command_buffers[image_index],
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
            image_index as _,
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

    fn record_command_buffer<B: BindingTrait>(&mut self, image_index: usize, binding: &Binding<B>, render_state: &mut B::RenderState, logic_state: &mut B::LogicState) -> OctaResult<()> {
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

pub struct ImageAndView {
    pub view: ImageView,
    pub image: Image,
}

struct InFlightFrames {
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

#[derive(Debug)]
struct Queue<T>(Vec<T>, usize);

impl<T> Queue<T> {
    fn new(max_size: usize) -> Self {
        Self(Vec::with_capacity(max_size), max_size)
    }

    fn push(&mut self, value: T) {
        if self.0.len() == self.1 {
            self.0.remove(0);
        }
        self.0.push(value);
    }
}
