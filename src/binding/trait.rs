use core::fmt;
use std::time::Duration;
use ash::vk::AttachmentLoadOp;
use winit::event::WindowEvent;
use crate::{Engine, OctaResult};

pub trait BindingTrait: fmt::Debug {
    type RenderState: fmt::Debug;
    type LogicState: fmt::Debug;
    fn new_logic_state() -> OctaResult<Self::LogicState>;
    fn new_render_state(logic_state: &mut Self::LogicState, engine: &mut Engine) -> OctaResult<Self::RenderState>;

    fn update(
        logic_state: &mut Self::LogicState,
        render_state: &mut Self::RenderState,
        engine: &mut Engine,
        image_index: usize,
        delta_time: Duration,
    ) -> OctaResult<()>{
        // prevents reports of unused parameters without needing to use #[allow]
        let _ = render_state;
        let _ = logic_state;
        let _ = engine;
        let _ = image_index;
        let _ = delta_time;

        Ok(())
    }

    fn record_render_commands(
        logic_state: &mut Self::LogicState,
        render_state: &mut Self::RenderState,
        engine: &mut Engine,
        image_index: usize,
    ) -> OctaResult<()> {
        // prevents reports of unused parameters without needing to use #[allow]
        let _ = render_state;
        let _ = logic_state;

        // Render empty Screen
        let command_buffer = &engine.command_buffers[image_index];
        let size = engine.swapchain.size;
        let swap_chain_image = &engine.swapchain.images_and_views[image_index].image;
        let swap_chain_view = &engine.swapchain.images_and_views[image_index].view;
        let swap_chain_depth_view = &engine.swapchain.depht_images_and_views[image_index].view;

        command_buffer.begin_rendering(swap_chain_view, swap_chain_depth_view, size, AttachmentLoadOp::CLEAR, None);
        command_buffer.end_rendering();

        command_buffer.swapchain_image_render_barrier(swap_chain_image)?;

        Ok(())
    }

    fn on_window_event(
        logic_state: &mut Self::LogicState,
        render_state: &mut Self::RenderState,
        engine: &mut Engine,
        event: &WindowEvent
    ) -> OctaResult<()> {
        // prevents reports of unused parameters without needing to use #[allow]
        let _ = render_state;
        let _ = logic_state;
        let _ = engine;
        let _ = event;

        Ok(())
    }

    fn on_recreate_swapchain(
        logic_state: &mut Self::LogicState,
        render_state: &mut Self::RenderState,
        engine: &mut Engine,
    ) -> OctaResult<()> {
        // prevents reports of unused parameters without needing to use #[allow]
        let _ = render_state;
        let _ = logic_state;
        let _ = engine;

        Ok(())
    }
}
