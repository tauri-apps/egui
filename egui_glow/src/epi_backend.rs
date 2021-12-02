//use glutin::platform::windows::EventLoopExtWindows;
use crate::*;
use glutin::platform::*;
use gtk::prelude::*;

#[derive(Debug)]
struct RequestRepaintEvent;

struct GlowRepaintSignal(std::sync::Mutex<glutin::event_loop::EventLoopProxy<RequestRepaintEvent>>);

impl epi::RepaintSignal for GlowRepaintSignal {
    fn request_repaint(&self) {
        self.0.lock().unwrap().send_event(RequestRepaintEvent).ok();
    }
}

#[allow(unsafe_code)]
fn create_display(
    window_builder: glutin::window::WindowBuilder,
    event_loop: &glutin::event_loop::EventLoop<RequestRepaintEvent>,
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
use std::rc::Rc;
use std::cell::RefCell;
use std::ops::DerefMut;
use std::sync::mpsc::sync_channel;

/// Run an egui app
#[allow(unsafe_code)]
pub fn run(app: Box<dyn epi::App>, native_options: &epi::NativeOptions) -> ! {
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

    let mut painter = crate::Painter::new(&gl, None, "")
        .map_err(|error| eprintln!("some OpenGL error occurred {}\n", error))
        .unwrap();
    let integration = Rc::new(RefCell::new(egui_tao::epi::EpiIntegration::new(
        "egui_glow",
        gl_window.window(),
        &mut painter,
        repaint_signal,
        persistence,
        app,
    )));

    let painter = Rc::new(RefCell::new(painter));
    let (tx, rx) = sync_channel(1);
    let i = integration.clone();
    let p = painter.clone();
    area.connect_render(move |_, _| {
        let mut integration = i.borrow_mut();
        let mut painter = p.borrow_mut();
        let (needs_repaint, shapes) = integration.update(gl_window.window(), painter.deref_mut());
        let clipped_meshes = integration.egui_ctx.tessellate(shapes);

        {
            let color = integration.app.clear_color();
            unsafe {
                use glow::HasContext as _;
                gl.disable(glow::SCISSOR_TEST);
                gl.clear_color(color[0], color[1], color[2], color[3]);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
            painter.upload_egui_texture(&gl, &integration.egui_ctx.texture());
            painter.paint_meshes(
                gl_window.window().inner_size().into(),
                &gl,
                integration.egui_ctx.pixels_per_point(),
                clipped_meshes,
            );

        }

        {
            let control_flow = if integration.should_quit() {
                glutin::event_loop::ControlFlow::Exit
            } else if needs_repaint {
                glutin::event_loop::ControlFlow::Poll
            } else {
                glutin::event_loop::ControlFlow::Wait
            };
            tx.send(control_flow).unwrap();
        }

        integration.maybe_autosave(gl_window.window());
        gtk::Inhibit(false)
    });

    event_loop.run(move |event, _, control_flow| {
        let mut integration = integration.borrow_mut();
        let painter = painter.borrow_mut();
        dbg!(&event);
        match event {
            glutin::event::Event::MainEventsCleared => {
                area.queue_render();
                while let Ok(c) = rx.try_recv() {
                    *control_flow = c;
                }
            },
            glutin::event::Event::WindowEvent { event, .. } => {
                area.queue_render();
                integration.on_event(&event);
                if integration.should_quit() {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                }
            }
            glutin::event::Event::LoopDestroyed => {
                // TODO
                //integration.on_exit(gl_window.window());
                //painter.destroy(&gl);
            }
            glutin::event::Event::UserEvent(RequestRepaintEvent) => {
                area.queue_render();
            }
            _ => (),
        }
    });
}
