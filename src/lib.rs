#![forbid(unused_must_use)]

pub extern crate anyhow;
pub extern crate glam;
pub extern crate log;
pub extern crate simplelog;
pub extern crate egui;
pub extern crate egui_ash_renderer;
pub extern crate egui_winit;
pub extern crate puffin_egui;
pub extern crate egui_extras;
pub extern crate image;

pub mod camera;
pub mod controls;
pub mod gui;
pub mod logger;
pub mod stats;
pub mod vulkan;
pub mod utils;
pub mod hot_reloading;
pub mod binding;
pub mod engine;
pub mod in_flight_frames;

use anyhow::{bail, Context as _};
use engine::{Engine, EngineConfig};
use std::{env, process, thread, time::{Duration, Instant}};
use log::{debug, error, info, trace, warn};
use vulkan::{entry::Entry, *};
use winit::{
    application::ApplicationHandler, event::{ElementState, MouseButton, WindowEvent}, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, platform::{x11::EventLoopBuilderExtX11}};
use winit::event::KeyEvent;
use winit::keyboard::{KeyCode, PhysicalKey};

#[cfg(debug_assertions)]
use std::mem;

use crate::binding::{get_binding, Binding};
use crate::binding::r#trait::BindingTrait;
use crate::logger::{log_init};


pub type OctaResult<V> = anyhow::Result<V>;

struct GlobalContainer<B: BindingTrait> {
    pub entry: Entry,
    pub engine_config: EngineConfig,
    pub binding: Binding<B>,
    pub logic_state: B::LogicState,

    pub active: Option<ActiveContainer<B>>,
    pub dropped_render_state: Vec<B::RenderState>,
}

#[derive(Debug)]
struct ActiveContainer<B: BindingTrait> {
    pub render_state: B::RenderState,

    pub engine: Engine,
    
    pub is_swapchain_dirty: bool,
    pub last_frame: Instant,
    pub last_frame_start: Instant,

    pub fps_as_duration: Duration,
}

pub fn run<B: BindingTrait>(engine_config: EngineConfig) { 
    unsafe {
        env::set_var("RUST_BACKTRACE", "1");
        //std::env::set_var("WINIT_UNIX_BACKEND", "x11");
    }

    let res = log_init();
    if res.is_err() {
        let err = res.unwrap_err();
        println!("{}", err);
    }

    let res = run_iternal::<B>(engine_config);

    if res.is_err() {
        let err = res.unwrap_err();
        error!("{:#}", err);
        trace!("{}", err.backtrace());
    }
}

fn run_iternal<B: BindingTrait>(engine_config: EngineConfig) -> OctaResult<()> { 
    let mut global_container = GlobalContainer::<B>::new(engine_config)?;
      
    let mut event_loop_builder = EventLoop::builder();
    
    // Fallback to X11 when wayland is not supported by vulkan 
    if cfg!(target_os = "linux") && !global_container.entry.supports_wayland()? {
        warn!("Wayland is not supported by Vulkan. Falling back to X11.");
        
        event_loop_builder.with_x11();
    }

    let event_loop = event_loop_builder.build()?;

    event_loop.set_control_flow(ControlFlow::Poll);

    event_loop.run_app(&mut global_container)?;
    
    #[cfg(debug_assertions)]
    if let Binding::HotReload(b) = global_container.binding {            
        if b.active {
            // Killing process because normal dropping would freeze the window.
            process::exit(0);
        }
    }
     
    Ok(())
}

impl<B: BindingTrait> GlobalContainer<B> {
    fn new(engine_config: EngineConfig) -> OctaResult<Self> {
        let entry = Entry::new();

        if !entry.supports_surface()? {
            bail!("Vulkan does not support Surface Extension")
        }

        let binding = get_binding::<B>(&engine_config)?;
        let logic_state = binding.new_logic_state()?;

        Ok(Self {
            entry,
            engine_config,
            binding,
            logic_state,
            active: None,
            dropped_render_state: vec![],
        })
    }
}

impl<B: BindingTrait> ApplicationHandler for GlobalContainer<B> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let active_container = ActiveContainer::new(
            self.entry.to_owned(),
            event_loop, 
            &self.engine_config, 
            &self.binding,
            &mut self.logic_state); 
         
        if active_container.is_err() {
            let err = active_container.unwrap_err()
                .context("resumed");

            error!("{:#}", err);
            trace!("{}", err.backtrace());
            event_loop.exit();
        } else {
            self.active = Some(active_container.unwrap()); 
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        Self::handle_err(&mut self.active, |x| { 
            x.new_events() 
        }, "new_events", event_loop); 
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        Self::handle_err(&mut self.active, |x| { 
            x.window_event(event_loop, event, &self.binding, &mut self.logic_state) 
        }, "window_event", event_loop); 
    }

    fn device_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _device_id: winit::event::DeviceId,
            event: winit::event::DeviceEvent,
        ) {
        Self::handle_err(&mut self.active, |x| { 
            x.device_event(event) 
        }, "device_event", event_loop); 
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        Self::handle_err(&mut self.active, |x| { 
            x.about_to_wait(event_loop, &mut self.logic_state, &mut self.binding, &mut self.dropped_render_state) 
        }, "about_to_wait", event_loop);  
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        Self::handle_err(&mut self.active, |x| { 
            x.exiting() 
        }, "exiting", event_loop); 
    }
}

impl<B: BindingTrait> GlobalContainer<B> {
    fn handle_err<F: FnOnce(&mut ActiveContainer<B>) -> OctaResult<()>>(active: &mut Option<ActiveContainer<B>>,  func: F, context: &str, event_loop: &ActiveEventLoop) {
        match active {
            Some(active) => {
                let res = func(active);

                if res.is_err() {
                    let err = res.unwrap_err()
                        .context(context.to_string());

                    error!("{:#}", err);
                    trace!("{}", err.backtrace());
                    event_loop.exit();
                }
            },
            None => {
                trace!("Cant run {context} -> no active Container!");
            },
        }
    }
}

impl<B: BindingTrait> ActiveContainer<B> {
    fn new(
        entry: Entry,
        event_loop: &ActiveEventLoop,
        engine_config: &EngineConfig,
        binding: &Binding<B>,
        logic_state: &mut B::LogicState,
    ) -> OctaResult<Self> {

        let mut engine = Engine::new(entry, &event_loop, &engine_config)?;
        let render_state = binding.new_render_state(logic_state, &mut engine)?;
        
        let is_swapchain_dirty = false;

        let last_frame = Instant::now();
        let last_frame_start = Instant::now();

        let fps_as_duration = Duration::from_secs_f64(1.0 / 60.0);
         
        Ok(Self {
            render_state,
            engine,
            is_swapchain_dirty,
            last_frame,
            last_frame_start,
            fps_as_duration,
        })
    }

    fn new_events(&mut self) -> OctaResult<()> {
        let frame_start = Instant::now();
        let frame_time = frame_start - self.last_frame;
        let compute_time = frame_start - self.last_frame_start;
        self.last_frame = frame_start;

        if self.fps_as_duration > compute_time {
            thread::sleep(self.fps_as_duration - compute_time)
        };
        self.last_frame_start = Instant::now();
        
        self.engine.frame_stats.set_cpu_time(frame_time, compute_time);

        self.engine.controls.reset();

        Ok(())
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: WindowEvent,
        binding: &Binding<B>,
        logic_state: &mut B::LogicState,
    ) -> OctaResult<()> {
        
        // Send Event to Controls Struct
        self.engine.controls.window_event(&event);

        self.engine.stats_gui.handle_event(&self.engine.window, &event);
        binding.on_window_event(&mut self.render_state, logic_state, &mut self.engine, &event)
            .context("Failed in On Window Event")?;
        
        match event { 
            // On resize
            WindowEvent::Resized(..) => {
                debug!("Window has been resized");
                self.is_swapchain_dirty = true;
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
                    self.engine.frame_stats.toggle_stats();
                }
            }
            // Mouse
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Right {
                    if state == ElementState::Pressed {
                        self.engine.window.set_cursor_visible(false);
                    } else {
                        self.engine.window.set_cursor_visible(true);
                    }
                }
            }
            // Exit app on request to close window
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => (),
        }

        Ok(())
    }

    fn device_event(
            &mut self,
            event: winit::event::DeviceEvent,
        ) -> OctaResult<()> {

        self.engine.controls.device_event(&event);
        
        Ok(())
    }

    fn about_to_wait(
        &mut self, 
        _event_loop: &ActiveEventLoop, 
        logic_state: &mut B::LogicState, 
        binding: &mut Binding<B>, 
        dropped_render_state: &mut Vec<B::RenderState>,
    ) -> OctaResult<()> {

        #[cfg(debug_assertions)]
        if let Binding::HotReload(b) = binding {            
            if b.lib_reloader.can_update() {
                b.active = true;
                info!("Init Hot Reload");

                b.lib_reloader.update()?;
                binding.init_hot_reload()?;
                let mut render_state = binding.new_render_state(logic_state, &mut self.engine)?;
                mem::swap(&mut render_state, &mut self.render_state);

                // Droppeing memory created in lib will frezze the game so we just keep it.
                dropped_render_state.push(render_state);

                info!("Hot Reload done");
            }
        }

        if self.is_swapchain_dirty {
            let dim = self.engine.window.inner_size();
            if dim.width > 0 && dim.height > 0 {
                self.engine
                    .recreate_swapchain(dim.width, dim.height)
                    .context("Failed to recreate swapchain")?;
                binding.on_recreate_swapchain(&mut self.render_state, logic_state, &mut self.engine)
                    .context("Error on recreate swapchain callback")?;
            } else {
                return Ok(());
            }
        }

        self.is_swapchain_dirty = self.engine.draw(
            binding,
            &mut self.render_state,
            logic_state,
        ).context("Failed to tick")?;

        Ok(())
    }
    
    fn exiting(&mut self) -> OctaResult<()> {
        self.engine
            .wait_for_gpu()
            .context("Failed to wait for gpu to finish work")?;
                
        info!("Stopping");

        Ok(())
    }
}

