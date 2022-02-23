//use glutin::platform::windows::EventLoopExtWindows;
use crate::*;
use egui_tao::tao;

#[derive(Debug)]
struct RequestRepaintEvent;

struct GlowRepaintSignal(std::sync::Mutex<tao::event_loop::EventLoopProxy<RequestRepaintEvent>>);

impl epi::backend::RepaintSignal for GlowRepaintSignal {
    fn request_repaint(&self) {
        self.0.lock().unwrap().send_event(RequestRepaintEvent).ok();
    }
}

#[allow(unsafe_code)]
fn create_display(
    window_builder: tao::window::WindowBuilder,
    event_loop: &tao::event_loop::EventLoop<RequestRepaintEvent>,
) -> (
    glutin::WindowedContext<glutin::PossiblyCurrent>,
    glow::Context,
) {
    let gl_window = unsafe {
        glutin::ContextBuilder::new()
            .with_depth_buffer(0)
            .with_srgb(true)
            .with_stencil_buffer(0)
            .with_vsync(true)
            .build_windowed(window_builder, event_loop)
            .unwrap()
            .make_current()
            .unwrap()
    };

    let gl = unsafe { glow::Context::from_loader_function(|s| gl_window.get_proc_address(s)) };

    unsafe {
        use glow::HasContext as _;
        gl.enable(glow::FRAMEBUFFER_SRGB);
    }

    (gl_window, gl)
}

// ----------------------------------------------------------------------------

pub use epi::NativeOptions;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU8, Ordering};

/// Run an egui app
#[cfg(not(target_os = "linux"))]
#[allow(unsafe_code)]
pub fn run(app: Box<dyn epi::App>, native_options: &epi::NativeOptions) -> ! {
    let persistence = egui_tao::epi::Persistence::from_app_name(app.name());
    let window_settings = persistence.load_window_settings();
    let window_builder =
        egui_tao::epi::window_builder(native_options, &window_settings).with_title(app.name());
    let event_loop = tao::event_loop::EventLoop::with_user_event();
    let (gl_window, gl) = create_display(window_builder, &event_loop);

    let repaint_signal = std::sync::Arc::new(GlowRepaintSignal(std::sync::Mutex::new(
        event_loop.create_proxy(),
    )));

    let mut painter = crate::Painter::new(&gl, None, "")
        .unwrap_or_else(|error| panic!("some OpenGL error occurred {}\n", error));
    let mut integration = egui_tao::epi::EpiIntegration::new(
        "egui_glow",
        painter.max_texture_side(),
        gl_window.window(),
        repaint_signal,
        persistence,
        app,
    );

    let mut is_focused = true;

    event_loop.run(move |event, _, control_flow| {
        let mut redraw = || {
            if !is_focused {
                // On Mac, a minimized Window uses up all CPU: https://github.com/emilk/egui/issues/325
                // We can't know if we are minimized: https://github.com/rust-windowing/winit/issues/208
                // But we know if we are focused (in foreground). When minimized, we are not focused.
                // However, a user may want an egui with an animation in the background,
                // so we still need to repaint quite fast.
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            let egui::FullOutput {
                platform_output,
                needs_repaint,
                textures_delta,
                shapes,
            } = integration.update(gl_window.window());

            integration.handle_platform_output(gl_window.window(), platform_output);

            let clipped_meshes = integration.egui_ctx.tessellate(shapes);

            // paint:
            {
                let color = integration.app.clear_color();
                unsafe {
                    use glow::HasContext as _;
                    gl.disable(glow::SCISSOR_TEST);
                    gl.clear_color(color[0], color[1], color[2], color[3]);
                    gl.clear(glow::COLOR_BUFFER_BIT);
                }
                painter.paint_and_update_textures(
                    &gl,
                    gl_window.window().inner_size().into(),
                    integration.egui_ctx.pixels_per_point(),
                    clipped_meshes,
                    &textures_delta,
                );

                gl_window.swap_buffers().unwrap();
            }

            {
                *control_flow = if integration.should_quit() {
                    tao::event_loop::ControlFlow::Exit
                } else if needs_repaint {
                    gl_window.window().request_redraw();
                    tao::event_loop::ControlFlow::Poll
                } else {
                    tao::event_loop::ControlFlow::Wait
                };
            }

            integration.maybe_autosave(gl_window.window());
        };

        match event {
            // Platform-dependent event handlers to workaround a winit bug
            // See: https://github.com/rust-windowing/winit/issues/987
            // See: https://github.com/rust-windowing/winit/issues/1619
            tao::event::Event::RedrawEventsCleared if cfg!(windows) => redraw(),
            tao::event::Event::RedrawRequested(_) if !cfg!(windows) => redraw(),

            tao::event::Event::WindowEvent { event, .. } => {
                if let tao::event::WindowEvent::Focused(new_focused) = event {
                    is_focused = new_focused;
                }

                if let tao::event::WindowEvent::Resized(physical_size) = event {
                    gl_window.resize(physical_size);
                }

                integration.on_event(&event);
                if integration.should_quit() {
                    *control_flow = tao::event_loop::ControlFlow::Exit;
                }

                gl_window.window().request_redraw(); // TODO: ask egui if the events warrants a repaint instead
            }
            tao::event::Event::LoopDestroyed => {
                integration.on_exit(gl_window.window());
                painter.destroy(&gl);
            }
            tao::event::Event::UserEvent(RequestRepaintEvent) => {
                gl_window.window().request_redraw();
            }
            _ => (),
        }
    });
}

/// Run an egui app
#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
pub fn run(app: Box<dyn epi::App>, native_options: &epi::NativeOptions) -> ! {
    use glutin::platform::ContextTraitExt;
    use gtk::prelude::*;
    let persistence = egui_tao::epi::Persistence::from_app_name(app.name());
    let window_settings = persistence.load_window_settings();
    let window_builder =
        egui_tao::epi::window_builder(native_options, &window_settings).with_title(app.name());
    let event_loop = glutin::event_loop::EventLoop::with_user_event();
    let (gl_window, gl) = create_display(window_builder, &event_loop);
    let area = unsafe { gl_window.raw_handle() };

    let repaint_signal = std::sync::Arc::new(GlowRepaintSignal(std::sync::Mutex::new(
        event_loop.create_proxy(),
    )));

    let painter = crate::Painter::new(&gl, None, "")
        .map_err(|error| eprintln!("some OpenGL error occurred {}\n", error))
        .unwrap();
    let integration = Rc::new(RefCell::new(egui_tao::epi::EpiIntegration::new(
        "egui_glow",
        painter.max_texture_side(),
        gl_window.window(),
        repaint_signal,
        persistence,
        app,
    )));

    let painter = Rc::new(RefCell::new(painter));
    let render_flow = Rc::new(AtomicU8::new(1));
    let gl_window = Rc::new(gl_window);
    let gl = Rc::new(gl);

    let i = integration.clone();
    let p = painter.clone();
    let r = render_flow.clone();
    let gl_window_ = gl_window.clone();
    let gl_ = gl.clone();
    area.connect_render(move |_, _| {
        let mut integration = i.borrow_mut();
        let mut painter = p.borrow_mut();
        //let (needs_repaint, mut tex_allocation_data, shapes) =
        let egui::FullOutput {
            platform_output,
            needs_repaint,
            textures_delta,
            shapes,
        } = integration.update(gl_window_.window());

        integration.handle_platform_output(gl_window_.window(), platform_output);

        let clipped_meshes = integration.egui_ctx.tessellate(shapes);

        {
            let color = integration.app.clear_color();
            unsafe {
                use glow::HasContext as _;
                gl_.disable(glow::SCISSOR_TEST);
                gl_.clear_color(color[0], color[1], color[2], color[3]);
                gl_.clear(glow::COLOR_BUFFER_BIT);
            }
            painter.paint_and_update_textures(
                &gl_,
                gl_window_.window().inner_size().into(),
                integration.egui_ctx.pixels_per_point(),
                clipped_meshes,
                &textures_delta,
            );

            //gl_window.swap_buffers().unwrap();
        }

        {
            let control_flow = if integration.should_quit() {
                2
            } else if needs_repaint {
                0
            } else {
                1
            };
            r.store(control_flow, Ordering::Relaxed);
        }

        integration.maybe_autosave(gl_window_.window());
        gtk::Inhibit(false)
    });

    event_loop.run(move |event, _, control_flow| {
        let mut integration = integration.borrow_mut();
        let mut painter = painter.borrow_mut();
        //dbg!(&event);
        match event {
            glutin::event::Event::MainEventsCleared => {
                area.queue_render();
                match render_flow.load(Ordering::Relaxed) {
                    0 => *control_flow = glutin::event_loop::ControlFlow::Poll,
                    1 => *control_flow = glutin::event_loop::ControlFlow::Wait,
                    2 => *control_flow = glutin::event_loop::ControlFlow::Exit,
                    _ => unreachable!(),
                }
            }
            glutin::event::Event::WindowEvent { event, .. } => {
                area.queue_render();
                integration.on_event(&event);
                if integration.should_quit() {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                }
            }
            glutin::event::Event::LoopDestroyed => {
                integration.on_exit(gl_window.window());
                painter.destroy(&gl);
            }
            glutin::event::Event::UserEvent(RequestRepaintEvent) => {
                area.queue_render();
            }
            _ => (),
        }
    });
}
