use glam::{vec2, Vec2};
use winit::event::{
    DeviceEvent, ElementState, Event, KeyboardInput, MouseButton, VirtualKeyCode, WindowEvent,
};

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
                        input:
                            KeyboardInput {
                                state,
                                virtual_keycode,
                                ..
                            },
                        ..
                    } => {
                        if virtual_keycode.is_some() {
                            match virtual_keycode.unwrap() {
                                VirtualKeyCode::W => {
                                    self.w = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::S => {
                                    self.s = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::A => {
                                    self.a = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::D => {
                                    self.d = *state == ElementState::Pressed;
                                }

                                VirtualKeyCode::Up => {
                                    self.up = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::Down => {
                                    self.down = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::Left => {
                                    self.left = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::Right => {
                                    self.rigth = *state == ElementState::Pressed;
                                }

                                VirtualKeyCode::Q => {
                                    self.q = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::E => {
                                    self.e = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::R => {
                                    self.r = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::T => {
                                    self.t = *state == ElementState::Pressed;
                                }

                                VirtualKeyCode::LShift => {
                                    self.lshift = *state == ElementState::Pressed;
                                }

                                VirtualKeyCode::F1 => {
                                    self.f1 = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::F2 => {
                                    self.f2 = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::F3 => {
                                    self.f3 = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::F4 => {
                                    self.f4 = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::F5 => {
                                    self.f5 = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::F6 => {
                                    self.f6 = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::F7 => {
                                    self.f7 = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::F8 => {
                                    self.f8 = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::F9 => {
                                    self.f9 = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::F10 => {
                                    self.f10 = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::F11 => {
                                    self.f11 = *state == ElementState::Pressed;
                                }
                                VirtualKeyCode::F12 => {
                                    self.f12 = *state == ElementState::Pressed;
                                }

                                _ => {}
                            }
                        }
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
