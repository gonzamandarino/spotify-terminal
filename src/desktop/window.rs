//! Ventana nativa (winit) dibujada por CPU: egui arma la imagen,
//! `egui_software_backend` la rasteriza y `softbuffer` la muestra con GDI.
//! Sin OpenGL ni DirectX: el driver de la placa de video no se carga
//! (con OpenGL la ventana sola ocupaba ~120 MB; ver `docs/decisiones.md`).
//!
//! Se redibuja solo ante eventos (teclado, mouse, tamaño) o cuando alguien
//! pide un repintado (el motor al mandar algo, la barra de progreso): en
//! reposo el hilo queda dormido.

use std::{
    num::NonZeroU32,
    rc::Rc,
    time::{Duration, Instant},
};

use egui::{ResizeDirection, ViewportCommand, ViewportId};
use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{self, Icon, Window, WindowId},
};

use crate::{config, error::AppError};

/// Lo que se dibuja adentro de la ventana.
pub(super) trait Content {
    fn ui(&mut self, ui: &mut egui::Ui);
}

/// Pedido de repintado desde cualquier hilo (el callback de egui).
#[derive(Debug)]
struct Repaint(Duration);

/// Abre la ventana y atiende eventos hasta que se cierra.
///
/// - Pre: `ctx` con fuentes y estilo ya instalados.
/// - Post: la ventana se cerró (botón, Alt+F4 o `ViewportCommand::Close`).
/// - Errores: no se pudo crear la ventana o la superficie de dibujo →
///   `Internal` con el motivo.
pub(super) fn run(ctx: egui::Context, content: &mut impl Content) -> Result<(), AppError> {
    let event_loop = EventLoop::<Repaint>::with_user_event()
        .build()
        .map_err(|e| internal("no se pudo iniciar la ventana", e))?;
    let proxy: EventLoopProxy<Repaint> = event_loop.create_proxy();
    ctx.set_request_repaint_callback(move |info| {
        let _ = proxy.send_event(Repaint(info.delay));
    });
    let mut runner = Runner {
        ctx,
        content,
        renderer: EguiSoftwareRender::new(ColorFieldOrder::Bgra),
        gui: None,
        next_repaint: None,
        error: None,
    };
    event_loop
        .run_app(&mut runner)
        .map_err(|e| internal("error en la ventana", e))?;
    match runner.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

fn internal(context: &str, error: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("{context}: {error}"))
}

/// Ventana creada y su superficie de dibujo.
struct Gui {
    // `surface` antes que `_context`: se sueltan en ese orden.
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    _context: softbuffer::Context<Rc<Window>>,
    state: egui_winit::State,
    window: Rc<Window>,
}

struct Runner<'a, C: Content> {
    ctx: egui::Context,
    content: &'a mut C,
    renderer: EguiSoftwareRender,
    gui: Option<Gui>,
    /// Próximo repintado pedido con demora (p. ej. la barra de progreso).
    next_repaint: Option<Instant>,
    error: Option<AppError>,
}

impl<C: Content> ApplicationHandler<Repaint> for Runner<'_, C> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gui.is_some() {
            return;
        }
        match create_gui(event_loop, &self.ctx) {
            Ok(gui) => self.gui = Some(gui),
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(gui) = &mut self.gui else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.paint(event_loop) {
                    self.error = Some(e);
                    event_loop.exit();
                }
            }
            other => {
                if gui.state.on_window_event(&gui.window, &other).repaint {
                    gui.window.request_redraw();
                }
            }
        }
    }

    fn user_event(&mut self, _: &ActiveEventLoop, Repaint(delay): Repaint) {
        let Some(gui) = &self.gui else {
            return;
        };
        if delay.is_zero() {
            gui.window.request_redraw();
        } else if let Some(at) = Instant::now().checked_add(delay) {
            self.next_repaint = Some(self.next_repaint.map_or(at, |t| t.min(at)));
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.next_repaint {
            Some(at) if at <= Instant::now() => {
                self.next_repaint = None;
                if let Some(gui) = &self.gui {
                    gui.window.request_redraw();
                }
                event_loop.set_control_flow(ControlFlow::Wait);
            }
            Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}

impl<C: Content> Runner<'_, C> {
    fn paint(&mut self, event_loop: &ActiveEventLoop) -> Result<(), AppError> {
        let Some(gui) = &mut self.gui else {
            return Ok(());
        };
        let size = gui.window.inner_size();
        // Minimizada en Windows mide 0x0: no hay nada que dibujar.
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(());
        };

        let mut raw_input = gui.state.take_egui_input(&gui.window);
        let info = raw_input.viewports.entry(ViewportId::ROOT).or_default();
        egui_winit::update_viewport_info(info, &self.ctx, &gui.window, false);
        let content = &mut *self.content;
        let output = self.ctx.run_ui(raw_input, |ui| content.ui(ui));
        gui.state
            .handle_platform_output(&gui.window, output.platform_output);
        if let Some(viewport) = output.viewport_output.get(&ViewportId::ROOT) {
            for command in &viewport.commands {
                apply(command, &gui.window, event_loop);
            }
        }

        let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        gui.surface
            .resize(width, height)
            .map_err(|e| internal("superficie de dibujo", e))?;
        let mut buffer = gui
            .surface
            .buffer_mut()
            .map_err(|e| internal("superficie de dibujo", e))?;
        let mut target = BufferMutRef::new(
            bytemuck::cast_slice_mut(&mut buffer),
            width.get() as usize,
            height.get() as usize,
        );
        self.renderer.render(
            &mut target,
            &primitives,
            &output.textures_delta,
            output.pixels_per_point,
        );
        buffer
            .present()
            .map_err(|e| internal("superficie de dibujo", e))
    }
}

/// Los pedidos de la UI a la ventana que usa la app (barra de título
/// propia). El resto no hace falta y se ignora.
fn apply(command: &ViewportCommand, window: &Window, event_loop: &ActiveEventLoop) {
    match command {
        ViewportCommand::Close => event_loop.exit(),
        ViewportCommand::StartDrag => {
            let _ = window.drag_window();
        }
        ViewportCommand::BeginResize(direction) => {
            let _ = window.drag_resize_window(resize_direction(*direction));
        }
        ViewportCommand::Minimized(on) => window.set_minimized(*on),
        ViewportCommand::Maximized(on) => window.set_maximized(*on),
        _ => {}
    }
}

fn resize_direction(direction: ResizeDirection) -> window::ResizeDirection {
    match direction {
        ResizeDirection::North => window::ResizeDirection::North,
        ResizeDirection::South => window::ResizeDirection::South,
        ResizeDirection::East => window::ResizeDirection::East,
        ResizeDirection::West => window::ResizeDirection::West,
        ResizeDirection::NorthEast => window::ResizeDirection::NorthEast,
        ResizeDirection::SouthEast => window::ResizeDirection::SouthEast,
        ResizeDirection::NorthWest => window::ResizeDirection::NorthWest,
        ResizeDirection::SouthWest => window::ResizeDirection::SouthWest,
    }
}

fn create_gui(event_loop: &ActiveEventLoop, ctx: &egui::Context) -> Result<Gui, AppError> {
    let [width, height] = config::WINDOW_SIZE;
    let [min_width, min_height] = config::WINDOW_MIN_SIZE;
    let attributes = Window::default_attributes()
        .with_title(config::WINDOW_TITLE)
        .with_inner_size(LogicalSize::new(width, height))
        .with_min_inner_size(LogicalSize::new(min_width, min_height))
        .with_decorations(false)
        .with_window_icon(icon());
    let window = Rc::new(
        event_loop
            .create_window(attributes)
            .map_err(|e| internal("no se pudo crear la ventana", e))?,
    );
    let context = softbuffer::Context::new(window.clone())
        .map_err(|e| internal("superficie de dibujo", e))?;
    let surface = softbuffer::Surface::new(&context, window.clone())
        .map_err(|e| internal("superficie de dibujo", e))?;
    let state = egui_winit::State::new(
        ctx.clone(),
        ViewportId::ROOT,
        &*window,
        Some(window.scale_factor() as f32),
        None,
        None,
    );
    Ok(Gui {
        surface,
        _context: context,
        state,
        window,
    })
}

/// Ícono de la ventana (64x64 RGBA, generado por `scripts/generar-icono.py`).
/// Si no se puede usar, la ventana queda con el ícono por defecto.
fn icon() -> Option<Icon> {
    const SIDE: u32 = 64;
    let rgba = include_bytes!("../../assets/icon-64.rgba").to_vec();
    Icon::from_rgba(rgba, SIDE, SIDE).ok()
}
