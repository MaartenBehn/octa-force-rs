use glam::{vec2, Vec2};
use winit::event::{DeviceEvent, ElementState, Event, KeyEvent, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Debug, Clone, Copy, Default)]
pub struct Controls {
    pub w: bool,
    pub s: bool,
    pub d: bool,
    pub a: bool,
    pub up: bool,
    pub down: bool,
    pub rigth: bool,
    pub left: bool,

    pub q: bool,
    pub e: bool,
    pub r: bool,
    pub t: bool,
    pub lshift: bool,

    pub f1: bool,
    pub f2: bool,
    pub f3: bool,
    pub f4: bool,
    pub f5: bool,
    pub f6: bool,
    pub f7: bool,
    pub f8: bool,
    pub f9: bool,
    pub f10: bool,
    pub f11: bool,
    pub f12: bool,

    pub cursor_delta: Vec2,
    pub scroll_delta: f32,
}

impl Controls {
    pub fn reset(&mut self) {
        self.cursor_delta = Vec2::ZERO;
        self.scroll_delta = 0.0;
    }

    pub fn handle_event(&mut self, event: &Event<()>) {
        match event {
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::KeyboardInput {
                        event:
                        KeyEvent {
                            physical_key: PhysicalKey::Code(code),
                            state,
                            ..
                        },
                        ..
                    } => match *code {
                        KeyCode::KeyA => {self.a = *state == ElementState::Pressed;}
                        KeyCode::KeyD => {self.d = *state == ElementState::Pressed;}
                        KeyCode::KeyS => {self.s = *state == ElementState::Pressed;}
                        KeyCode::KeyW => {self.w = *state == ElementState::Pressed;}
                     
                        KeyCode::ArrowDown => {self.down = *state == ElementState::Pressed;}
                        KeyCode::ArrowLeft => {self.left = *state == ElementState::Pressed;}
                        KeyCode::ArrowRight => {self.rigth = *state == ElementState::Pressed;}
                        KeyCode::ArrowUp => {self.up = *state == ElementState::Pressed;}

                        KeyCode::KeyQ => {self.q = *state == ElementState::Pressed;}
                        KeyCode::KeyE => {self.e = *state == ElementState::Pressed;}
                        KeyCode::KeyR => {self.r = *state == ElementState::Pressed;}
                        KeyCode::KeyT => {self.t = *state == ElementState::Pressed;}
                        
                        KeyCode::ShiftLeft => {self.lshift = *state == ElementState::Pressed;}
                        
                        KeyCode::F1 => {self.f1 = *state == ElementState::Pressed;}
                        KeyCode::F2 => {self.f2 = *state == ElementState::Pressed;}
                        KeyCode::F3 => {self.f3 = *state == ElementState::Pressed;}
                        KeyCode::F4 => {self.f4 = *state == ElementState::Pressed;}
                        KeyCode::F5 => {self.f5 = *state == ElementState::Pressed;}
                        KeyCode::F6 => {self.f6 = *state == ElementState::Pressed;}
                        KeyCode::F7 => {self.f7 = *state == ElementState::Pressed;}
                        KeyCode::F8 => {self.f8 = *state == ElementState::Pressed;}
                        KeyCode::F9 => {self.f9 = *state == ElementState::Pressed;}
                        KeyCode::F10 => {self.f10 = *state == ElementState::Pressed;}
                        KeyCode::F11 => {self.f11 = *state == ElementState::Pressed;}
                        KeyCode::F12 => {self.f12 = *state == ElementState::Pressed;}
                        
                        _ => {}
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        if *button == MouseButton::Right {
                            self.rigth = *state == ElementState::Pressed;
                        }

                        if *button == MouseButton::Left {
                            self.left = *state == ElementState::Pressed;
                        }
                    }
                    _ => {}
                };
            }
            Event::DeviceEvent { event, .. } => {
                match event {
                    DeviceEvent::MouseMotion { delta: (x, y) } => {
                        let x = *x as f32;
                        let y = *y as f32;
                        self.cursor_delta =
                            vec2(self.cursor_delta[0] + x, self.cursor_delta[1] + y);
                    }
                    DeviceEvent::MouseWheel { delta } => match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => {
                            self.scroll_delta = *y;
                        }
                        winit::event::MouseScrollDelta::PixelDelta(d) => {
                            self.scroll_delta = d.y as f32;
                        }
                    },

                    _ => (),
                };
            }
            _ => (),
        }
    }
}
