use glam::UVec2;
use winit::window::Fullscreen;
use crate::{vulkan::{CommandBuffer, ImageAndView}, Engine};

impl Engine {
    pub fn set_fullscreen(&self, value: bool) {
        let fullscreen = if value {
            Some(Fullscreen::Borderless(None))
        } else {
            None
        };

        self.window.set_fullscreen(fullscreen);
    }

    pub fn get_fullscreen(&self) -> bool {
        self.window.fullscreen().is_some()
    }

    pub fn get_current_command_buffer(&self) -> &CommandBuffer {
        &self.command_buffers[self.in_flight_frames.in_flight_index]
    }
 
    pub fn get_current_swapchain_image_and_view(&self) -> &ImageAndView {
        &self.swapchain.images_and_views[self.in_flight_frames.frame_index]
    }

    pub fn get_current_depth_image_and_view(&self) -> &ImageAndView {
        &self.swapchain.depht_images_and_views[self.in_flight_frames.frame_index]
    }

    pub fn get_resolution(&self) -> UVec2 {
        self.swapchain.size
    }

    pub fn get_num_frames_in_flight(&self) -> usize {
        self.in_flight_frames.num_frames
    }

    pub fn get_current_in_flight_frame_index(&self) -> usize {
        self.in_flight_frames.in_flight_index
    }
}
