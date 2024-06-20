use winit::window::Fullscreen;
use crate::{App, BaseApp};

impl<B: App> BaseApp<B> {
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
