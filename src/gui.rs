use crate::camera::Camera;
use crate::vulkan;
use crate::vulkan::{CommandBuffer, CommandPool};
use anyhow::Result;
use ash::vk;
use ash::vk::{Extent2D, Format};
use glam::{vec3, Mat4};
use imgui::{
    BackendFlags, Condition, ConfigFlags, FontConfig, FontSource, PlatformMonitor,
    PlatformViewportBackend, RendererViewportBackend, SuspendedContext, Ui, Viewport,
};
use imgui_rs_vulkan_renderer::{DynamicRendering, Options, Renderer};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use std::ffi::c_void;
use std::time::Duration;
use winit::{event::Event, window::Window};

pub enum Gui {
    Screen(GuiScreen),
    InWorld(GuiInWorld),
}

pub struct GuiScreen {
    context: Option<SuspendedContext>,
    renderer: Renderer,
    platform: WinitPlatform,
}

pub struct GuiInWorld {
    context: Option<SuspendedContext>,
    renderer: Renderer,
}

// Called by the Engine
impl Gui {
    pub(crate) fn handle_event<T>(&mut self, window: &Window, event: &Event<T>) {
        match self {
            Gui::Screen(gui_screen) => gui_screen.handle_event(window, event),
            _ => {}
        }
    }

    pub(crate) fn update_delta_time(&mut self, frame_time: Duration) {
        match self {
            Gui::Screen(gui_screen) => gui_screen.update_delta_time(frame_time),
            Gui::InWorld(gui_in_world) => gui_in_world.update_delta_time(frame_time),
        }
    }
}

impl GuiScreen {
    pub fn new(
        context: &vulkan::Context,
        command_pool: &CommandPool,
        window: &Window,
        format: Format,
        in_flight_frames: usize,
    ) -> Result<GuiScreen> {
        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);

        let mut platform = WinitPlatform::init(&mut imgui);

        let hidpi_factor = platform.hidpi_factor();
        let font_size = (13.0 * hidpi_factor) as f32;
        imgui.fonts().add_font(&[
            FontSource::DefaultFontData {
                config: Some(FontConfig {
                    size_pixels: font_size,
                    ..FontConfig::default()
                }),
            },
            FontSource::TtfData {
                data: include_bytes!("../assets/fonts/mplus-1p-regular.ttf"),
                size_pixels: font_size,
                config: Some(FontConfig {
                    rasterizer_multiply: 1.75,
                    ..FontConfig::default()
                }),
            },
        ]);
        imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;
        platform.attach_window(imgui.io_mut(), window, HiDpiMode::Rounded);

        let renderer = Renderer::with_gpu_allocator(
            context.allocator.clone(),
            context.device.inner.clone(),
            context.graphics_queue.inner,
            command_pool.inner,
            DynamicRendering {
                color_attachment_format: format,
                depth_attachment_format: Some(Format::D32_SFLOAT),
            },
            &mut imgui,
            Some(Options {
                in_flight_frames,
                ..Default::default()
            }),
        )?;

        Ok(GuiScreen {
            context: Some(imgui.suspend()),
            renderer,
            platform,
        })
    }

    pub(crate) fn update_delta_time(&mut self, frame_time: Duration) {
        let mut imgui = self.context.take().unwrap().activate().unwrap();
        imgui.io_mut().update_delta_time(frame_time);
        self.context = Some(imgui.suspend());
    }

    pub(crate) fn handle_event<T>(&mut self, window: &Window, event: &Event<T>) {
        let mut imgui = self.context.take().unwrap().activate().unwrap();
        self.platform.handle_event(imgui.io_mut(), &window, &event);
        self.context = Some(imgui.suspend());
    }

    pub fn render<F: FnOnce(&Ui) -> Result<()>>(
        &mut self,
        buffer: &CommandBuffer,
        window: &Window,
        build: F,
    ) -> Result<()> {
        let mut imgui = self.context.take().unwrap().activate().unwrap();
        self.platform.prepare_frame(imgui.io_mut(), window)?;
        let ui = imgui.new_frame();

        build(&ui)?;

        self.platform.prepare_render(&ui, window);
        let draw_data = imgui.render();
        self.renderer.cmd_draw(
            buffer.inner,
            draw_data,
            Extent2D {
                width: window.inner_size().width,
                height: window.inner_size().height,
            },
            None,
        )?;

        self.context = Some(imgui.suspend());

        Ok(())
    }
}

struct PlatformBackend {}
struct RenderBackend {}
impl GuiInWorld {
    pub fn new(
        context: &vulkan::Context,
        command_pool: &CommandPool,
        format: Format,
        in_flight_frames: usize,
    ) -> Result<GuiInWorld> {
        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);

        let font_size = 13.0;
        imgui.fonts().add_font(&[
            FontSource::DefaultFontData {
                config: Some(FontConfig {
                    size_pixels: font_size,
                    ..FontConfig::default()
                }),
            },
            FontSource::TtfData {
                data: include_bytes!("../assets/fonts/mplus-1p-regular.ttf"),
                size_pixels: font_size,
                config: Some(FontConfig {
                    rasterizer_multiply: 1.75,
                    ..FontConfig::default()
                }),
            },
        ]);
        imgui.io_mut().font_global_scale = 1.0;
        //imgui.io_mut().display_size = [display_size.x as f32, display_size.y as f32];
        imgui.io_mut().display_size = [1.0, 1.0];
        imgui.io_mut().config_flags |= ConfigFlags::DOCKING_ENABLE;
        imgui.io_mut().config_flags |= ConfigFlags::VIEWPORTS_ENABLE;
        imgui.io_mut().backend_flags |= BackendFlags::PLATFORM_HAS_VIEWPORTS;
        imgui.io_mut().backend_flags |= BackendFlags::RENDERER_HAS_VIEWPORTS;
        imgui
            .platform_io_mut()
            .monitors
            .replace_from_slice(&[PlatformMonitor {
                main_pos: [0.0, 0.0],
                main_size: [f32::MAX, f32::MAX],
                work_pos: [0.0, 0.0],
                work_size: [f32::MAX, f32::MAX],
                dpi_scale: 1.0,
            }]);
        imgui.main_viewport_mut().size = [1.0, 1.0];

        let mut platform_backend = PlatformBackend {};
        let ptr: *mut PlatformBackend = &mut platform_backend;
        let voidptr = ptr as *mut c_void;
        imgui.main_viewport_mut().platform_handle = voidptr;
        imgui.set_platform_backend(platform_backend);
        imgui.set_renderer_backend(RenderBackend {});

        let renderer = Renderer::with_gpu_allocator(
            context.allocator.clone(),
            context.device.inner.clone(),
            context.graphics_queue.inner,
            command_pool.inner,
            DynamicRendering {
                color_attachment_format: format,
                depth_attachment_format: Some(vk::Format::D32_SFLOAT),
            },
            &mut imgui,
            Some(Options {
                in_flight_frames,
                render_3d: true,
                ..Default::default()
            }),
        )?;

        Ok(GuiInWorld {
            context: Some(imgui.suspend()),
            renderer,
        })
    }

    pub(crate) fn update_delta_time(&mut self, frame_time: Duration) {
        let mut imgui = self.context.take().unwrap().activate().unwrap();
        imgui.io_mut().update_delta_time(frame_time);
        self.context = Some(imgui.suspend());
    }

    pub fn render<F: FnOnce(&Ui) -> Result<()>>(
        &mut self,
        buffer: &CommandBuffer,
        extent: Extent2D,
        camera: &Camera,
        build: F,
    ) -> Result<()> {
        let mut imgui = self.context.take().unwrap().activate().unwrap();

        let ui = imgui.new_frame();
        ui.window("Test")
            .position([1.0, 0.0], Condition::FirstUseEver)
            .size([300.0, 300.0], Condition::FirstUseEver)
            .resizable(false)
            .movable(false)
            .build(|| {
                ui.text("Hello World");
            });

        ui.window("Test 2")
            .position([2.0, 0.0], Condition::FirstUseEver)
            .size([300.0, 300.0], Condition::FirstUseEver)
            .resizable(false)
            .movable(false)
            .build(|| {
                ui.text("Hello World2");
            });

        imgui.render();
        imgui.update_platform_windows();
        imgui.render_platform_windows_default();

        for (i, viewport) in imgui.viewports().enumerate() {
            let transform = Mat4::from_scale(vec3(1.0, -1.0, 1.0) * 0.01)
                * Mat4::from_rotation_x(f32::to_radians(0.0))
                * Mat4::from_translation(vec3(-(i as f32), 0.0, 0.0));

            let mat = camera.projection_matrix() * camera.view_matrix() * transform;

            let draw_data = viewport.draw_data();
            self.renderer
                .cmd_draw(buffer.inner, draw_data, extent, Some(&mat.to_cols_array()))?;
        }
        self.context = Some(imgui.suspend());
        Ok(())
    }
}

impl PlatformViewportBackend for PlatformBackend {
    fn create_window(&mut self, _: &mut Viewport) {}
    fn destroy_window(&mut self, _: &mut Viewport) {}
    fn show_window(&mut self, _: &mut Viewport) {}
    fn set_window_pos(&mut self, _: &mut Viewport, _: [f32; 2]) {}
    fn get_window_pos(&mut self, _: &mut Viewport) -> [f32; 2] {
        [0.0, 0.0]
    }
    fn set_window_size(&mut self, _: &mut Viewport, _: [f32; 2]) {}
    fn get_window_size(&mut self, _: &mut Viewport) -> [f32; 2] {
        [0.0, 0.0]
    }
    fn set_window_focus(&mut self, _: &mut Viewport) {}
    fn get_window_focus(&mut self, _: &mut Viewport) -> bool {
        false
    }
    fn get_window_minimized(&mut self, _: &mut Viewport) -> bool {
        false
    }
    fn set_window_title(&mut self, _: &mut Viewport, _: &str) {}
    fn set_window_alpha(&mut self, _: &mut Viewport, _: f32) {}
    fn update_window(&mut self, _: &mut Viewport) {}
    fn render_window(&mut self, _: &mut Viewport) {}
    fn swap_buffers(&mut self, _: &mut Viewport) {}
    fn create_vk_surface(&mut self, _: &mut Viewport, _: u64, _: &mut u64) -> i32 {
        0
    }
}

impl RendererViewportBackend for RenderBackend {
    fn create_window(&mut self, _: &mut Viewport) {}
    fn destroy_window(&mut self, _: &mut Viewport) {}
    fn set_window_size(&mut self, _: &mut Viewport, _: [f32; 2]) {}
    fn render_window(&mut self, _: &mut Viewport) {}
    fn swap_buffers(&mut self, _: &mut Viewport) {}
}
