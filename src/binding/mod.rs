pub mod r#trait;

use std::marker::PhantomData;
use std::time::Duration;

use winit::event::WindowEvent;

#[cfg(debug_assertions)]
use libloading::Symbol;
#[cfg(debug_assertions)]
use log::Log;


use crate::{Engine, EngineConfig, OctaResult};
use crate::binding::r#trait::BindingTrait;
use crate::hot_reloading::HotReloadController;

pub enum Binding<B: BindingTrait> {
    Static(PhantomData<B>),
    HotReload(HotReloadController)
}

pub fn get_binding<B: BindingTrait>(_engine_config: &EngineConfig) -> OctaResult<Binding<B>> {
    #[cfg(not(debug_assertions))]
    return Ok(Binding::Static(PhantomData::default()));

    #[cfg(debug_assertions)]
    Ok(if let Some(config) = &_engine_config.hot_reload_config {
        Binding::HotReload(HotReloadController::new(config.to_owned())?)
    } else {
        Binding::Static(PhantomData::default())
    })
}

impl<B: BindingTrait> Binding<B> {

    #[cfg(debug_assertions)]
    pub fn init_hot_reload(&self) -> OctaResult<()> {
        match self {
            Binding::HotReload(b) => {
                if b.active {
                    unsafe {
                        let call: Symbol<unsafe extern fn(&'static dyn Log) -> OctaResult<()>> =
                            b.lib_reloader.get_symbol("init_hot_reload")?;
                        return call(log::logger())
                    }
                }
            }
            _ => {}
        }
        
        Ok(())
    }
    
    pub fn new_render_state(&self, logic_state: &mut B::LogicState, engine: &mut Engine) -> OctaResult<B::RenderState> {
        #[cfg(not(debug_assertions))]
        return B::new_render_state(engine);

        #[cfg(debug_assertions)]
        match self {
            Binding::HotReload(b) => {
                if b.active {
                    unsafe {
                        let call: Symbol<unsafe extern fn(&mut B::LogicState, &mut Engine) -> OctaResult<B::RenderState>> =
                            b.lib_reloader.get_symbol("new_render_state")?;
                        call(logic_state, engine)
                    }
                } else {
                    B::new_render_state(logic_state, engine)
                }
            }
            Binding::Static(_) => {
                B::new_render_state(logic_state, engine)
            }
        }
    }

    pub fn new_logic_state(&self) -> OctaResult<B::LogicState> {
        #[cfg(not(debug_assertions))]
        return B::new_logic_state(render_state, engine);

        #[cfg(debug_assertions)]
        match self {
            Binding::HotReload(b) => {
                if b.active {
                    unsafe {
                        let call: Symbol<unsafe extern fn() -> OctaResult<B::LogicState>> =
                            b.lib_reloader.get_symbol("new_logic_state")?;
                        call()
                    }
                } else {
                    B::new_logic_state()
                }
            }
            Binding::Static(_) => {
                B::new_logic_state()
            }
        }
    }

    pub fn update(
        &self, 
        render_state: &mut B::RenderState,
        logic_state: &mut B::LogicState,
        engine: &mut Engine, 
        image_index: usize, 
        delta_time: Duration
    ) -> OctaResult<()> {
        #[cfg(not(debug_assertions))]
        return B::update(render_state, logic_state, engine, image_index, delta_time);

        #[cfg(debug_assertions)]
        match self {
            Binding::HotReload(b) => {
                if b.active {
                    unsafe {
                        let call: Symbol<unsafe extern fn(&mut B::LogicState, &mut B::RenderState, &mut Engine, usize, Duration) -> OctaResult<()>> =
                            b.lib_reloader.get_symbol("update")?;
                        call(logic_state, render_state, engine, image_index, delta_time)
                    }
                } else {
                    B::update(logic_state, render_state,  engine, image_index, delta_time)
                }
            }
            Binding::Static(_) => {
                B::update(logic_state, render_state,  engine, image_index, delta_time)
            }
        }
    }

    pub fn record_render_commands(
        &self,
        render_state: &mut B::RenderState,
        logic_state: &mut B::LogicState,
        engine: &mut Engine, 
        image_index: usize
    ) -> OctaResult<()> {
        #[cfg(not(debug_assertions))]
        return B::record_render_commands(render_state, logic_state, engine, image_index);

        #[cfg(debug_assertions)]
        match self {
            Binding::HotReload(b) => {
                if b.active {
                    unsafe {
                        let call: Symbol<unsafe extern fn(&mut B::LogicState, &mut B::RenderState, &mut Engine, usize) -> OctaResult<()>> =
                            b.lib_reloader.get_symbol("record_render_commands")?;
                        call(logic_state, render_state, engine, image_index)
                    }
                } else {
                    B::record_render_commands(logic_state, render_state, engine, image_index)
                }
            }
            Binding::Static(_) => {
                B::record_render_commands(logic_state, render_state, engine, image_index)
            }
        }
    }

    pub fn on_window_event(
        &self,
        render_state: &mut B::RenderState,
        logic_state: &mut B::LogicState,
        engine: &mut Engine, 
        event: &WindowEvent
    ) -> OctaResult<()> {
        #[cfg(not(debug_assertions))]
        return B::on_window_event(render_state, logic_state, engine, event);

        #[cfg(debug_assertions)]
        match self {
            Binding::HotReload(b) => {
                if b.active {
                    unsafe {
                        let call: Symbol<unsafe extern fn(&mut B::LogicState, &mut B::RenderState, &mut Engine, &WindowEvent) -> OctaResult<()>> =
                            b.lib_reloader.get_symbol("on_window_event")?;
                        call(logic_state, render_state, engine, event)
                    }
                } else {
                    B::on_window_event(logic_state, render_state, engine, event)
                }
            }
            Binding::Static(_) => {
                B::on_window_event(logic_state, render_state, engine, event)
            }
        }
    }

    pub fn on_recreate_swapchain(
        &self,
        render_state: &mut B::RenderState,
        logic_state: &mut B::LogicState,
        engine: &mut Engine
    ) -> OctaResult<()> {
        #[cfg(not(debug_assertions))]
        return B::on_recreate_swapchain(render_state, logic_state, engine);

        #[cfg(debug_assertions)]
        match self {
            Binding::HotReload(b) => {
                if b.active {
                    unsafe {
                        let call: Symbol<unsafe extern fn(&mut B::LogicState, &mut B::RenderState, &mut Engine) -> OctaResult<()>> =
                            b.lib_reloader.get_symbol("on_recreate_swapchain")?;
                        call( logic_state, render_state, engine)
                    }
                } else {
                    B::on_recreate_swapchain(logic_state, render_state, engine)
                }
            }
            Binding::Static(_) => {
                B::on_recreate_swapchain(logic_state, render_state, engine)
            }
        }
    }
}
