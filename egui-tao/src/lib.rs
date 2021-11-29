//! [`egui`] bindings for [`tao`](https://github.com/rust-windowing/tao).
//!
//! The library translates tao events to egui, handled copy/paste,
//! updates the cursor, open links clicked in egui, etc.

#![forbid(unsafe_code)]
#![warn(
    clippy::all,
    clippy::await_holding_lock,
    clippy::char_lit_as_u8,
    clippy::checked_conversions,
    clippy::dbg_macro,
    clippy::debug_assert_with_mut_call,
    clippy::disallowed_method,
    clippy::doc_markdown,
    clippy::empty_enum,
    clippy::enum_glob_use,
    clippy::exit,
    clippy::expl_impl_clone_on_copy,
    clippy::explicit_deref_methods,
    clippy::explicit_into_iter_loop,
    clippy::fallible_impl_from,
    clippy::filter_map_next,
    clippy::flat_map_option,
    clippy::float_cmp_const,
    clippy::fn_params_excessive_bools,
    clippy::from_iter_instead_of_collect,
    clippy::if_let_mutex,
    clippy::implicit_clone,
    clippy::imprecise_flops,
    clippy::inefficient_to_string,
    clippy::invalid_upcast_comparisons,
    clippy::large_digit_groups,
    clippy::large_stack_arrays,
    clippy::large_types_passed_by_value,
    clippy::let_unit_value,
    clippy::linkedlist,
    clippy::lossy_float_literal,
    clippy::macro_use_imports,
    clippy::manual_ok_or,
    clippy::map_err_ignore,
    clippy::map_flatten,
    clippy::map_unwrap_or,
    clippy::match_on_vec_items,
    clippy::match_same_arms,
    clippy::match_wild_err_arm,
    clippy::match_wildcard_for_single_variants,
    clippy::mem_forget,
    clippy::mismatched_target_os,
    clippy::missing_errors_doc,
    clippy::missing_safety_doc,
    clippy::mut_mut,
    clippy::mutex_integer,
    clippy::needless_borrow,
    clippy::needless_continue,
    clippy::needless_for_each,
    clippy::needless_pass_by_value,
    clippy::option_option,
    clippy::path_buf_push_overwrite,
    clippy::ptr_as_ptr,
    clippy::ref_option_ref,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::same_functions_in_if_condition,
    clippy::semicolon_if_nothing_returned,
    clippy::single_match_else,
    clippy::string_add_assign,
    clippy::string_add,
    clippy::string_lit_as_bytes,
    clippy::string_to_string,
    clippy::todo,
    clippy::trait_duplication_in_bounds,
    clippy::unimplemented,
    clippy::unnested_or_patterns,
    clippy::unused_self,
    clippy::useless_transmute,
    clippy::verbose_file_reads,
    clippy::zero_sized_map_values,
    future_incompatible,
    missing_crate_level_docs,
    nonstandard_style,
    rust_2018_idioms
)]
#![allow(clippy::float_cmp)]
#![allow(clippy::manual_range_contains)]

pub use tao;

pub mod clipboard;
pub mod screen_reader;
mod window_settings;

#[cfg(feature = "epi")]
pub mod epi;

pub use window_settings::WindowSettings;

pub fn native_pixels_per_point(window: &tao::window::Window) -> f32 {
    window.scale_factor() as f32
}

pub fn screen_size_in_pixels(window: &tao::window::Window) -> egui::Vec2 {
    let size = window.inner_size();
    egui::vec2(size.width as f32, size.height as f32)
}

/// Handles the integration between egui and tao.
pub struct State {
    start_time: std::time::Instant,
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
    /// Initialize with the native `pixels_per_point` (dpi scaling).
    pub fn new(window: &tao::window::Window) -> Self {
        Self::from_pixels_per_point(native_pixels_per_point(window))
    }

    /// Initialize with a given dpi scaling.
    pub fn from_pixels_per_point(pixels_per_point: f32) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            egui_input: egui::RawInput {
                pixels_per_point: Some(pixels_per_point),
                ..Default::default()
            },
            pointer_pos_in_points: None,
            any_pointer_button_down: false,
            current_cursor_icon: egui::CursorIcon::Default,
            current_pixels_per_point: pixels_per_point,

            clipboard: Default::default(),
            screen_reader: screen_reader::ScreenReader::default(),

            simulate_touch_screen: false,
            pointer_touch_id: None,
        }
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
    pub fn take_egui_input(&mut self, window: &tao::window::Window) -> egui::RawInput {
        let pixels_per_point = self.pixels_per_point();

        self.egui_input.time = Some(self.start_time.elapsed().as_secs_f64());

        // On Windows, a minimized window will have 0 width and height.
        // See: https://github.com/rust-windowing/tao/issues/208
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
        event: &tao::event::WindowEvent<'_>,
    ) -> bool {
        use tao::event::WindowEvent;
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
                    tao::event::TouchPhase::Started
                    | tao::event::TouchPhase::Ended
                    | tao::event::TouchPhase::Cancelled => egui_ctx.wants_pointer_input(),
                    tao::event::TouchPhase::Moved => egui_ctx.is_using_pointer(),
                    _ => false,
                }
            }
            WindowEvent::ReceivedImeText(_ch) => { // TODO egui doesn't support all unicode yet
                false
                // // On Mac we get here when the user presses Cmd-C (copy), ctrl-W, etc.
                // // We need to ignore these characters that are side-effects of commands.
                // let is_mac_cmd = cfg!(target_os = "macos")
                //     && (self.egui_input.modifiers.ctrl || self.egui_input.modifiers.mac_cmd);
                //
                // if is_printable_char(*ch) && !is_mac_cmd {
                //     self.egui_input
                //         .events
                //         .push(egui::Event::Text(ch.to_string()));
                //     egui_ctx.wants_keyboard_input()
                // } else {
                //     false
                // }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.on_keyboard_input(event);
                egui_ctx.wants_keyboard_input()
                    || event.physical_key == tao::keyboard::KeyCode::Tab
            }
            WindowEvent::Focused(_) => {
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
        state: tao::event::ElementState,
        button: tao::event::MouseButton,
    ) {
        if let Some(pos) = self.pointer_pos_in_points {
            if let Some(button) = translate_mouse_button(button) {
                let pressed = state == tao::event::ElementState::Pressed;

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

    fn on_cursor_moved(&mut self, pos_in_pixels: tao::dpi::PhysicalPosition<f64>) {
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

    fn on_touch(&mut self, touch: &tao::event::Touch) {
        // Emit touch event
        self.egui_input.events.push(egui::Event::Touch {
            device_id: egui::TouchDeviceId(egui::epaint::util::hash(touch.device_id)),
            id: egui::TouchId::from(touch.id),
            phase: match touch.phase {
                tao::event::TouchPhase::Started => egui::TouchPhase::Start,
                tao::event::TouchPhase::Moved => egui::TouchPhase::Move,
                tao::event::TouchPhase::Ended => egui::TouchPhase::End,
                tao::event::TouchPhase::Cancelled => egui::TouchPhase::Cancel,
                _ => unreachable!(),
            },
            pos: egui::pos2(
                touch.location.x as f32 / self.pixels_per_point(),
                touch.location.y as f32 / self.pixels_per_point(),
            ),
            force: match touch.force {
                Some(tao::event::Force::Normalized(force)) => force as f32,
                Some(tao::event::Force::Calibrated {
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
                tao::event::TouchPhase::Started => {
                    self.pointer_touch_id = Some(touch.id);
                    // First move the pointer to the right location
                    self.on_cursor_moved(touch.location);
                    self.on_mouse_button_input(
                        tao::event::ElementState::Pressed,
                        tao::event::MouseButton::Left,
                    );
                }
                tao::event::TouchPhase::Moved => {
                    self.on_cursor_moved(touch.location);
                }
                tao::event::TouchPhase::Ended => {
                    self.pointer_touch_id = None;
                    self.on_mouse_button_input(
                        tao::event::ElementState::Released,
                        tao::event::MouseButton::Left,
                    );
                    // The pointer should vanish completely to not get any
                    // hover effects
                    self.pointer_pos_in_points = None;
                    self.egui_input.events.push(egui::Event::PointerGone);
                }
                tao::event::TouchPhase::Cancelled => {
                    self.pointer_touch_id = None;
                    self.pointer_pos_in_points = None;
                    self.egui_input.events.push(egui::Event::PointerGone);
                }
                _ => unreachable!(),
            }
        }
    }

    fn on_mouse_wheel(&mut self, delta: tao::event::MouseScrollDelta) {
        let mut delta = match delta {
            tao::event::MouseScrollDelta::LineDelta(x, y) => {
                let points_per_scroll_line = 50.0; // Scroll speed decided by consensus: https://github.com/emilk/egui/issues/461
                egui::vec2(x, y) * points_per_scroll_line
            }
            tao::event::MouseScrollDelta::PixelDelta(delta) => {
                egui::vec2(delta.x as f32, delta.y as f32) / self.pixels_per_point()
            }
            _ => unreachable!(),
        };
        if cfg!(target_os = "macos") {
            // This is still buggy in tao despite
            // https://github.com/rust-windowing/tao/issues/1695 being closed
            delta.x *= -1.0;
        }

        if self.egui_input.modifiers.ctrl || self.egui_input.modifiers.command {
            // Treat as zoom instead:
            let factor = (delta.y / 200.0).exp();
            self.egui_input.events.push(egui::Event::Zoom(factor));
        } else {
            self.egui_input.events.push(egui::Event::Scroll(delta));
        }
    }

    fn on_keyboard_input(&mut self, input: &tao::event::KeyEvent) {
        dbg!(&input);
        // TODO fix this
        let pressed = input.state == tao::event::ElementState::Pressed;

        if pressed {
            if input.logical_key == tao::keyboard::Key::Cut {
                self.egui_input.events.push(egui::Event::Cut);
            } else if input.logical_key == tao::keyboard::Key::Copy {
                self.egui_input.events.push(egui::Event::Copy);
            } else if input.logical_key == tao::keyboard::Key::Paste {
                if let Some(contents) = self.clipboard.get() {
                    self.egui_input
                        .events
                        .push(egui::Event::Text(contents.replace("\r\n", "\n")));
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

        if let Some(text) = input.text {
            if let Some(ch) = text.chars().next() {
                if is_printable_char(ch) {
                    self.egui_input
                        .events
                        .push(egui::Event::Text(ch.to_string()));
                }
            }
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
    pub fn handle_output(
        &mut self,
        window: &tao::window::Window,
        egui_ctx: &egui::Context,
        output: egui::Output,
    ) {
        self.current_pixels_per_point = egui_ctx.pixels_per_point(); // someone can have changed it to scale the UI

        if egui_ctx.memory().options.screen_reader {
            self.screen_reader.speak(&output.events_description());
        }

        self.set_cursor_icon(window, output.cursor_icon);

        if let Some(open) = output.open_url {
            open_url(&open.url);
        }

        if !output.copied_text.is_empty() {
            self.clipboard.set(output.copied_text);
        }

        if let Some(egui::Pos2 { x, y }) = output.text_cursor_pos {
            window.set_ime_position(tao::dpi::LogicalPosition { x, y });
        }
    }

    fn set_cursor_icon(&mut self, window: &tao::window::Window, cursor_icon: egui::CursorIcon) {
        // prevent flickering near frame boundary when Windows OS tries to control cursor icon for window resizing
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

fn open_url(_url: &str) {
    #[cfg(feature = "webbrowser")]
    if let Err(err) = webbrowser::open(_url) {
        eprintln!("Failed to open url: {}", err);
    }

    #[cfg(not(feature = "webbrowser"))]
    {
        eprintln!("Cannot open url - feature \"links\" not enabled.");
    }
}

/// tao sends special keys (backspace, delete, F1, …) as characters.
/// Ignore those.
/// We also ignore '\r', '\n', '\t'.
/// Newlines are handled by the `Key::Enter` event.
fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = '\u{e000}' <= chr && chr <= '\u{f8ff}'
        || '\u{f0000}' <= chr && chr <= '\u{ffffd}'
        || '\u{100000}' <= chr && chr <= '\u{10fffd}';

    !is_in_private_use_area && !chr.is_ascii_control()
}

fn translate_mouse_button(button: tao::event::MouseButton) -> Option<egui::PointerButton> {
    match button {
        tao::event::MouseButton::Left => Some(egui::PointerButton::Primary),
        tao::event::MouseButton::Right => Some(egui::PointerButton::Secondary),
        tao::event::MouseButton::Middle => Some(egui::PointerButton::Middle),
        tao::event::MouseButton::Other(_) => None,
        _ => None,
    }
}

fn translate_virtual_key_code(key: tao::keyboard::KeyCode) -> Option<egui::Key> {
    use egui::Key;
    use tao::keyboard::KeyCode;

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

        _ => {
            return None;
        }
    })
}

fn translate_cursor(cursor_icon: egui::CursorIcon) -> Option<tao::window::CursorIcon> {
    match cursor_icon {
        egui::CursorIcon::None => None,

        egui::CursorIcon::Alias => Some(tao::window::CursorIcon::Alias),
        egui::CursorIcon::AllScroll => Some(tao::window::CursorIcon::AllScroll),
        egui::CursorIcon::Cell => Some(tao::window::CursorIcon::Cell),
        egui::CursorIcon::ContextMenu => Some(tao::window::CursorIcon::ContextMenu),
        egui::CursorIcon::Copy => Some(tao::window::CursorIcon::Copy),
        egui::CursorIcon::Crosshair => Some(tao::window::CursorIcon::Crosshair),
        egui::CursorIcon::Default => Some(tao::window::CursorIcon::Default),
        egui::CursorIcon::Grab => Some(tao::window::CursorIcon::Grab),
        egui::CursorIcon::Grabbing => Some(tao::window::CursorIcon::Grabbing),
        egui::CursorIcon::Help => Some(tao::window::CursorIcon::Help),
        egui::CursorIcon::Move => Some(tao::window::CursorIcon::Move),
        egui::CursorIcon::NoDrop => Some(tao::window::CursorIcon::NoDrop),
        egui::CursorIcon::NotAllowed => Some(tao::window::CursorIcon::NotAllowed),
        egui::CursorIcon::PointingHand => Some(tao::window::CursorIcon::Hand),
        egui::CursorIcon::Progress => Some(tao::window::CursorIcon::Progress),
        egui::CursorIcon::ResizeHorizontal => Some(tao::window::CursorIcon::EwResize),
        egui::CursorIcon::ResizeNeSw => Some(tao::window::CursorIcon::NeswResize),
        egui::CursorIcon::ResizeNwSe => Some(tao::window::CursorIcon::NwseResize),
        egui::CursorIcon::ResizeVertical => Some(tao::window::CursorIcon::NsResize),
        egui::CursorIcon::Text => Some(tao::window::CursorIcon::Text),
        egui::CursorIcon::VerticalText => Some(tao::window::CursorIcon::VerticalText),
        egui::CursorIcon::Wait => Some(tao::window::CursorIcon::Wait),
        egui::CursorIcon::ZoomIn => Some(tao::window::CursorIcon::ZoomIn),
        egui::CursorIcon::ZoomOut => Some(tao::window::CursorIcon::ZoomOut),
    }
}
