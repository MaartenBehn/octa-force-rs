use anyhow::Result;
use egui::{
    Context as EguiContext, FullOutput, TextureId,
    ViewportId,
};
use egui_ash_renderer::{DynamicRendering, Options, Renderer};
use egui_winit::State as EguiWinit;
use egui_winit::winit::window::Window;
use crate::vulkan::{ash::vk, CommandBuffer, Context as VkContext, Context};
use winit::{event::WindowEvent};

pub struct Gui {
    pub egui: EguiContext,
    pub egui_winit: EguiWinit,
    pub renderer: Renderer,
    gui_textures_to_free: Vec<Vec<TextureId>>,
}

impl Gui {
    pub fn new(
        context: &VkContext,
        format: vk::Format,
        window: &Window,
        in_flight_frames: usize,
    ) -> Result<Self> {
        let egui = EguiContext::default();
        let platform = EguiWinit::new(egui.clone(), ViewportId::ROOT, &window, None, None);

        let gui_renderer = Renderer::with_gpu_allocator(
            context.allocator.clone(),
            context.device.inner.clone(),
            DynamicRendering {
                color_attachment_format: format,
                depth_attachment_format: None,
            },
            Options {
                in_flight_frames,
                ..Default::default()
            },
        )?;

        Ok(Self {
            egui,
            egui_winit: platform,
            renderer: gui_renderer,
            gui_textures_to_free: vec![Vec::new(); in_flight_frames]
        })
    }

    pub fn handle_event(&mut self, window: &Window, event: &WindowEvent) {
        let _ = self.egui_winit.on_window_event(window, event);
    }

    pub fn cmd_draw<F: FnOnce(&egui::Context)>(
        &mut self,
        command_buffer: &CommandBuffer,
        extent: vk::Extent2D,
        image_index: usize,
        window: &Window,
        context: &Context,
        build: F,
    ) -> Result<()> {
        if !self.gui_textures_to_free[image_index].is_empty() {
            self.renderer.free_textures(&self.gui_textures_to_free[image_index])?;
        }

        let raw_input =  self.egui_winit.take_egui_input(window);

        let FullOutput {
            platform_output,
            textures_delta,
            shapes,
            pixels_per_point,
            ..
        } = self.egui.run(raw_input, build);

        self.egui_winit.handle_platform_output(window, platform_output);

        if !textures_delta.free.is_empty() {
            self.gui_textures_to_free[image_index] = textures_delta.free;
        }

        if !textures_delta.set.is_empty() {
            self.renderer
                .set_textures(context.graphics_queue.inner, context.command_pool.inner, textures_delta.set.as_slice())
                .expect("Failed to update texture");
        }

        let primitives = self.egui.tessellate(shapes, pixels_per_point);
        
        self.renderer.cmd_draw(command_buffer.inner, extent, pixels_per_point, &primitives)?;

        Ok(())
    }
}
