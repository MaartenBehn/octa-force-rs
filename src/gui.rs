use crate::camera::Camera;
use crate::{App, BaseApp, vulkan};
use crate::vulkan::{CommandBuffer, CommandPool};
use anyhow::Result;
use ash::vk;
use ash::vk::{Extent2D, Format};
use glam::{vec3, Mat4, Vec3, Quat, EulerRot};
use imgui::{BackendFlags, ConfigFlags, FontConfig, FontId, FontSource, PlatformMonitor, PlatformViewportBackend, RendererViewportBackend, Style, SuspendedContext, Ui, Viewport};
use imgui_rs_vulkan_renderer::{DynamicRendering, Options, Renderer};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use std::ffi::c_void;
use std::time::Duration;
use winit::{event::Event, window::Window};

pub type GuiId = usize;

pub struct ScreenGui {
    context: Option<SuspendedContext>,
    renderer: Renderer,
    platform: WinitPlatform,
}

pub struct InWorldGui {
    context: Option<SuspendedContext>,
    renderer: Renderer,
    transform_mats: Vec<Mat4>,
}


impl<B: App> BaseApp<B> {
    pub fn add_screen_gui(&mut self) -> Result<GuiId> {
        let gui = ScreenGui::new(&self.context, &self.command_pool, &self.window, self.swapchain.format, self.swapchain.images.len())?;
        self.screen_guis.push(gui);
        Ok(self.screen_guis.len() -1)
    }

    pub fn add_in_world_gui(&mut self) -> Result<GuiId> {
        let gui = InWorldGui::new(&self.context, &self.command_pool, self.swapchain.format, self.swapchain.images.len())?;
        self.in_world_guis.push(gui);
        Ok(self.in_world_guis.len() -1)
    }
}

impl ScreenGui {
    pub fn new(
        context: &vulkan::Context,
        command_pool: &CommandPool,
        window: &Window,
        format: Format,
        in_flight_frames: usize,
    ) -> Result<ScreenGui> {
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

        Ok(ScreenGui {
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

    pub fn draw<F: FnOnce(&Ui) -> Result<()>>(
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
        )?;

        self.context = Some(imgui.suspend());

        Ok(())
    }

    pub fn set_style<F: FnOnce(&mut Style)>(&mut self, set: F) {
        let mut imgui = self.context.take().unwrap().activate().unwrap();
        set(imgui.style_mut());
        self.context = Some(imgui.suspend());
    }

    pub fn add_font(&mut self, font_sources: &[FontSource<'_>]) -> FontId {
        let mut imgui = self.context.take().unwrap().activate().unwrap();
        let id = imgui.fonts().add_font(font_sources);
        self.context = Some(imgui.suspend());

        return id
    }
}



struct PlatformBackend {}
struct RenderBackend {}

#[derive(Debug, Clone, Copy)]
pub struct InWorldGuiTransform {
    pub pos: Vec3,
    pub rot: Vec3,
    pub scale: Vec3,
}
impl InWorldGui {
    pub fn new(
        context: &vulkan::Context,
        command_pool: &CommandPool,
        format: Format,
        in_flight_frames: usize,
    ) -> Result<InWorldGui> {
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
                enable_depth_test: true,
                enable_depth_write: true,
            }),
        )?;

        Ok(InWorldGui {
            context: Some(imgui.suspend()),
            renderer,
            transform_mats: Vec::new(),
        })
    }

    pub(crate) fn update_delta_time(&mut self, frame_time: Duration) {
        let mut imgui = self.context.take().unwrap().activate().unwrap();
        imgui.io_mut().update_delta_time(frame_time);
        self.context = Some(imgui.suspend());
    }

    pub fn set_transfrom(&mut self, transforms: &Vec<InWorldGuiTransform>) {
        self.transform_mats.clear();
        
        for (i, t) in transforms.iter().enumerate() {
            let quat = Quat::from_euler(EulerRot::XYZ, t.rot.x.to_radians(), t.rot.y.to_radians(), t.rot.z.to_radians());
            let factor = 0.005;
            let mat = Mat4::from_scale_rotation_translation(
                vec3(1.0, -1.0, 1.0) * factor * t.scale,
                quat,
                vec3(-(i as f32) * factor, 0.0, 0.0) + t.pos
            );

            self.transform_mats.push(mat);
        }
    }

    pub fn draw<F: FnOnce(&Ui) -> Result<()>>(
        &mut self,
        buffer: &CommandBuffer,
        extent: Extent2D,
        camera: &Camera,
        build: F,
    ) -> Result<()> {
        let mut imgui = self.context.take().unwrap().activate().unwrap();

        let ui = imgui.new_frame();
        build(ui)?;

        imgui.render();
        imgui.update_platform_windows();
        imgui.render_platform_windows_default();


        debug_assert!(imgui.viewports().count() == self.transform_mats.len() + 1, "transform_mats {} size dosen't match created windows {}", self.transform_mats.len() + 1, imgui.viewports().count());

        let cam_mat = camera.projection_matrix() * camera.view_matrix();

        let mut mats = Vec::new();
        let mut draw_datas = Vec::new();
        for (i, viewport) in imgui.viewports().enumerate().skip(1) {
            let mat = cam_mat * self.transform_mats[i - 1];
            let draw_data = viewport.draw_data();

            mats.push(mat.to_cols_array());
            draw_datas.push(draw_data);
        }

        self.renderer
            .cmd_draw_3d(buffer.inner, draw_datas.as_slice(), mats.as_slice(), extent)?;
        self.context = Some(imgui.suspend());
        Ok(())
    }

    pub fn set_style<F: FnOnce(&mut Style)>(&mut self, set: F) {
        let mut imgui = self.context.take().unwrap().activate().unwrap();
        set(imgui.style_mut());
        self.context = Some(imgui.suspend());
    }

    pub fn add_font(&mut self, font_sources: &[FontSource<'_>]) -> FontId {
        let mut imgui = self.context.take().unwrap().activate().unwrap();
        let id = imgui.fonts().add_font(font_sources);
        self.context = Some(imgui.suspend());

        return id
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

impl Default for InWorldGuiTransform {

    fn default() -> Self {
        Self{
            pos: Vec3::ZERO,
            rot: Vec3::ZERO,
            scale: Vec3::ONE,
        }
    }
}
