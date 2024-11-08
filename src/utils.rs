use winit::window::Fullscreen;
use crate::{Engine};

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
}
