//! Consola de la app de escritorio: barra de título propia, historial de
//! salida, línea de entrada y barra de "sonando ahora". No sabe nada de
//! Spotify: manda lo escrito al motor (`app::engine`) y muestra lo que
//! vuelve.

use std::{collections::VecDeque, sync::Arc, sync::mpsc, time::Duration};

use egui::{
    Align2, CentralPanel, Color32, CursorIcon, FontId, Frame, Galley, Id, Key, Margin, Modifiers,
    Panel, Pos2, Rect, ResizeDirection, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit,
    Ui, Vec2, ViewportCommand,
    text::{CCursor, CCursorRange, LayoutJob, TextWrapping},
};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    theme::{bold, color, mono},
    window::Content,
};
use crate::{
    app::{
        engine::{Input, LineKind, NowPlaying, Output, Prompt},
        queue::PlayState,
        shell,
    },
    config::{self, theme as colors},
};

const TITLE_HEIGHT: f32 = 36.0;
const INPUT_HEIGHT: f32 = 34.0;
const NOW_HEIGHT: f32 = 64.0;
/// Ancho del borde de la ventana que sirve para cambiarle el tamaño.
const GRIP: f32 = 5.0;
const PROMPT: &str = "♫ ›";

/// Una línea de la consola.
enum ConsoleLine {
    /// Lo que escribió el usuario, con el prompt que tenía.
    Echo {
        prompt: String,
        text: String,
    },
    Out(LineKind, String),
}

pub(super) struct DesktopApp {
    inputs: UnboundedSender<Input>,
    outputs: mpsc::Receiver<Output>,
    lines: VecDeque<ConsoleLine>,
    input: String,
    input_id: Id,
    history: VecDeque<String>,
    /// Posición en `history` mientras se recorre con ↑/↓.
    history_pos: Option<usize>,
    /// Lo que se estaba escribiendo antes de empezar a recorrer el historial.
    draft: String,
    now: Option<NowPlaying>,
    prompt: Prompt,
    engine_gone: bool,
}

impl DesktopApp {
    pub(super) fn new(inputs: UnboundedSender<Input>, outputs: mpsc::Receiver<Output>) -> Self {
        let mut app = DesktopApp {
            inputs,
            outputs,
            lines: VecDeque::new(),
            input: String::new(),
            input_id: Id::new("entrada"),
            history: VecDeque::new(),
            history_pos: None,
            draft: String::new(),
            now: None,
            prompt: Prompt::Ready,
            engine_gone: false,
        };
        app.push(ConsoleLine::Out(
            LineKind::Track,
            format!("{PROMPT} {}", config::WINDOW_TITLE),
        ));
        app.push(ConsoleLine::Out(
            LineKind::Dim,
            "Escribí `play <nombre>` para arrancar, o `help` para ver los comandos.".into(),
        ));
        app
    }

    fn push(&mut self, line: ConsoleLine) {
        if self.lines.len() >= config::SCROLLBACK_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    fn send(&mut self, input: Input) {
        if self.inputs.send(input).is_err() && !self.engine_gone {
            self.engine_gone = true;
            self.push(ConsoleLine::Out(
                LineKind::Error,
                "Error interno: el motor se detuvo. Cerrá y abrí la app de nuevo.".into(),
            ));
        }
    }

    /// Todo lo que mandó el motor desde el último frame.
    fn drain(&mut self, ctx: &egui::Context) {
        while let Ok(output) = self.outputs.try_recv() {
            match output {
                Output::Line(kind, text) => self.push(ConsoleLine::Out(kind, text)),
                Output::NowPlaying(now) => self.now = now,
                Output::Prompt(prompt) => self.prompt = prompt,
                Output::Clear => self.lines.clear(),
                Output::Exit => ctx.send_viewport_cmd(ViewportCommand::Close),
            }
        }
    }

    fn prompt_text(&self) -> (String, Color32) {
        match self.prompt {
            Prompt::Ready => (PROMPT.into(), color(colors::ACCENT)),
            Prompt::Choose(n) => (format!("1-{n} ›"), color(colors::WARNING)),
            Prompt::Busy(_) => ("… ›".into(), color(colors::SECONDARY)),
        }
    }

    fn submit(&mut self) {
        let text = std::mem::take(&mut self.input);
        let (prompt, _) = self.prompt_text();
        self.push(ConsoleLine::Echo {
            prompt,
            text: text.clone(),
        });
        let trimmed = text.trim();
        if !trimmed.is_empty() && self.history.back().map(String::as_str) != Some(trimmed) {
            if self.history.len() >= config::HISTORY_LEN {
                self.history.pop_front();
            }
            self.history.push_back(trimmed.to_string());
        }
        self.history_pos = None;
        self.send(Input::Line(text));
    }

    /// Atajos que se atienden antes que la línea de entrada (así Ctrl+→
    /// no mueve el cursor por palabras y Tab no saca el foco).
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let mut pressed = Vec::new();
        ctx.input_mut(|i| {
            for (modifiers, key) in [
                (Modifiers::CTRL, Key::Space),
                (Modifiers::CTRL, Key::ArrowRight),
                (Modifiers::CTRL, Key::ArrowLeft),
                (Modifiers::NONE, Key::Escape),
                (Modifiers::NONE, Key::ArrowUp),
                (Modifiers::NONE, Key::ArrowDown),
                (Modifiers::NONE, Key::Tab),
            ] {
                // Varias pulsaciones pueden llegar en el mismo frame.
                for _ in 0..i.count_and_consume_key(modifiers, key) {
                    pressed.push((modifiers, key));
                }
            }
        });
        for (modifiers, key) in pressed {
            match (modifiers, key) {
                (Modifiers::CTRL, Key::Space) => self.send(Input::TogglePause),
                (Modifiers::CTRL, Key::ArrowRight) => self.send(Input::Next),
                (Modifiers::CTRL, Key::ArrowLeft) => self.send(Input::Prev),
                (_, Key::Escape) => {
                    if self.input.is_empty() {
                        self.send(Input::Cancel);
                    } else {
                        self.input.clear();
                    }
                }
                (_, Key::ArrowUp) => self.browse_history(ctx, true),
                (_, Key::ArrowDown) => self.browse_history(ctx, false),
                (_, Key::Tab) => self.complete(ctx),
                _ => {}
            }
        }
    }

    fn browse_history(&mut self, ctx: &egui::Context, older: bool) {
        if self.history.is_empty() {
            return;
        }
        let last = self.history.len() - 1;
        let next = match (self.history_pos, older) {
            (None, true) => {
                self.draft = self.input.clone();
                Some(last)
            }
            (None, false) => None,
            (Some(pos), true) => Some(pos.saturating_sub(1)),
            (Some(pos), false) if pos < last => Some(pos + 1),
            (Some(_), false) => None,
        };
        self.history_pos = next;
        self.input = match next {
            Some(pos) => self.history[pos].clone(),
            None => std::mem::take(&mut self.draft),
        };
        self.cursor_to_end(ctx);
    }

    fn complete(&mut self, ctx: &egui::Context) {
        match shell::complete(&self.input).as_slice() {
            [] => {}
            [only] => {
                self.input = format!("{only} ");
                self.cursor_to_end(ctx);
            }
            many => {
                let options = many.join("  ");
                self.push(ConsoleLine::Out(LineKind::Dim, options));
            }
        }
    }

    fn cursor_to_end(&self, ctx: &egui::Context) {
        if let Some(mut state) = TextEdit::load_state(ctx, self.input_id) {
            let end = CCursor::new(self.input.chars().count());
            state.cursor.set_char_range(Some(CCursorRange::one(end)));
            state.store(ctx, self.input_id);
        }
    }

    fn title_bar(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        let ctx = ui.ctx().clone();
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let drag = ui.interact(rect, Id::new("barra-titulo"), Sense::click_and_drag());
        if drag.double_clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        } else if drag.drag_started_by(egui::PointerButton::Primary) {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        let painter = ui.painter();
        let mid = rect.center().y;
        let icon = painter.text(
            Pos2::new(rect.left() + 14.0, mid),
            Align2::LEFT_CENTER,
            "♫",
            bold(16.0),
            color(colors::ACCENT),
        );
        painter.text(
            Pos2::new(icon.right() + 8.0, mid),
            Align2::LEFT_CENTER,
            config::WINDOW_TITLE,
            bold(13.0),
            color(colors::TEXT),
        );

        let button = Vec2::new(46.0, rect.height());
        let close = Rect::from_min_size(Pos2::new(rect.right() - button.x, rect.top()), button);
        let max = close.translate(Vec2::new(-button.x, 0.0));
        let min = max.translate(Vec2::new(-button.x, 0.0));
        if title_button(ui, close, "cerrar", TitleIcon::Close).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        let icon = if maximized {
            TitleIcon::Restore
        } else {
            TitleIcon::Maximize
        };
        if title_button(ui, max, "maximizar", icon).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        }
        if title_button(ui, min, "minimizar", TitleIcon::Minimize).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
    }

    fn console(&self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                for line in &self.lines {
                    match line {
                        ConsoleLine::Echo { prompt, text } => {
                            let mut job = LayoutJob::default();
                            job.append(
                                &format!("{prompt} "),
                                0.0,
                                egui::TextFormat::simple(
                                    bold(colors::FONT_SIZE),
                                    color(colors::ACCENT),
                                ),
                            );
                            job.append(
                                text,
                                0.0,
                                egui::TextFormat::simple(
                                    mono(colors::FONT_SIZE),
                                    color(colors::TEXT_STRONG),
                                ),
                            );
                            ui.label(job);
                        }
                        ConsoleLine::Out(kind, text) => {
                            ui.label(RichText::new(text).color(line_color(*kind)));
                        }
                    }
                }
            });
    }

    fn input_row(&mut self, ui: &mut Ui) {
        let (prompt, prompt_color) = self.prompt_text();
        ui.horizontal_centered(|ui| {
            ui.label(
                RichText::new(prompt)
                    .font(bold(colors::FONT_SIZE))
                    .color(prompt_color),
            );
            let busy = match self.prompt {
                Prompt::Busy(what) => Some(what),
                _ => None,
            };
            let hint = match self.prompt {
                Prompt::Ready => "escribí un comando · help",
                Prompt::Choose(_) => "número y Enter · Enter = el primero · Esc cancela",
                Prompt::Busy(_) => "",
            };
            let edit = TextEdit::singleline(&mut self.input)
                .id(self.input_id)
                // Tab no saca el foco: lo usa `complete`.
                .lock_focus(true)
                .frame(Frame::NONE)
                .font(mono(colors::FONT_SIZE))
                .text_color(color(colors::TEXT_STRONG))
                .hint_text(RichText::new(hint).color(color(colors::SECONDARY).gamma_multiply(0.7)))
                .desired_width(ui.available_width() - if busy.is_some() { 260.0 } else { 0.0 });
            let response = ui.add(edit);
            if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                self.submit();
            }
            // La consola es la entrada: el foco vuelve siempre acá.
            response.request_focus();
            if let Some(what) = busy {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("⋯ {what}  (Esc cancela)"))
                            .small()
                            .color(color(colors::SECONDARY)),
                    );
                });
            }
        });
    }

    fn now_bar(&self, ui: &mut Ui) {
        let rect = ui.max_rect().shrink2(Vec2::new(16.0, 0.0));
        let painter = ui.painter();
        let Some(now) = &self.now else {
            painter.text(
                rect.left_center(),
                Align2::LEFT_CENTER,
                "Nada sonando · escribí  play <nombre>",
                mono(13.0),
                color(colors::SECONDARY),
            );
            return;
        };

        // Fila de arriba: estado, tema — artistas, e indicadores a la derecha.
        let top = rect.top() + 20.0;
        let mut chips = Vec::new();
        if !now.position.trim().is_empty() {
            chips.push((now.position.trim().to_string(), color(colors::SECONDARY)));
        }
        if now.queued > 0 {
            chips.push((format!("cola {}", now.queued), color(colors::SECONDARY)));
        }
        let shuffle = if now.shuffle {
            color(colors::ACCENT)
        } else {
            color(colors::SECONDARY).gamma_multiply(0.5)
        };
        chips.push(("🔀".to_string(), shuffle));
        let mut right = rect.right();
        for (text, chip_color) in chips.iter().rev() {
            let r = painter.text(
                Pos2::new(right, top),
                Align2::RIGHT_CENTER,
                text,
                mono(12.0),
                *chip_color,
            );
            right = r.left() - 14.0;
        }

        let (state_icon, state_color) = match now.state {
            PlayState::Loading => ("…", color(colors::SECONDARY)),
            PlayState::Playing => ("▶", color(colors::ACCENT)),
            PlayState::Paused => ("⏸", color(colors::WARNING)),
        };
        let icon = painter.text(
            Pos2::new(rect.left(), top),
            Align2::LEFT_CENTER,
            state_icon,
            bold(14.0),
            state_color,
        );
        let title = if now.title.is_empty() {
            "Cargando…"
        } else {
            now.title.as_str()
        };
        let x = icon.right() + 10.0;
        let max_width = (right - x).max(0.0);
        let title_galley = one_line(ui, title, bold(14.0), color(colors::TEXT_STRONG), max_width);
        let title_width = title_galley.size().x;
        painter.galley(
            Pos2::new(x, top - title_galley.size().y / 2.0),
            title_galley,
            color(colors::TEXT_STRONG),
        );
        if !now.artists.is_empty() {
            let artists = format!("  —  {}", now.artists);
            let galley = one_line(
                ui,
                &artists,
                mono(13.0),
                color(colors::SECONDARY),
                (max_width - title_width).max(0.0),
            );
            painter.galley(
                Pos2::new(x + title_width, top - galley.size().y / 2.0),
                galley,
                color(colors::SECONDARY),
            );
        }

        // Fila de abajo: tiempo, barra de progreso y duración.
        let bottom = rect.bottom() - 18.0;
        let elapsed = now.clock.elapsed().min(now.duration);
        let left = painter.text(
            Pos2::new(rect.left(), bottom),
            Align2::LEFT_CENTER,
            clock_text(elapsed),
            mono(11.0),
            color(colors::SECONDARY),
        );
        let right = painter.text(
            Pos2::new(rect.right(), bottom),
            Align2::RIGHT_CENTER,
            clock_text(now.duration),
            mono(11.0),
            color(colors::SECONDARY),
        );
        let track = Rect::from_min_max(
            Pos2::new(left.right() + 12.0, bottom - 2.0),
            Pos2::new(right.left() - 12.0, bottom + 2.0),
        );
        painter.rect_filled(track, 2.0, color(colors::BORDER));
        if now.duration > Duration::ZERO {
            let fraction = elapsed.as_secs_f32() / now.duration.as_secs_f32();
            let mut done = track;
            done.set_width(track.width() * fraction.clamp(0.0, 1.0));
            painter.rect_filled(done, 2.0, color(colors::ACCENT));
        }
    }

    /// Bordes y esquinas para cambiar el tamaño (la ventana no tiene marco
    /// de Windows). Van al final para quedar por encima del resto.
    fn resize_grips(&self, ui: &mut Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
            return;
        }
        let (l, r, t, b) = (rect.left(), rect.right(), rect.top(), rect.bottom());
        let edges = [
            (
                ResizeDirection::North,
                Rect::from_min_max(Pos2::new(l + GRIP, t), Pos2::new(r - GRIP, t + GRIP)),
                CursorIcon::ResizeVertical,
            ),
            (
                ResizeDirection::South,
                Rect::from_min_max(Pos2::new(l + GRIP, b - GRIP), Pos2::new(r - GRIP, b)),
                CursorIcon::ResizeVertical,
            ),
            (
                ResizeDirection::West,
                Rect::from_min_max(Pos2::new(l, t + GRIP), Pos2::new(l + GRIP, b - GRIP)),
                CursorIcon::ResizeHorizontal,
            ),
            (
                ResizeDirection::East,
                Rect::from_min_max(Pos2::new(r - GRIP, t + GRIP), Pos2::new(r, b - GRIP)),
                CursorIcon::ResizeHorizontal,
            ),
            (
                ResizeDirection::NorthWest,
                Rect::from_min_size(Pos2::new(l, t), Vec2::splat(GRIP)),
                CursorIcon::ResizeNwSe,
            ),
            (
                ResizeDirection::SouthEast,
                Rect::from_min_size(Pos2::new(r - GRIP, b - GRIP), Vec2::splat(GRIP)),
                CursorIcon::ResizeNwSe,
            ),
            (
                ResizeDirection::NorthEast,
                Rect::from_min_size(Pos2::new(r - GRIP, t), Vec2::splat(GRIP)),
                CursorIcon::ResizeNeSw,
            ),
            (
                ResizeDirection::SouthWest,
                Rect::from_min_size(Pos2::new(l, b - GRIP), Vec2::splat(GRIP)),
                CursorIcon::ResizeNeSw,
            ),
        ];
        for (i, (direction, area, cursor)) in edges.into_iter().enumerate() {
            let response = ui.interact(area, Id::new(("borde", i)), Sense::drag());
            if response.hovered() || response.dragged() {
                ctx.set_cursor_icon(cursor);
            }
            if response.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
            }
        }
    }
}

impl Content for DesktopApp {
    fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.drain(&ctx);
        self.shortcuts(&ctx);

        let rect = ui.max_rect();
        ui.painter()
            .rect_filled(rect, 0.0, color(colors::BACKGROUND));

        Panel::top("titulo")
            .exact_size(TITLE_HEIGHT)
            .frame(Frame::new().fill(color(colors::PANEL)))
            .show_inside(ui, |ui| self.title_bar(ui));
        Panel::bottom("sonando")
            .exact_size(NOW_HEIGHT)
            .frame(
                Frame::new()
                    .fill(color(colors::PANEL))
                    .stroke(Stroke::new(1.0_f32, color(colors::BORDER))),
            )
            .show_inside(ui, |ui| self.now_bar(ui));
        Panel::bottom("entrada")
            .exact_size(INPUT_HEIGHT)
            .frame(Frame::new().inner_margin(Margin::symmetric(14, 0)))
            .show_inside(ui, |ui| self.input_row(ui));
        CentralPanel::default()
            .frame(Frame::new().inner_margin(Margin {
                left: 14,
                right: 6,
                top: 10,
                bottom: 4,
            }))
            .show_inside(ui, |ui| self.console(ui));

        ui.painter().rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0_f32, color(colors::BORDER)),
            StrokeKind::Inside,
        );
        self.resize_grips(ui, rect);

        // La barra de progreso avanza sola solo mientras suena.
        if self
            .now
            .as_ref()
            .is_some_and(|now| now.state == PlayState::Playing)
        {
            ctx.request_repaint_after(config::PROGRESS_REPAINT);
        }
    }
}

fn line_color(kind: LineKind) -> Color32 {
    match kind {
        LineKind::Normal => color(colors::TEXT),
        LineKind::Track => color(colors::ACCENT),
        LineKind::Ok => color(colors::ACCENT),
        LineKind::Warn => color(colors::WARNING),
        LineKind::Error => color(colors::ERROR),
        LineKind::Dim => color(colors::SECONDARY),
    }
}

/// "m:ss".
fn clock_text(time: Duration) -> String {
    let secs = time.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Texto en una línea, cortado con "…" si no entra en `max_width`.
fn one_line(ui: &Ui, text: &str, font: FontId, text_color: Color32, max_width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::simple_singleline(text.to_string(), font, text_color);
    job.wrap = TextWrapping {
        max_width,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

#[derive(Clone, Copy)]
enum TitleIcon {
    Minimize,
    Maximize,
    Restore,
    Close,
}

/// Botón de la barra de título, dibujado con líneas (no depende de que la
/// fuente tenga los símbolos).
fn title_button(ui: &mut Ui, rect: Rect, id: &str, icon: TitleIcon) -> egui::Response {
    let response = ui.interact(rect, Id::new(("boton-titulo", id)), Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        let fill = match icon {
            TitleIcon::Close => color(colors::CLOSE_HOVER),
            _ => color(colors::BORDER),
        };
        painter.rect_filled(rect, 0.0, fill);
    }
    let stroke = Stroke::new(1.2_f32, color(colors::TEXT));
    let c = rect.center();
    let s = 5.0;
    match icon {
        TitleIcon::Minimize => {
            painter.line_segment([Pos2::new(c.x - s, c.y), Pos2::new(c.x + s, c.y)], stroke);
        }
        TitleIcon::Maximize => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(2.0 * s)),
                0.0,
                stroke,
                StrokeKind::Middle,
            );
        }
        TitleIcon::Restore => {
            let back = Rect::from_center_size(c + Vec2::new(2.0, -2.0), Vec2::splat(1.6 * s));
            let front = Rect::from_center_size(c + Vec2::new(-1.5, 1.5), Vec2::splat(1.6 * s));
            painter.rect_stroke(back, 0.0, stroke, StrokeKind::Middle);
            painter.rect_filled(front, 0.0, color(colors::PANEL));
            painter.rect_stroke(front, 0.0, stroke, StrokeKind::Middle);
        }
        TitleIcon::Close => {
            painter.line_segment([c + Vec2::new(-s, -s), c + Vec2::new(s, s)], stroke);
            painter.line_segment([c + Vec2::new(-s, s), c + Vec2::new(s, -s)], stroke);
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reloj_en_minutos_y_segundos() {
        assert_eq!(clock_text(Duration::ZERO), "0:00");
        assert_eq!(clock_text(Duration::from_millis(83_900)), "1:23");
        assert_eq!(clock_text(Duration::from_secs(3_605)), "60:05");
    }

    fn app() -> (DesktopApp, tokio::sync::mpsc::UnboundedReceiver<Input>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (_out_tx, out_rx) = mpsc::channel();
        (DesktopApp::new(tx, out_rx), rx)
    }

    #[test]
    fn enter_manda_la_linea_y_la_guarda_en_el_historial() {
        let (mut app, mut rx) = app();
        let ctx = egui::Context::default();
        for text in ["play algo", "play algo", "  ", "next"] {
            app.input = text.into();
            app.submit();
        }
        assert_eq!(app.history, ["play algo", "next"]);
        assert_eq!(rx.try_recv().ok(), Some(Input::Line("play algo".into())));

        app.input = "a medio".into();
        app.browse_history(&ctx, true);
        assert_eq!(app.input, "next");
        app.browse_history(&ctx, true);
        app.browse_history(&ctx, true);
        assert_eq!(app.input, "play algo");
        app.browse_history(&ctx, false);
        assert_eq!(app.input, "next");
        app.browse_history(&ctx, false);
        assert_eq!(
            app.input, "a medio",
            "vuelve a lo que se estaba escribiendo"
        );
    }

    #[test]
    fn el_scrollback_tiene_tope() {
        let (mut app, _rx) = app();
        for i in 0..config::SCROLLBACK_LINES + 10 {
            app.push(ConsoleLine::Out(LineKind::Normal, i.to_string()));
        }
        assert_eq!(app.lines.len(), config::SCROLLBACK_LINES);
    }

    #[test]
    fn tab_completa_o_muestra_opciones() {
        let (mut app, _rx) = app();
        let ctx = egui::Context::default();
        app.input = "who".into();
        app.complete(&ctx);
        assert_eq!(app.input, "whoami ");
        app.input = "log".into();
        let before = app.lines.len();
        app.complete(&ctx);
        assert_eq!(app.input, "log");
        assert_eq!(app.lines.len(), before + 1);
    }
}
