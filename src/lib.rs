pub extern crate anyhow;
pub extern crate glam;
pub extern crate log;
pub extern crate egui;
pub extern crate egui_ash_renderer;
pub extern crate egui_winit;
pub extern crate egui_extras;

pub mod camera;
pub mod controls;
pub mod gui;
pub mod logger;
mod stats;
pub mod vulkan;


use crate::stats::{FrameStats, StatsDisplayMode};
use anyhow::Result;
use ash::vk::{self};
use controls::Controls;
use glam::UVec2;
use logger::log_init;
use std::{
    marker::PhantomData,
    thread,
    time::{Duration, Instant},
};
use vulkan::*;
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};
use winit::event::KeyEvent;
use winit::keyboard::{KeyCode, PhysicalKey};
use crate::gui::Gui;

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
    pub validation_layers: EngineFeatureValue,
    pub shader_debug_printing: EngineFeatureValue,
}


pub struct BaseApp<B: App> {
    phantom: PhantomData<B>,
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

pub trait App: Sized {
    fn new(base: &mut BaseApp<Self>) -> Result<Self>;
    
    fn update(
        &mut self,
        base: &mut BaseApp<Self>,
        image_index: usize,
        delta_time: Duration,
    ) -> Result<()>;

    fn record_render_commands(
        &mut self,
        base: &mut BaseApp<Self>,
        image_index: usize,
    ) -> Result<()> {
        // prevents reports of unused parameters without needing to use #[allow]
        let _ = base;
        let _ = image_index;

        Ok(())
    }
    
    fn on_window_event(&mut self, base: &mut BaseApp<Self>, event: &WindowEvent) -> Result<()> {
        // prevents reports of unused parameters without needing to use #[allow]
        let _ = base;
        let _ = event;

        Ok(())
    }

    fn on_recreate_swapchain(&mut self, base: &mut BaseApp<Self>) -> Result<()> {
        // prevents reports of unused parameters without needing to use #[allow]
        let _ = base;

        Ok(())
    }
}

pub fn run<A: App + 'static>(engine_config: EngineConfig) -> Result<()> {
    // Setup cfg aliases




    log_init("app_log.log");

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    
    let mut base_app = BaseApp::new(&event_loop, &engine_config)?;

    let mut app = A::new(&mut base_app)?;

    let mut is_swapchain_dirty = false;

    let mut last_frame = Instant::now();
    let mut last_frame_start = Instant::now();

    let fps_as_duration = Duration::from_secs_f64(1.0 / 60.0);

    event_loop.run(move |event, elwt| {
        
        let app = &mut app; // Make sure it is dropped before base_app
        
        // Send Event to Controls Struct
        base_app.controls.handle_event(&event);
        
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
                
                base_app
                    .frame_stats
                    .set_frame_time(frame_time, compute_time);

                base_app.controls.reset();
            }
            
            Event::WindowEvent { event, .. } => {
                base_app.stats_gui.handle_event(&base_app.window, &event);
                app.on_window_event(&mut base_app, &event)
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
                            base_app.frame_stats.toggle_stats();
                        }
                    }
                    // Mouse
                    WindowEvent::MouseInput { state, button, .. } => {
                        if button == MouseButton::Right {
                            if state == ElementState::Pressed {
                                base_app.window.set_cursor_visible(false);
                            } else {
                                base_app.window.set_cursor_visible(true);
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
                    let dim = base_app.window.inner_size();
                    if dim.width > 0 && dim.height > 0 {
                        base_app
                            .recreate_swapchain(dim.width, dim.height)
                            .expect("Failed to recreate swapchain");
                        app.on_recreate_swapchain(&mut base_app)
                            .expect("Error on recreate swapchain callback");
                    } else {
                        return;
                    }
                }

                is_swapchain_dirty = base_app.draw(app).expect("Failed to tick");
            }
            
            // Wait for gpu to finish pending work before closing app
            Event::LoopExiting => base_app
                .wait_for_gpu()
                .expect("Failed to wait for gpu to finish work"),
            _ => (),
        }
    })?;
    
    Ok(())
}

impl<B: App> BaseApp<B> {
    fn new(
        event_loop: &EventLoop<()>,
        engine_config: &EngineConfig
    ) -> Result<Self> {
        log::info!("Creating Engine");

        let window = WindowBuilder::new()
            .with_title(&engine_config.name)
            .with_inner_size(PhysicalSize::new(engine_config.start_size.x, engine_config.start_size.y))
            .with_resizable(true)
            .build(&event_loop)
            .unwrap();

        // Vulkan context
        let context = Context::new(&window, &window, engine_config)?;

        let command_pool = context.create_command_pool(
            context.graphics_queue_family,
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
        
        let frame_stats = FrameStats::default();
        let stats_gui = Gui::new(&context, swapchain.format, swapchain.depth_format,  &window, num_frames)?;
        

        Ok(Self {
            phantom: PhantomData,
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
            stats_gui
        })
    }

    fn recreate_swapchain(&mut self, width: u32, height: u32) -> Result<()> {
        log::debug!("Recreating the swapchain");

        self.wait_for_gpu()?;

        // Swapchain and dependent resources
        self.swapchain.resize(&self.context, width, height)?;

        Ok(())
    }

    pub fn wait_for_gpu(&self) -> Result<()> {
        self.context.device_wait_idle()
    }

    fn draw(&mut self, base_app: &mut B) -> Result<bool> {
        // Drawing the frame
        self.in_flight_frames.next();
        self.in_flight_frames.fence().wait(None)?;

        // Can't get for gpu time on the first frames or vkGetQueryPoolResults gets stuck
        // due to VK_QUERY_RESULT_WAIT_BIT
        let gpu_time = (self.frame_stats.total_frame_count >= self.num_frames_in_flight)
            .then(|| self.in_flight_frames.gpu_frame_time_ms())
            .transpose()?
            .unwrap_or_default();
        self.frame_stats.set_gpu_time_time(gpu_time);
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
        
        base_app.update(self, image_index, self.frame_stats.frame_time)?;

        self.record_command_buffer(image_index, base_app)?;

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

    fn record_command_buffer(&mut self, image_index: usize, base_app: &mut B) -> Result<()> {
        let buffer = &self.command_buffers[image_index];
        buffer.reset()?;
        buffer.begin(None)?;
        buffer.reset_all_timestamp_queries_from_pool(self.in_flight_frames.timing_query_pool());
        buffer.write_timestamp(
            vk::PipelineStageFlags2::NONE,
            self.in_flight_frames.timing_query_pool(),
            0,
        );

        base_app.record_render_commands(self, image_index)?;


        let buffer = &self.command_buffers[image_index];

        if self.frame_stats.stats_display_mode != StatsDisplayMode::None {
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

fn create_command_buffers(pool: &CommandPool, swapchain: &Swapchain) -> Result<Vec<CommandBuffer>> {
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
    fn new(context: &Context, frame_count: usize) -> Result<Self> {
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
            .collect::<Result<Vec<_>>>()?;

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

    fn gpu_frame_time_ms(&self) -> Result<Duration> {
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
