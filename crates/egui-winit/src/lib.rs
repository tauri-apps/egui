//! [`egui`] bindings for [`winit`](https://github.com/rust-windowing/winit).
//!
//! The library translates winit events to egui, handled copy/paste,
//! updates the cursor, open links clicked in egui, etc.
//!
//! ## Feature flags
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]
//!

#![allow(clippy::manual_range_contains)]

use std::os::raw::c_void;

pub use egui;
pub use winit;

pub mod clipboard;
pub mod screen_reader;
mod window_settings;

pub use window_settings::WindowSettings;

use winit::event_loop::EventLoopWindowTarget;

pub fn native_pixels_per_point(window: &winit::window::Window) -> f32 {
    window.scale_factor() as f32
}

pub fn screen_size_in_pixels(window: &winit::window::Window) -> egui::Vec2 {
    let size = window.inner_size();
    egui::vec2(size.width as f32, size.height as f32)
}

/// Handles the integration between egui and winit.
pub struct State {
    start_time: instant::Instant,
    egui_input: egui::RawInput,
    pointer_pos_in_points: Option<egui::Pos2>,
    any_pointer_button_down: bool,
    current_cursor_icon: egui::CursorIcon,
    /// What egui uses.
    current_pixels_per_point: f32,

    clipboard: clipboard::Clipboard,
    screen_reader: screen_reader::ScreenReader,

    /// If `true`, mouse inputs will be treated as touches.
    /// Useful for debugging touch support in egui.
    ///
    /// Creates duplicate touches, if real touch inputs are coming.
    simulate_touch_screen: bool,

    /// Is Some(…) when a touch is being translated to a pointer.
    ///
    /// Only one touch will be interpreted as pointer at any time.
    pointer_touch_id: Option<u64>,
}

impl State {
    pub fn new<T>(event_loop: &EventLoopWindowTarget<T>) -> Self {
        Self::new_with_wayland_display(wayland_display(event_loop))
    }

    pub fn new_with_wayland_display(wayland_display: Option<*mut c_void>) -> Self {
        let egui_input = egui::RawInput {
            has_focus: false, // winit will tell us when we have focus
            ..Default::default()
        };

        Self {
            start_time: instant::Instant::now(),
            egui_input,
            pointer_pos_in_points: None,
            any_pointer_button_down: false,
            current_cursor_icon: egui::CursorIcon::Default,
            current_pixels_per_point: 1.0,

            clipboard: clipboard::Clipboard::new(wayland_display),
            screen_reader: screen_reader::ScreenReader::default(),

            simulate_touch_screen: false,
            pointer_touch_id: None,
        }
    }

    /// Call this once a graphics context has been created to update the maximum texture dimensions
    /// that egui will use.
    pub fn set_max_texture_side(&mut self, max_texture_side: usize) {
        self.egui_input.max_texture_side = Some(max_texture_side);
    }

    /// Call this when a new native Window is created for rendering to initialize the `pixels_per_point`
    /// for that window.
    ///
    /// In particular, on Android it is necessary to call this after each `Resumed` lifecycle
    /// event, each time a new native window is created.
    ///
    /// Once this has been initialized for a new window then this state will be maintained by handling
    /// [`winit::event::WindowEvent::ScaleFactorChanged`] events.
    pub fn set_pixels_per_point(&mut self, pixels_per_point: f32) {
        self.egui_input.pixels_per_point = Some(pixels_per_point);
        self.current_pixels_per_point = pixels_per_point;
    }

    /// The number of physical pixels per logical point,
    /// as configured on the current egui context (see [`egui::Context::pixels_per_point`]).
    #[inline]
    pub fn pixels_per_point(&self) -> f32 {
        self.current_pixels_per_point
    }

    /// The current input state.
    /// This is changed by [`Self::on_event`] and cleared by [`Self::take_egui_input`].
    #[inline]
    pub fn egui_input(&self) -> &egui::RawInput {
        &self.egui_input
    }

    /// Prepare for a new frame by extracting the accumulated input,
    /// as well as setting [the time](egui::RawInput::time) and [screen rectangle](egui::RawInput::screen_rect).
    pub fn take_egui_input(&mut self, window: &winit::window::Window) -> egui::RawInput {
        let pixels_per_point = self.pixels_per_point();

        self.egui_input.time = Some(self.start_time.elapsed().as_secs_f64());

        // On Windows, a minimized window will have 0 width and height.
        // See: https://github.com/rust-windowing/winit/issues/208
        // This solves an issue where egui window positions would be changed when minimizing on Windows.
        let screen_size_in_pixels = screen_size_in_pixels(window);
        let screen_size_in_points = screen_size_in_pixels / pixels_per_point;
        self.egui_input.screen_rect =
            if screen_size_in_points.x > 0.0 && screen_size_in_points.y > 0.0 {
                Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    screen_size_in_points,
                ))
            } else {
                None
            };

        self.egui_input.take()
    }

    /// Call this when there is a new event.
    ///
    /// The result can be found in [`Self::egui_input`] and be extracted with [`Self::take_egui_input`].
    ///
    /// Returns `true` if egui wants exclusive use of this event
    /// (e.g. a mouse click on an egui window, or entering text into a text field).
    /// For instance, if you use egui for a game, you want to first call this
    /// and only when this returns `false` pass on the events to your game.
    ///
    /// Note that egui uses `tab` to move focus between elements, so this will always return `true` for tabs.
    pub fn on_event(
        &mut self,
        egui_ctx: &egui::Context,
        event: &winit::event::WindowEvent<'_>,
    ) -> bool {
        use winit::event::WindowEvent;
        match event {
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let pixels_per_point = *scale_factor as f32;
                self.egui_input.pixels_per_point = Some(pixels_per_point);
                self.current_pixels_per_point = pixels_per_point;
                false
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.on_mouse_button_input(*state, *button);
                egui_ctx.wants_pointer_input()
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.on_mouse_wheel(*delta);
                egui_ctx.wants_pointer_input()
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.on_cursor_moved(*position);
                egui_ctx.is_using_pointer()
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer_pos_in_points = None;
                self.egui_input.events.push(egui::Event::PointerGone);
                false
            }
            // WindowEvent::TouchpadPressure {device_id, pressure, stage, ..  } => {} // TODO
            WindowEvent::Touch(touch) => {
                self.on_touch(touch);
                match touch.phase {
                    winit::event::TouchPhase::Started
                    | winit::event::TouchPhase::Ended
                    | winit::event::TouchPhase::Cancelled => egui_ctx.wants_pointer_input(),
                    winit::event::TouchPhase::Moved => egui_ctx.is_using_pointer(),
                    _ => unreachable!(),
                }
            }
            WindowEvent::ReceivedImeText(ch) => {
                // On Mac we get here when the user presses Cmd-C (copy), ctrl-W, etc.
                // We need to ignore these characters that are side-effects of commands.
                let is_mac_cmd = cfg!(target_os = "macos")
                    && (self.egui_input.modifiers.ctrl || self.egui_input.modifiers.mac_cmd);

                if ch.chars().all(is_printable_char) && !is_mac_cmd {
                    self.egui_input
                        .events
                        .push(egui::Event::Text(ch.to_string()));
                    egui_ctx.wants_keyboard_input()
                } else {
                    false
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.on_keyboard_input(event);
                egui_ctx.wants_keyboard_input()
                    || event.physical_key == winit::keyboard::KeyCode::Tab
            }
            WindowEvent::Focused(has_focus) => {
                self.egui_input.has_focus = *has_focus;
                // We will not be given a KeyboardInput event when the modifiers are released while
                // the window does not have focus. Unset all modifier state to be safe.
                self.egui_input.modifiers = egui::Modifiers::default();
                false
            }
            WindowEvent::HoveredFile(path) => {
                self.egui_input.hovered_files.push(egui::HoveredFile {
                    path: Some(path.clone()),
                    ..Default::default()
                });
                false
            }
            WindowEvent::HoveredFileCancelled => {
                self.egui_input.hovered_files.clear();
                false
            }
            WindowEvent::DroppedFile(path) => {
                self.egui_input.hovered_files.clear();
                self.egui_input.dropped_files.push(egui::DroppedFile {
                    path: Some(path.clone()),
                    ..Default::default()
                });
                false
            }
            WindowEvent::ModifiersChanged(state) => {
                self.egui_input.modifiers.alt = state.alt_key();
                self.egui_input.modifiers.ctrl = state.control_key();
                self.egui_input.modifiers.shift = state.shift_key();
                self.egui_input.modifiers.mac_cmd = cfg!(target_os = "macos") && state.super_key();
                self.egui_input.modifiers.command = if cfg!(target_os = "macos") {
                    state.super_key()
                } else {
                    state.control_key()
                };
                false
            }
            _ => {
                // dbg!(event);
                false
            }
        }
    }

    fn on_mouse_button_input(
        &mut self,
        state: winit::event::ElementState,
        button: winit::event::MouseButton,
    ) {
        if let Some(pos) = self.pointer_pos_in_points {
            if let Some(button) = translate_mouse_button(button) {
                let pressed = state == winit::event::ElementState::Pressed;

                self.egui_input.events.push(egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: self.egui_input.modifiers,
                });

                if self.simulate_touch_screen {
                    if pressed {
                        self.any_pointer_button_down = true;

                        self.egui_input.events.push(egui::Event::Touch {
                            device_id: egui::TouchDeviceId(0),
                            id: egui::TouchId(0),
                            phase: egui::TouchPhase::Start,
                            pos,
                            force: 0.0,
                        });
                    } else {
                        self.any_pointer_button_down = false;

                        self.egui_input.events.push(egui::Event::PointerGone);

                        self.egui_input.events.push(egui::Event::Touch {
                            device_id: egui::TouchDeviceId(0),
                            id: egui::TouchId(0),
                            phase: egui::TouchPhase::End,
                            pos,
                            force: 0.0,
                        });
                    };
                }
            }
        }
    }

    fn on_cursor_moved(&mut self, pos_in_pixels: winit::dpi::PhysicalPosition<f64>) {
        let pos_in_points = egui::pos2(
            pos_in_pixels.x as f32 / self.pixels_per_point(),
            pos_in_pixels.y as f32 / self.pixels_per_point(),
        );
        self.pointer_pos_in_points = Some(pos_in_points);

        if self.simulate_touch_screen {
            if self.any_pointer_button_down {
                self.egui_input
                    .events
                    .push(egui::Event::PointerMoved(pos_in_points));

                self.egui_input.events.push(egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(0),
                    phase: egui::TouchPhase::Move,
                    pos: pos_in_points,
                    force: 0.0,
                });
            }
        } else {
            self.egui_input
                .events
                .push(egui::Event::PointerMoved(pos_in_points));
        }
    }

    fn on_touch(&mut self, touch: &winit::event::Touch) {
        // Emit touch event
        self.egui_input.events.push(egui::Event::Touch {
            device_id: egui::TouchDeviceId(egui::epaint::util::hash(touch.device_id)),
            id: egui::TouchId::from(touch.id),
            phase: match touch.phase {
                winit::event::TouchPhase::Started => egui::TouchPhase::Start,
                winit::event::TouchPhase::Moved => egui::TouchPhase::Move,
                winit::event::TouchPhase::Ended => egui::TouchPhase::End,
                winit::event::TouchPhase::Cancelled => egui::TouchPhase::Cancel,
                _ => unreachable!(),
            },
            pos: egui::pos2(
                touch.location.x as f32 / self.pixels_per_point(),
                touch.location.y as f32 / self.pixels_per_point(),
            ),
            force: match touch.force {
                Some(winit::event::Force::Normalized(force)) => force as f32,
                Some(winit::event::Force::Calibrated {
                    force,
                    max_possible_force,
                    ..
                }) => (force / max_possible_force) as f32,
                None => 0_f32,
                _ => unreachable!(),
            },
        });
        // If we're not yet tanslating a touch or we're translating this very
        // touch …
        if self.pointer_touch_id.is_none() || self.pointer_touch_id.unwrap() == touch.id {
            // … emit PointerButton resp. PointerMoved events to emulate mouse
            match touch.phase {
                winit::event::TouchPhase::Started => {
                    self.pointer_touch_id = Some(touch.id);
                    // First move the pointer to the right location
                    self.on_cursor_moved(touch.location);
                    self.on_mouse_button_input(
                        winit::event::ElementState::Pressed,
                        winit::event::MouseButton::Left,
                    );
                }
                winit::event::TouchPhase::Moved => {
                    self.on_cursor_moved(touch.location);
                }
                winit::event::TouchPhase::Ended => {
                    self.pointer_touch_id = None;
                    self.on_mouse_button_input(
                        winit::event::ElementState::Released,
                        winit::event::MouseButton::Left,
                    );
                    // The pointer should vanish completely to not get any
                    // hover effects
                    self.pointer_pos_in_points = None;
                    self.egui_input.events.push(egui::Event::PointerGone);
                }
                winit::event::TouchPhase::Cancelled => {
                    self.pointer_touch_id = None;
                    self.pointer_pos_in_points = None;
                    self.egui_input.events.push(egui::Event::PointerGone);
                }
                _ => unreachable!(),
            }
        }
    }

    fn on_mouse_wheel(&mut self, delta: winit::event::MouseScrollDelta) {
        let delta = match delta {
            winit::event::MouseScrollDelta::LineDelta(x, y) => {
                let points_per_scroll_line = 50.0; // Scroll speed decided by consensus: https://github.com/emilk/egui/issues/461
                egui::vec2(x, y) * points_per_scroll_line
            }
            winit::event::MouseScrollDelta::PixelDelta(delta) => {
                egui::vec2(delta.x as f32, delta.y as f32) / self.pixels_per_point()
            }
            _ => unreachable!(),
        };

        if self.egui_input.modifiers.ctrl || self.egui_input.modifiers.command {
            // Treat as zoom instead:
            let factor = (delta.y / 200.0).exp();
            self.egui_input.events.push(egui::Event::Zoom(factor));
        } else if self.egui_input.modifiers.shift {
            // Treat as horizontal scrolling.
            // Note: one Mac we already get horizontal scroll events when shift is down.
            self.egui_input
                .events
                .push(egui::Event::Scroll(egui::vec2(delta.x + delta.y, 0.0)));
        } else {
            self.egui_input.events.push(egui::Event::Scroll(delta));
        }
    }

    fn on_keyboard_input(&mut self, input: &winit::event::KeyEvent) {
        let keycode = input.physical_key;
        let pressed = input.state == winit::event::ElementState::Pressed;

        if pressed {
            // VirtualKeyCode::Paste etc in winit are broken/untrustworthy,
            // so we detect these things manually:
            if is_cut_command(self.egui_input.modifiers, keycode) {
                self.egui_input.events.push(egui::Event::Cut);
            } else if is_copy_command(self.egui_input.modifiers, keycode) {
                self.egui_input.events.push(egui::Event::Copy);
            } else if is_paste_command(self.egui_input.modifiers, keycode) {
                if let Some(contents) = self.clipboard.get() {
                    self.egui_input
                        .events
                        .push(egui::Event::Paste(contents.replace("\r\n", "\n")));
                }
            }
        }

        if let Some(key) = translate_virtual_key_code(input.physical_key) {
            self.egui_input.events.push(egui::Event::Key {
                key,
                pressed,
                modifiers: self.egui_input.modifiers,
            });
        }
    }

    /// Call with the output given by `egui`.
    ///
    /// This will, if needed:
    /// * update the cursor
    /// * copy text to the clipboard
    /// * open any clicked urls
    /// * update the IME
    /// *
    pub fn handle_platform_output(
        &mut self,
        window: &winit::window::Window,
        egui_ctx: &egui::Context,
        platform_output: egui::PlatformOutput,
    ) {
        if egui_ctx.options().screen_reader {
            self.screen_reader
                .speak(&platform_output.events_description());
        }

        let egui::PlatformOutput {
            cursor_icon,
            open_url,
            copied_text,
            events: _,                    // handled above
            mutable_text_under_cursor: _, // only used in eframe web
            text_cursor_pos,
        } = platform_output;
        self.current_pixels_per_point = egui_ctx.pixels_per_point(); // someone can have changed it to scale the UI

        self.set_cursor_icon(window, cursor_icon);

        if let Some(open_url) = open_url {
            open_url_in_browser(&open_url.url);
        }

        if !copied_text.is_empty() {
            self.clipboard.set(copied_text);
        }

        if let Some(egui::Pos2 { x, y }) = text_cursor_pos {
            window.set_ime_position(winit::dpi::LogicalPosition { x, y });
        }
    }

    fn set_cursor_icon(&mut self, window: &winit::window::Window, cursor_icon: egui::CursorIcon) {
        // prevent flickering near frame boundary when Windows OS tries to control cursor icon for window resizing
        #[cfg(windows)]
        if self.current_cursor_icon == cursor_icon {
            return;
        }
        self.current_cursor_icon = cursor_icon;

        if let Some(cursor_icon) = translate_cursor(cursor_icon) {
            window.set_cursor_visible(true);

            let is_pointer_in_window = self.pointer_pos_in_points.is_some();
            if is_pointer_in_window {
                window.set_cursor_icon(cursor_icon);
            }
        } else {
            window.set_cursor_visible(false);
        }
    }
}

fn open_url_in_browser(_url: &str) {
    #[cfg(feature = "webbrowser")]
    if let Err(err) = webbrowser::open(_url) {
        tracing::warn!("Failed to open url: {}", err);
    }

    #[cfg(not(feature = "webbrowser"))]
    {
        tracing::warn!("Cannot open url - feature \"links\" not enabled.");
    }
}

/// Winit sends special keys (backspace, delete, F1, …) as characters.
/// Ignore those.
/// We also ignore '\r', '\n', '\t'.
/// Newlines are handled by the `Key::Enter` event.
fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = '\u{e000}' <= chr && chr <= '\u{f8ff}'
        || '\u{f0000}' <= chr && chr <= '\u{ffffd}'
        || '\u{100000}' <= chr && chr <= '\u{10fffd}';

    !is_in_private_use_area && !chr.is_ascii_control()
}

fn is_cut_command(modifiers: egui::Modifiers, keycode: winit::keyboard::KeyCode) -> bool {
    (modifiers.command && keycode == winit::keyboard::KeyCode::KeyX)
        || (cfg!(target_os = "windows")
            && modifiers.shift
            && keycode == winit::keyboard::KeyCode::Delete)
}

fn is_copy_command(modifiers: egui::Modifiers, keycode: winit::keyboard::KeyCode) -> bool {
    (modifiers.command && keycode == winit::keyboard::KeyCode::KeyC)
        || (cfg!(target_os = "windows")
            && modifiers.ctrl
            && keycode == winit::keyboard::KeyCode::Insert)
}

fn is_paste_command(modifiers: egui::Modifiers, keycode: winit::keyboard::KeyCode) -> bool {
    (modifiers.command && keycode == winit::keyboard::KeyCode::KeyV)
        || (cfg!(target_os = "windows")
            && modifiers.shift
            && keycode == winit::keyboard::KeyCode::Insert)
}

fn translate_mouse_button(button: winit::event::MouseButton) -> Option<egui::PointerButton> {
    match button {
        winit::event::MouseButton::Left => Some(egui::PointerButton::Primary),
        winit::event::MouseButton::Right => Some(egui::PointerButton::Secondary),
        winit::event::MouseButton::Middle => Some(egui::PointerButton::Middle),
        winit::event::MouseButton::Other(1) => Some(egui::PointerButton::Extra1),
        winit::event::MouseButton::Other(2) => Some(egui::PointerButton::Extra2),
        winit::event::MouseButton::Other(_) => None,
        _ => unreachable!(),
    }
}

fn translate_virtual_key_code(key: winit::keyboard::KeyCode) -> Option<egui::Key> {
    use egui::Key;
    use winit::keyboard::KeyCode;

    Some(match key {
        KeyCode::ArrowDown => Key::ArrowDown,
        KeyCode::ArrowLeft => Key::ArrowLeft,
        KeyCode::ArrowRight => Key::ArrowRight,
        KeyCode::ArrowUp => Key::ArrowUp,

        KeyCode::Escape => Key::Escape,
        KeyCode::Tab => Key::Tab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter => Key::Enter,
        KeyCode::Space => Key::Space,

        KeyCode::Insert => Key::Insert,
        KeyCode::Delete => Key::Delete,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,

        KeyCode::Digit0 | KeyCode::Numpad0 => Key::Num0,
        KeyCode::Digit1 | KeyCode::Numpad1 => Key::Num1,
        KeyCode::Digit2 | KeyCode::Numpad2 => Key::Num2,
        KeyCode::Digit3 | KeyCode::Numpad3 => Key::Num3,
        KeyCode::Digit4 | KeyCode::Numpad4 => Key::Num4,
        KeyCode::Digit5 | KeyCode::Numpad5 => Key::Num5,
        KeyCode::Digit6 | KeyCode::Numpad6 => Key::Num6,
        KeyCode::Digit7 | KeyCode::Numpad7 => Key::Num7,
        KeyCode::Digit8 | KeyCode::Numpad8 => Key::Num8,
        KeyCode::Digit9 | KeyCode::Numpad9 => Key::Num9,

        KeyCode::KeyA => Key::A,
        KeyCode::KeyB => Key::B,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyD => Key::D,
        KeyCode::KeyE => Key::E,
        KeyCode::KeyF => Key::F,
        KeyCode::KeyG => Key::G,
        KeyCode::KeyH => Key::H,
        KeyCode::KeyI => Key::I,
        KeyCode::KeyJ => Key::J,
        KeyCode::KeyK => Key::K,
        KeyCode::KeyL => Key::L,
        KeyCode::KeyM => Key::M,
        KeyCode::KeyN => Key::N,
        KeyCode::KeyO => Key::O,
        KeyCode::KeyP => Key::P,
        KeyCode::KeyQ => Key::Q,
        KeyCode::KeyR => Key::R,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyT => Key::T,
        KeyCode::KeyU => Key::U,
        KeyCode::KeyV => Key::V,
        KeyCode::KeyW => Key::W,
        KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y,
        KeyCode::KeyZ => Key::Z,

        KeyCode::F1 => Key::F1,
        KeyCode::F2 => Key::F2,
        KeyCode::F3 => Key::F3,
        KeyCode::F4 => Key::F4,
        KeyCode::F5 => Key::F5,
        KeyCode::F6 => Key::F6,
        KeyCode::F7 => Key::F7,
        KeyCode::F8 => Key::F8,
        KeyCode::F9 => Key::F9,
        KeyCode::F10 => Key::F10,
        KeyCode::F11 => Key::F11,
        KeyCode::F12 => Key::F12,
        KeyCode::F13 => Key::F13,
        KeyCode::F14 => Key::F14,
        KeyCode::F15 => Key::F15,
        KeyCode::F16 => Key::F16,
        KeyCode::F17 => Key::F17,
        KeyCode::F18 => Key::F18,
        KeyCode::F19 => Key::F19,
        KeyCode::F20 => Key::F20,

        _ => {
            return None;
        }
    })
}

fn translate_cursor(cursor_icon: egui::CursorIcon) -> Option<winit::window::CursorIcon> {
    match cursor_icon {
        egui::CursorIcon::None => None,

        egui::CursorIcon::Alias => Some(winit::window::CursorIcon::Alias),
        egui::CursorIcon::AllScroll => Some(winit::window::CursorIcon::AllScroll),
        egui::CursorIcon::Cell => Some(winit::window::CursorIcon::Cell),
        egui::CursorIcon::ContextMenu => Some(winit::window::CursorIcon::ContextMenu),
        egui::CursorIcon::Copy => Some(winit::window::CursorIcon::Copy),
        egui::CursorIcon::Crosshair => Some(winit::window::CursorIcon::Crosshair),
        egui::CursorIcon::Default => Some(winit::window::CursorIcon::Default),
        egui::CursorIcon::Grab => Some(winit::window::CursorIcon::Grab),
        egui::CursorIcon::Grabbing => Some(winit::window::CursorIcon::Grabbing),
        egui::CursorIcon::Help => Some(winit::window::CursorIcon::Help),
        egui::CursorIcon::Move => Some(winit::window::CursorIcon::Move),
        egui::CursorIcon::NoDrop => Some(winit::window::CursorIcon::NoDrop),
        egui::CursorIcon::NotAllowed => Some(winit::window::CursorIcon::NotAllowed),
        egui::CursorIcon::PointingHand => Some(winit::window::CursorIcon::Hand),
        egui::CursorIcon::Progress => Some(winit::window::CursorIcon::Progress),

        egui::CursorIcon::ResizeHorizontal => Some(winit::window::CursorIcon::EwResize),
        egui::CursorIcon::ResizeNeSw => Some(winit::window::CursorIcon::NeswResize),
        egui::CursorIcon::ResizeNwSe => Some(winit::window::CursorIcon::NwseResize),
        egui::CursorIcon::ResizeVertical => Some(winit::window::CursorIcon::NsResize),

        egui::CursorIcon::ResizeEast => Some(winit::window::CursorIcon::EResize),
        egui::CursorIcon::ResizeSouthEast => Some(winit::window::CursorIcon::SeResize),
        egui::CursorIcon::ResizeSouth => Some(winit::window::CursorIcon::SResize),
        egui::CursorIcon::ResizeSouthWest => Some(winit::window::CursorIcon::SwResize),
        egui::CursorIcon::ResizeWest => Some(winit::window::CursorIcon::WResize),
        egui::CursorIcon::ResizeNorthWest => Some(winit::window::CursorIcon::NwResize),
        egui::CursorIcon::ResizeNorth => Some(winit::window::CursorIcon::NResize),
        egui::CursorIcon::ResizeNorthEast => Some(winit::window::CursorIcon::NeResize),
        egui::CursorIcon::ResizeColumn => Some(winit::window::CursorIcon::ColResize),
        egui::CursorIcon::ResizeRow => Some(winit::window::CursorIcon::RowResize),

        egui::CursorIcon::Text => Some(winit::window::CursorIcon::Text),
        egui::CursorIcon::VerticalText => Some(winit::window::CursorIcon::VerticalText),
        egui::CursorIcon::Wait => Some(winit::window::CursorIcon::Wait),
        egui::CursorIcon::ZoomIn => Some(winit::window::CursorIcon::ZoomIn),
        egui::CursorIcon::ZoomOut => Some(winit::window::CursorIcon::ZoomOut),
    }
}

/// Returns a Wayland display handle if the target is running Wayland
fn wayland_display<T>(_event_loop: &EventLoopWindowTarget<T>) -> Option<*mut c_void> {
    // TODO add wayland display
    // #[cfg(any(
    //     target_os = "linux",
    //     target_os = "dragonfly",
    //     target_os = "freebsd",
    //     target_os = "netbsd",
    //     target_os = "openbsd"
    // ))]
    // {
    //     return _event_loop.wayland_display();
    // }

    #[allow(unreachable_code)]
    {
        let _ = _event_loop;
        None
    }
}

// ---------------------------------------------------------------------------

/// Profiling macro for feature "puffin"
#[allow(unused_macros)]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(feature = "puffin")]
        puffin::profile_function!($($arg)*);
    };
}

#[allow(unused_imports)]
pub(crate) use profile_function;

/// Profiling macro for feature "puffin"
#[allow(unused_macros)]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(feature = "puffin")]
        puffin::profile_scope!($($arg)*);
    };
}

#[allow(unused_imports)]
pub(crate) use profile_scope;
