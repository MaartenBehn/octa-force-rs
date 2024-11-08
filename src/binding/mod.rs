pub mod r#trait;

use std::marker::PhantomData;
use std::time::Duration;
use anyhow::bail;
use libloading::Symbol;
use winit::event::WindowEvent;
use crate::{Engine, OctaResult};
use crate::binding::r#trait::BindingTrait;
use crate::hot_reloading::lib_reloader::LibReloader;

pub enum Binding<B: BindingTrait> {
    HotReload(HotReloadBinding),
    Static(PhantomData<B>)
}

pub struct HotReloadBinding {
    pub(crate) lib_reloader: LibReloader
}

impl HotReloadBinding {
    pub fn new(lib_dir: String, lib_name: String) -> OctaResult<Self> {
        let lib_reloader = LibReloader::new(lib_dir, lib_name, None, None)?;

        Ok(HotReloadBinding{
            lib_reloader,
        })
    }
}

pub fn get_used_binding<B: BindingTrait>(bindings: Vec<Binding<B>>) -> OctaResult<Binding<B>> {
    #[cfg(not(debug_assertions))]
    {
        let _ = bindings; // Remove unused warning
        return Ok(Binding::Static(PhantomData::default()));
    }
    
    let mut hot_reload = None;
    let mut static_binding = None;
    for binding in bindings {
        match binding {
            Binding::HotReload(_) => {
                hot_reload = Some(binding);
            }
            Binding::Static(_) => {
                static_binding = Some(binding);
            }
        }
    }

    if hot_reload.is_some(){
        Ok(hot_reload.unwrap())
    } else if static_binding.is_some() {
        Ok(static_binding.unwrap())
    } else{
        bail!("No Binding!")
    }
}

impl<B: BindingTrait> Binding<B> {
    pub fn new_render_state(&self, engine: &mut Engine) -> OctaResult<B::RenderState> {
        #[cfg(not(debug_assertions))]
        return B::new_render_state(engine);
        
        match self {
            Binding::HotReload(b) => {
                unsafe {
                    let call: Symbol<unsafe extern fn(&mut Engine) -> OctaResult<B::RenderState>> = 
                        b.lib_reloader.get_symbol("new_render_state")?;
                    call(engine)
                }
            }
            Binding::Static(_) => {
                B::new_render_state(engine)
            }
        }
    }

    pub fn new_logic_state(&self, engine: &mut Engine) -> OctaResult<B::LogicState> {
        #[cfg(not(debug_assertions))]
        return B::new_logic_state(engine);

        match self {
            Binding::HotReload(b) => {
                unsafe {
                    let call: Symbol<unsafe extern fn(&mut Engine) -> OctaResult<B::LogicState>> =
                        b.lib_reloader.get_symbol("new_logic_state")?;
                    call(engine)
                }
            }
            Binding::Static(_) => {
                B::new_logic_state(engine)
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
        return B::update(engine);
        
        match self {
            Binding::HotReload(b) => {
                unsafe {
                    let call: Symbol<unsafe extern fn(&mut B::RenderState, &mut B::LogicState, &mut Engine, usize, Duration) -> OctaResult<()>> =
                        b.lib_reloader.get_symbol("update")?;
                    call(render_state, logic_state, engine, image_index, delta_time)
                }
            }
            Binding::Static(_) => {
                B::update(render_state, logic_state, engine, image_index, delta_time)
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
        return B::record_render_commands(engine);
        
        match self {
            Binding::HotReload(b) => {
                unsafe {
                    let call: Symbol<unsafe extern fn(&mut B::RenderState, &mut B::LogicState, &mut Engine, usize) -> OctaResult<()>> =
                        b.lib_reloader.get_symbol("record_render_commands")?;
                    call(render_state, logic_state, engine, image_index)
                }
            }
            Binding::Static(_) => {
                B::record_render_commands(render_state, logic_state, engine, image_index)
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
        return B::on_window_event(engine);
        
        match self {
            Binding::HotReload(b) => {
                unsafe {
                    let call: Symbol<unsafe extern fn(&mut B::RenderState, &mut B::LogicState, &mut Engine, &WindowEvent) -> OctaResult<()>> =
                        b.lib_reloader.get_symbol("on_window_event")?;
                    call(render_state, logic_state, engine, event)
                }
            }
            Binding::Static(_) => {
                B::on_window_event(render_state, logic_state, engine, event)
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
        return B::on_recreate_swapchain(engine);
        
        match self {
            Binding::HotReload(b) => {
                unsafe {
                    let call: Symbol<unsafe extern fn(&mut B::RenderState, &mut B::LogicState, &mut Engine) -> OctaResult<()>> =
                        b.lib_reloader.get_symbol("on_recreate_swapchain")?;
                    call(render_state, logic_state, engine)
                }
            }
            Binding::Static(_) => {
                B::on_recreate_swapchain(render_state, logic_state, engine)
            }
        }
    }
}
