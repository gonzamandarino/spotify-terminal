//! Consola de la app de escritorio: barra de título propia, renglón de
//! menús (spec 007), historial de salida, línea de entrada y barra de
//! "sonando ahora". No sabe nada de Spotify: manda lo escrito al motor
//! (`app::engine`) y muestra lo que vuelve.
//!
//! Es la dueña de los ajustes vivos (`settings::Settings`): los menús y los
//! atajos los cambian, y al final de cada frame se aplica lo que cambió
//! (tema, atajos globales, reproducción...) y se agenda el guardado.

use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::Arc,
    sync::mpsc,
    time::{Duration, Instant},
};

use egui::{
    Align, Align2, CentralPanel, Color32, CursorIcon, FontId, Frame, Galley, Id, Key, Layout,
    Margin, Modifiers, Panel, Pos2, Rect, ResizeDirection, RichText, ScrollArea, Sense, Shape,
    Stroke, StrokeKind, TextEdit, Ui, UiBuilder, Vec2, ViewportCommand, WindowLevel,
    text::{CCursor, CCursorRange, LayoutJob, TextWrapping},
};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    combo,
    hotkeys::{self, Hotkeys},
    layout,
    menu::{self, Command, Menus},
    settings::{self, Geometry, Palette, Settings, Target, WindowAction},
    theme::{Theme, bold, color, installed_fonts, mono},
    viz::{Track, Visualizer},
    window::Content,
};
use crate::{
    app::{
        engine::{GlobalAction, Input, LineKind, NowPlaying, Output, Prompt},
        queue::PlayState,
        shell,
        volume::Volume,
    },
    config::{
        self,
        layout::{
            CONSOLE_MARGIN_BOTTOM, CONSOLE_MARGIN_TOP, MENU_HEIGHT, PLAYER_BUTTON,
            PLAYER_BUTTON_GAP, PLAYER_SPACE, TITLE_HEIGHT,
        },
        theme::FONT_SIZE,
    },
    spotify::tap::AudioTap,
};

/// Espacio entre renglones de la consola.
const CONSOLE_ROW_GAP: f32 = 4.0;
/// Margen interno del panel de visualización.
const VIZ_MARGIN: i8 = 12;
/// Ancho del borde de la ventana que sirve para cambiarle el tamaño.
const GRIP: f32 = 5.0;

/// Una línea de la consola.
enum ConsoleLine {
    /// Lo que escribió el usuario, con el prompt que tenía.
    Echo {
        prompt: String,
        text: String,
    },
    Out(LineKind, String),
}

/// Lo que la app necesita al abrir, además de los canales del motor.
pub(super) struct Startup {
    pub(super) settings: Settings,
    /// Avisos de `settings::load`. Si hay, el archivo no se toca hasta que
    /// el usuario cambie algo (para no perder lo que escribió a mano).
    pub(super) settings_warnings: Vec<String>,
    /// Otros avisos del arranque (atajos globales que no se registraron).
    pub(super) warnings: Vec<String>,
    /// Dónde se guardan los ajustes; `None` = no se guardan.
    pub(super) data_dir: Option<PathBuf>,
    pub(super) hotkeys: Option<Hotkeys>,
    /// Con los ajustes ya aplicados.
    pub(super) theme: Theme,
    /// Lo que suena, para la visualización (spec 010).
    pub(super) tap: AudioTap,
}

pub(super) struct DesktopApp {
    inputs: UnboundedSender<Input>,
    outputs: mpsc::Receiver<Output>,
    /// Cada línea con la hora en que llegó ("14:05:09").
    lines: VecDeque<(String, ConsoleLine)>,
    input: String,
    input_id: Id,
    history: VecDeque<String>,
    /// Posición en `history` mientras se recorre con ↑/↓.
    history_pos: Option<usize>,
    /// Lo que se estaba escribiendo antes de empezar a recorrer el historial.
    draft: String,
    now: Option<NowPlaying>,
    /// `None` hasta que el motor lo manda al arrancar.
    volume: Option<Volume>,
    prompt: Prompt,
    engine_gone: bool,

    settings: Settings,
    data_dir: Option<PathBuf>,
    menus: Menus,
    commands: Vec<Command>,
    theme: Theme,
    fonts: Vec<&'static str>,
    hotkeys: Option<Hotkeys>,
    /// Cuándo guardar los ajustes (cambio pendiente).
    save_at: Option<Instant>,
    /// El archivo tuvo avisos al cargarse y el usuario todavía no cambió
    /// nada: no se pisa.
    keep_file: bool,
    started: bool,
    /// Visualización del panel derecho (spec 010).
    viz: Visualizer,
    /// Dónde quedó cada botón de "sonando" en el último frame.
    player_buttons: Vec<(PlayerButton, Rect)>,
}

impl DesktopApp {
    pub(super) fn new(
        inputs: UnboundedSender<Input>,
        outputs: mpsc::Receiver<Output>,
        startup: Startup,
    ) -> Self {
        let Startup {
            settings,
            settings_warnings,
            warnings,
            data_dir,
            hotkeys,
            theme,
            tap,
        } = startup;
        let keep_file = !settings_warnings.is_empty();
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
            volume: None,
            prompt: Prompt::Ready,
            engine_gone: false,
            settings,
            data_dir,
            menus: Menus::default(),
            commands: Vec::new(),
            theme,
            fonts: installed_fonts(),
            hotkeys,
            save_at: None,
            keep_file,
            started: false,
            viz: Visualizer::new(tap),
            player_buttons: Vec::new(),
        };
        app.push(ConsoleLine::Out(
            LineKind::Track,
            format!("{} {}", app.settings.console.prompt, config::WINDOW_TITLE),
        ));
        app.push(ConsoleLine::Out(
            LineKind::Dim,
            "Escribí `play <nombre>` para arrancar, o `help` para ver los comandos. \
             Arriba, los menús para personalizar la app."
                .into(),
        ));
        for warning in settings_warnings.into_iter().chain(warnings) {
            app.push(ConsoleLine::Out(LineKind::Warn, warning));
        }
        app.send(Input::Playback(app.settings.playback));
        app.send_shortcuts();
        app
    }

    fn push(&mut self, line: ConsoleLine) {
        while self.lines.len() >= self.settings.console.scrollback {
            self.lines.pop_front();
        }
        self.lines.push_back((local_clock(), line));
    }

    fn warn(&mut self, text: String) {
        self.push(ConsoleLine::Out(LineKind::Warn, text));
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

    /// Los atajos vigentes, para `help`.
    fn send_shortcuts(&mut self) {
        let window = self
            .settings
            .window_keys
            .iter()
            .filter_map(|(action, combo)| {
                combo.map(|combo| format!("{:<16} {}", combo.to_string(), action.label()))
            })
            .chain([format!(
                "{:<16} restaurar todos los ajustes",
                config::RESTORE_ALL_SHORTCUT.to_string()
            )])
            .collect();
        let global = self
            .hotkeys
            .iter()
            .flat_map(|h| h.active.iter().map(hotkeys::Shortcut::describe))
            .collect();
        self.send(Input::Shortcuts { window, global });
    }

    /// Todo lo que mandó el motor desde el último frame.
    fn drain(&mut self, ctx: &egui::Context) {
        while let Ok(output) = self.outputs.try_recv() {
            match output {
                Output::Line(kind, text) => self.push(ConsoleLine::Out(kind, text)),
                Output::NowPlaying(now) => self.now = now,
                Output::Prompt(prompt) => self.prompt = prompt,
                Output::Volume(volume) => self.volume = Some(volume),
                Output::Clear => self.lines.clear(),
                Output::Exit => ctx.send_viewport_cmd(ViewportCommand::Close),
                Output::Cover { url, cover } => {
                    // Tamaños de una tapa (≤ COVER_SIZE): entran en usize.
                    let size = [cover.width as usize, cover.height as usize];
                    let image = egui::ColorImage::from_rgba_unmultiplied(size, &cover.rgba);
                    self.viz.set_cover(ctx, url, image);
                }
            }
        }
    }

    /// Lleva el modo de los ajustes a la visualización y le dice al motor
    /// si hacen falta las tapas.
    fn sync_viz(&mut self) {
        if self.viz.mode() != self.settings.visualization {
            self.viz.set_mode(self.settings.visualization);
            self.send(Input::Covers(self.viz.wants_covers()));
        }
    }

    fn palette(&self) -> Palette {
        self.settings.appearance.palette
    }

    /// `base` (un tamaño pensado para la letra por defecto) llevado al
    /// tamaño de letra de los ajustes: texto de la consola, la entrada y
    /// "sonando" (spec 009).
    fn text_size(&self, base: f32) -> f32 {
        base * layout::scale(self.settings.appearance.font_size)
    }

    /// Fuente del título y del tema que suena: negrita si así está en los
    /// ajustes.
    fn strong(&self, size: f32) -> FontId {
        if self.settings.appearance.bold_titles {
            bold(size)
        } else {
            mono(size)
        }
    }

    fn prompt_text(&self) -> (String, Color32) {
        let p = self.palette();
        match self.prompt {
            Prompt::Ready => (self.settings.console.prompt.clone(), color(p.accent)),
            Prompt::Choose(n) => (format!("1-{n} ›"), color(p.warning)),
            Prompt::Busy(_) => ("… ›".into(), color(p.secondary)),
            Prompt::ClientId => ("Client ID ›".into(), color(p.warning)),
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
            while self.history.len() >= self.settings.console.history {
                self.history.pop_front();
            }
            self.history.push_back(trimmed.to_string());
        }
        self.history_pos = None;
        self.send(Input::Line(text));
    }

    /// Atajos que se atienden antes que la línea de entrada (así Ctrl+→
    /// no mueve el cursor por palabras y Tab no saca el foco). Con un menú
    /// abierto o una combinación grabándose, el teclado es de los menús.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        self.menus.keyboard(ctx, &mut self.settings);
        if self.menus.wants_keyboard(ctx) {
            return;
        }
        let restore = config::RESTORE_ALL_SHORTCUT;
        if let Some(key) = restore.key.to_egui() {
            if ctx.input_mut(|i| i.consume_key(restore.egui_modifiers(), key)) {
                self.commands.push(Command::RestoreAll);
            }
        }

        // Los configurados antes que ↑/↓ solos, para que Ctrl+↑/↓ no los
        // tome el historial.
        let mut actions = Vec::new();
        ctx.input_mut(|i| {
            for (&action, combo) in &self.settings.window_keys {
                let Some(combo) = combo else {
                    continue;
                };
                let mut keys: Vec<Key> = combo.key.to_egui().into_iter().collect();
                if combo.key == combo::Key::Plus {
                    // Misma tecla física en teclados en inglés.
                    keys.push(Key::Equals);
                }
                for key in keys {
                    // Varias pulsaciones pueden llegar en el mismo frame.
                    for _ in 0..i.count_and_consume_key(combo.egui_modifiers(), key) {
                        actions.push(action);
                    }
                }
            }
        });
        // Ctrl+rueda: tamaño de letra.
        let zoom = ctx.input(|i| i.zoom_delta());
        if zoom > 1.0 {
            actions.push(WindowAction::FontBigger);
        } else if zoom < 1.0 {
            actions.push(WindowAction::FontSmaller);
        }
        for action in actions {
            self.window_action(action);
        }

        let mut pressed = Vec::new();
        ctx.input_mut(|i| {
            for key in [Key::Escape, Key::ArrowUp, Key::ArrowDown, Key::Tab] {
                for _ in 0..i.count_and_consume_key(Modifiers::NONE, key) {
                    pressed.push(key);
                }
            }
        });
        for key in pressed {
            match key {
                Key::Escape if self.menus.has_dialog() => self.menus.close_dialogs(),
                Key::Escape => {
                    if self.input.is_empty() {
                        self.send(Input::Cancel);
                    } else {
                        self.input.clear();
                    }
                }
                Key::ArrowUp => self.browse_history(ctx, true),
                Key::ArrowDown => self.browse_history(ctx, false),
                Key::Tab => self.complete(ctx),
                _ => {}
            }
        }
    }

    fn window_action(&mut self, action: WindowAction) {
        match action {
            WindowAction::TogglePause => self.send(Input::TogglePause),
            WindowAction::Next => self.send(Input::Next),
            WindowAction::Prev => self.send(Input::Prev),
            WindowAction::VolumeUp => self.send(Input::VolumeUp),
            WindowAction::VolumeDown => self.send(Input::VolumeDown),
            // Como el atajo global: sin nada sonando no hace nada.
            WindowAction::Stop => self.send(Input::Global(GlobalAction::Stop)),
            WindowAction::Shuffle => self.send(Input::Line("shuffle".into())),
            WindowAction::ClearConsole => self.lines.clear(),
            WindowAction::FontBigger | WindowAction::FontSmaller | WindowAction::FontReset => {
                menu::apply_font_action(&mut self.settings, action);
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

    /// Lo que pidió el menú que no es cambiar un ajuste.
    fn run_command(&mut self, ctx: &egui::Context, command: Command) -> bool {
        let mut save = true;
        match command {
            Command::OpenFile => self.open_settings_file(),
            Command::Reload => {
                if let Some(dir) = &self.data_dir {
                    let (loaded, warnings) = settings::load(dir);
                    self.keep_file = !warnings.is_empty();
                    for warning in warnings {
                        self.warn(warning);
                    }
                    self.settings = loaded;
                    self.push(ConsoleLine::Out(
                        LineKind::Ok,
                        format!("Ajustes recargados de {}.", config::SETTINGS_FILE),
                    ));
                    // Lo que se leyó no se reescribe.
                    save = false;
                }
            }
            Command::RestoreAll => {
                self.settings = Settings::default();
                self.menus.close_dialogs();
                self.push(ConsoleLine::Out(
                    LineKind::Ok,
                    "Ajustes de fábrica restaurados.".into(),
                ));
            }
            Command::ClearConsole => self.lines.clear(),
            Command::ResetGeometry => {
                self.settings.window.geometry = None;
                let [width, height] = config::WINDOW_SIZE;
                ctx.send_viewport_cmd(ViewportCommand::Maximized(false));
                // `window::apply` lo toma en puntos lógicos de Windows.
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(width, height)));
            }
        }
        save
    }

    fn open_settings_file(&mut self) {
        let Some(dir) = self.data_dir.clone() else {
            self.warn("⚠ No hay carpeta de datos: los ajustes no se guardan.".into());
            return;
        };
        let path = dir.join(config::SETTINGS_FILE);
        if !path.exists() {
            if let Err(e) = settings::save(&dir, &self.settings) {
                self.warn(format!("⚠ No se pudo crear {}: {e}", path.display()));
                return;
            }
        }
        #[cfg(windows)]
        let opener = "explorer";
        #[cfg(not(windows))]
        let opener = "xdg-open";
        match std::process::Command::new(opener).arg(&path).spawn() {
            Ok(_) => self.push(ConsoleLine::Out(
                LineKind::Dim,
                format!(
                    "Abierto {}. Después de guardarlo: Ajustes → Recargar desde el archivo.",
                    path.display()
                ),
            )),
            Err(e) => self.warn(format!("⚠ No se pudo abrir {}: {e}", path.display())),
        }
    }

    /// Aplica lo que cambió de los ajustes en este frame y, si `save`,
    /// agenda el guardado.
    fn settings_changed(&mut self, ctx: &egui::Context, before: &Settings, save: bool) {
        let keys_changed = self.settings.window_keys != before.window_keys
            || self.settings.global_keys != before.global_keys;
        if self.settings.appearance != before.appearance {
            if let Some(warning) = self.theme.apply(ctx, &self.settings.appearance) {
                self.warn(warning);
            }
        }
        if self.settings.global_keys != before.global_keys {
            self.reload_hotkeys(before);
        }
        if keys_changed {
            self.send_shortcuts();
        }
        if self.settings.playback != before.playback {
            self.send(Input::Playback(self.settings.playback));
        }
        if self.settings.visualization != before.visualization {
            // Se aplica al dibujar el panel (`sync_viz`), en el próximo
            // frame si el cambio vino después de dibujarlo.
            ctx.request_repaint();
        }
        while self.lines.len() > self.settings.console.scrollback {
            self.lines.pop_front();
        }
        while self.history.len() > self.settings.console.history {
            self.history.pop_front();
            self.history_pos = None;
        }
        if self.settings.window.always_on_top != before.window.always_on_top {
            ctx.send_viewport_cmd(ViewportCommand::WindowLevel(window_level(
                self.settings.window.always_on_top,
            )));
        }
        if save {
            self.keep_file = false;
            let at = Instant::now() + config::SETTINGS_SAVE_DELAY;
            self.save_at = Some(at);
            ctx.request_repaint_after(config::SETTINGS_SAVE_DELAY);
        }
    }

    /// Registra los atajos globales nuevos. Uno que Windows rechaza vuelve
    /// al de antes, con aviso.
    fn reload_hotkeys(&mut self, before: &Settings) {
        let wanted = hotkeys::from_settings(&self.settings);
        let previous = hotkeys::from_settings(before);
        match hotkeys::replace(self.hotkeys.take(), &wanted, &previous, &self.inputs) {
            Ok((hotkeys, rejected)) => {
                self.hotkeys = Some(hotkeys);
                for failed in rejected {
                    let action = failed.shortcut.action;
                    let old = before.combo(Target::Global(action));
                    self.settings.global_keys.insert(action, old);
                    self.warn(format!(
                        "⚠ {} {}: el atajo global para {} queda {}.",
                        failed.shortcut.combo,
                        failed.reason,
                        action.describe(),
                        old.map_or_else(|| "sin combinación".into(), |c| format!("en {c}"))
                    ));
                }
            }
            Err(e) => self.warn(format!("⚠ Sin atajos globales: {e}")),
        }
    }

    fn save_if_due(&mut self, ctx: &egui::Context) {
        let Some(at) = self.save_at else {
            return;
        };
        let now = Instant::now();
        if now < at {
            // Despertó antes (otro evento): una sola espera más.
            ctx.request_repaint_after(at - now);
            return;
        }
        self.save_now();
    }

    fn save_now(&mut self) {
        self.save_at = None;
        let Some(dir) = &self.data_dir else {
            return;
        };
        if let Err(e) = settings::save(dir, &self.settings) {
            self.warn(format!("⚠ No se pudieron guardar los ajustes: {e}"));
        }
    }

    /// Al cerrar: anota tamaño y posición (si así está en los ajustes) y
    /// guarda lo pendiente.
    ///
    /// - No debe: pisar un `ajustes.json` con avisos que el usuario no
    ///   tocó (solo por la geometría).
    pub(super) fn finish(&mut self, ctx: &egui::Context) {
        let geometry = self
            .settings
            .window
            .remember_geometry
            .then(|| window_geometry(ctx))
            .flatten();
        let moved = geometry.is_some() && geometry != self.settings.window.geometry;
        if let Some(geometry) = geometry {
            self.settings.window.geometry = Some(geometry);
        }
        if self.save_at.is_some() || (moved && !self.keep_file) {
            self.save_now();
        }
    }

    fn title_bar(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        let ctx = ui.ctx().clone();
        let p = self.palette();
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let drag = ui.interact(rect, Id::new("barra-titulo"), Sense::click_and_drag());
        if drag.double_clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        } else if drag.drag_started_by(egui::PointerButton::Primary) {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        let mid = rect.center().y;
        let icon = ui.painter().text(
            Pos2::new(rect.left() + 14.0, mid),
            Align2::LEFT_CENTER,
            "♫",
            bold(16.0),
            color(p.accent),
        );

        let button = Vec2::new(46.0, rect.height());
        let close = Rect::from_min_size(Pos2::new(rect.right() - button.x, rect.top()), button);
        let max = close.translate(Vec2::new(-button.x, 0.0));
        let min = max.translate(Vec2::new(-button.x, 0.0));

        // Título después del ícono, cortado con "…" si no entra.
        let x = icon.right() + 8.0;
        let title = one_line(
            ui,
            config::WINDOW_TITLE,
            self.strong(13.0),
            color(p.text),
            (min.left() - 16.0 - x).max(0.0),
        );
        ui.painter().galley(
            Pos2::new(x, mid - title.size().y / 2.0),
            title,
            color(p.text),
        );

        if title_button(ui, &p, close, "cerrar", TitleIcon::Close).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        let icon = if maximized {
            TitleIcon::Restore
        } else {
            TitleIcon::Maximize
        };
        if title_button(ui, &p, max, "maximizar", icon).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        }
        if title_button(ui, &p, min, "minimizar", TitleIcon::Minimize).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
    }

    /// Renglón de menús, debajo de la barra de título.
    fn menu_row(&mut self, ui: &mut Ui) {
        let fonts = &self.fonts;
        let commands = ui
            .scope_builder(
                UiBuilder::new()
                    .max_rect(ui.max_rect())
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| self.menus.bar(ui, &mut self.settings, fonts),
            )
            .inner;
        self.commands.extend(commands);
    }

    fn console(&self, ui: &mut Ui) {
        let p = self.palette();
        let stamps = self.settings.console.timestamps;
        ScrollArea::vertical()
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = CONSOLE_ROW_GAP;
                for (stamp, line) in &self.lines {
                    let mut job = LayoutJob::default();
                    if stamps {
                        job.append(
                            &format!("{stamp} "),
                            0.0,
                            egui::TextFormat::simple(
                                mono(self.text_size(FONT_SIZE - 2.0)),
                                color(p.secondary),
                            ),
                        );
                    }
                    match line {
                        ConsoleLine::Echo { prompt, text } => {
                            job.append(
                                &format!("{prompt} "),
                                0.0,
                                egui::TextFormat::simple(
                                    self.strong(self.text_size(FONT_SIZE)),
                                    color(p.accent),
                                ),
                            );
                            job.append(
                                text,
                                0.0,
                                egui::TextFormat::simple(
                                    mono(self.text_size(FONT_SIZE)),
                                    color(p.text_strong),
                                ),
                            );
                        }
                        ConsoleLine::Out(kind, text) => {
                            job.append(
                                text,
                                0.0,
                                egui::TextFormat::simple(
                                    mono(self.text_size(FONT_SIZE)),
                                    line_color(&p, *kind),
                                ),
                            );
                        }
                    }
                    job.wrap.max_width = ui.available_width();
                    ui.label(job);
                }
            });
    }

    fn input_row(&mut self, ui: &mut Ui) {
        let p = self.palette();
        let (prompt, prompt_color) = self.prompt_text();
        let menus_have_keyboard = self.menus.wants_keyboard(ui.ctx());
        let strong = self.strong(self.text_size(FONT_SIZE));
        let font = mono(self.text_size(FONT_SIZE));
        let busy_font = mono(self.text_size(FONT_SIZE - 3.0));
        ui.horizontal_centered(|ui| {
            ui.label(RichText::new(prompt).font(strong).color(prompt_color));
            let busy = match self.prompt {
                Prompt::Busy(what) => Some(format!("⋯ {what}  (Esc cancela)")),
                _ => None,
            };
            // Lugar para el aviso de ocupado, a la derecha.
            let busy_width = busy.as_ref().map_or(0.0, |text| {
                let galley = ui.painter().layout_no_wrap(
                    text.clone(),
                    busy_font.clone(),
                    color(p.secondary),
                );
                galley.size().x + ui.spacing().item_spacing.x
            });
            let hint = match self.prompt {
                Prompt::Ready => "escribí un comando · help",
                Prompt::Choose(_) => "número y Enter · Enter = el primero · Esc cancela",
                Prompt::Busy(_) => "",
                Prompt::ClientId => "pegá el Client ID y Enter · Esc cancela",
            };
            let edit = TextEdit::singleline(&mut self.input)
                .id(self.input_id)
                // Tab no saca el foco: lo usa `complete`.
                .lock_focus(true)
                .frame(Frame::NONE)
                .font(font.clone())
                .text_color(color(p.text_strong))
                .hint_text(
                    RichText::new(hint)
                        .font(font)
                        .color(color(p.secondary).gamma_multiply(0.7)),
                )
                .desired_width(ui.available_width() - busy_width);
            let response = ui.add(edit);
            if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                self.submit();
            }
            // La consola es la entrada: el foco vuelve siempre acá, salvo
            // mientras se usa un menú.
            if !menus_have_keyboard {
                response.request_focus();
            }
            if let Some(text) = busy {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(text)
                            .font(busy_font)
                            .color(color(p.secondary)),
                    );
                });
            }
        });
    }

    /// Barra "sonando": qué suena, los botones (spec 012) y el progreso.
    /// Sin nada sonando, los botones se ven atenuados y no hacen nada.
    fn now_bar(&mut self, ui: &mut Ui) {
        let p = self.palette();
        let rect = ui.max_rect().shrink2(Vec2::new(16.0, 0.0));
        let scale = layout::scale(self.settings.appearance.font_size);
        let t = move |base: f32| base * scale;
        let painter = ui.painter().clone();
        let now = self.now.clone();
        let chip_font = mono(t(12.0));
        let measure = |text: &str| {
            one_line(ui, text, chip_font.clone(), Color32::WHITE, f32::INFINITY)
                .size()
                .x
        };
        // Con algo sonando hay dos filas (con la letra por defecto, a 20 del
        // borde de arriba y 18 del de abajo, como en 0.1.0); si no, una.
        let top = if now.is_some() {
            rect.center().y - t(12.0)
        } else {
            rect.center().y
        };

        // Chips de la derecha: volumen y, con algo sonando, cola y posición.
        let volume = self.volume.map(|v| volume_chip(&p, v));
        let mut others = Vec::new();
        if let Some(now) = &now {
            if now.queued > 0 {
                others.push((format!("cola {}", now.queued), color(p.secondary)));
            }
            if !now.position.trim().is_empty() {
                others.push((now.position.trim().to_string(), color(p.secondary)));
            }
        }

        let mut left = rect.left();
        if let Some(now) = &now {
            let (state_icon, state_color) = match now.state {
                PlayState::Loading => ("…", color(p.secondary)),
                PlayState::Playing => ("▶", color(p.accent)),
                PlayState::Paused => ("⏸", color(p.warning)),
            };
            let icon = painter.text(
                Pos2::new(rect.left(), top),
                Align2::LEFT_CENTER,
                state_icon,
                bold(t(14.0)),
                state_color,
            );
            left = icon.right() + t(10.0);
        }

        let (button, space) = (t(PLAYER_BUTTON), t(PLAYER_SPACE));
        let widths: Vec<f32> = others.iter().map(|(text, _)| measure(text)).collect();
        let row = layout::player_row(
            left,
            rect.right(),
            rect.center().x,
            layout::ButtonSizes {
                button,
                gap: t(PLAYER_BUTTON_GAP),
                space,
            },
            volume.as_ref().map(|(text, _)| measure(text)),
            &widths,
        );
        if let (Some(x), Some((text, chip_color))) = (row.volume, &volume) {
            painter.text(
                Pos2::new(x, top),
                Align2::RIGHT_CENTER,
                text,
                chip_font.clone(),
                *chip_color,
            );
        }
        for ((text, chip_color), x) in others.iter().zip(&row.others) {
            if let Some(x) = x {
                painter.text(
                    Pos2::new(*x, top),
                    Align2::RIGHT_CENTER,
                    text,
                    chip_font.clone(),
                    *chip_color,
                );
            }
        }

        // Título — artistas (o "nada sonando"), cortados con "…" antes del ♥.
        let max_width = (row.text_right - button - space - left).max(0.0);
        let text_end = match &now {
            Some(now) => {
                let title = if now.title.is_empty() {
                    "Cargando…"
                } else {
                    now.title.as_str()
                };
                let title_galley = one_line(
                    ui,
                    title,
                    self.strong(t(14.0)),
                    color(p.text_strong),
                    max_width,
                );
                let title_width = title_galley.size().x;
                painter.galley(
                    Pos2::new(left, top - title_galley.size().y / 2.0),
                    title_galley,
                    color(p.text_strong),
                );
                let mut end = left + title_width;
                if !now.artists.is_empty() {
                    let artists = format!("  —  {}", now.artists);
                    let galley = one_line(
                        ui,
                        &artists,
                        mono(t(13.0)),
                        color(p.secondary),
                        (max_width - title_width).max(0.0),
                    );
                    end += galley.size().x;
                    painter.galley(
                        Pos2::new(left + title_width, top - galley.size().y / 2.0),
                        galley,
                        color(p.secondary),
                    );
                }
                end
            }
            None => {
                let idle = one_line(
                    ui,
                    "Nada sonando · escribí  play <nombre>",
                    mono(t(13.0)),
                    color(p.secondary),
                    max_width,
                );
                let end = left + idle.size().x;
                painter.galley(
                    Pos2::new(left, top - idle.size().y / 2.0),
                    idle,
                    color(p.secondary),
                );
                end
            }
        };
        let heart = (text_end + space / 2.0 + button / 2.0).min(row.text_right - button / 2.0);

        // Botones.
        let active = now.is_some();
        let playing = now.as_ref().is_some_and(|n| n.state == PlayState::Playing);
        let liked = now.as_ref().is_some_and(|n| n.liked == Some(true));
        let shuffled = now.as_ref().is_some_and(|n| n.shuffle);
        let buttons = [
            (PlayerButton::Prev, row.controls[0], false),
            (PlayerButton::PlayPause, row.controls[1], false),
            (PlayerButton::Next, row.controls[2], false),
            (PlayerButton::Shuffle, row.shuffle, shuffled),
            (PlayerButton::Like, heart, liked),
        ];
        let mut clicked = None;
        self.player_buttons.clear();
        for (kind, x, on) in buttons {
            let area = Rect::from_center_size(Pos2::new(x, top), Vec2::splat(button));
            let look = ButtonLook {
                active,
                on,
                playing,
                shuffle_font: mono(t(13.0)),
                corner: t(4.0),
            };
            let tip = self.button_tip(kind, playing, liked);
            if player_button(ui, &p, area, kind, &look, &tip) {
                clicked = Some(kind);
            }
            self.player_buttons.push((kind, area));
        }
        if let Some(kind) = clicked {
            self.send(kind.input());
        }

        let Some(now) = now else {
            return;
        };
        // Fila de abajo: tiempo, barra de progreso y duración.
        let bottom = rect.center().y + t(14.0);
        let elapsed = now.clock.elapsed().min(now.duration);
        let left = painter.text(
            Pos2::new(rect.left(), bottom),
            Align2::LEFT_CENTER,
            clock_text(elapsed),
            mono(t(11.0)),
            color(p.secondary),
        );
        let right = painter.text(
            Pos2::new(rect.right(), bottom),
            Align2::RIGHT_CENTER,
            clock_text(now.duration),
            mono(t(11.0)),
            color(p.secondary),
        );
        let track = Rect::from_min_max(
            Pos2::new(left.right() + t(12.0), bottom - 2.0),
            Pos2::new(right.left() - t(12.0), bottom + 2.0),
        );
        painter.rect_filled(track, 2.0, color(p.border));
        if now.duration > Duration::ZERO {
            let fraction = elapsed.as_secs_f32() / now.duration.as_secs_f32();
            let mut done = track;
            done.set_width(track.width() * fraction.clamp(0.0, 1.0));
            painter.rect_filled(done, 2.0, color(p.accent));
        }
    }

    /// Tooltip de un botón de "sonando": la acción y, si tiene, su atajo
    /// de ventana vigente.
    fn button_tip(&self, kind: PlayerButton, playing: bool, liked: bool) -> String {
        let label = match kind {
            PlayerButton::Prev => "Anterior",
            PlayerButton::PlayPause if playing => "Pausar",
            PlayerButton::PlayPause => "Reanudar",
            PlayerButton::Next => "Siguiente",
            PlayerButton::Shuffle => "Shuffle sí / no",
            PlayerButton::Like if liked => "Quitar de Tus me gusta",
            PlayerButton::Like => "Agregar a Tus me gusta",
        };
        let combo = kind
            .window_action()
            .and_then(|action| self.settings.window_keys.get(&action).copied().flatten());
        match combo {
            Some(combo) => format!("{label}  ({combo})"),
            None => label.to_string(),
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

impl DesktopApp {
    /// Borde izquierdo del panel de la visualización (`panel`, que hoy
    /// mide `width`): arrastrarlo cambia `settings.viz_width` (spec 010,
    /// AC-2), que se guarda como cualquier ajuste.
    fn viz_grip(&mut self, ui: &mut Ui, panel: Rect, width: f32) {
        let half = config::viz::PANEL_GRIP / 2.0;
        let grip =
            Rect::from_x_y_ranges(panel.left() - half..=panel.left() + half, panel.y_range());
        let response = ui.interact(grip, Id::new("borde-visualizacion"), Sense::drag());
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
        if response.dragged() {
            // Desde el ancho que se ve (puede estar recortado por la
            // ventana), así el borde sigue al mouse.
            self.settings.viz_width = (width - response.drag_delta().x)
                .round()
                .clamp(config::viz::PANEL_WIDTH_MIN, config::viz::PANEL_WIDTH_MAX);
        }
    }
}

impl Content for DesktopApp {
    fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if !self.started {
            self.started = true;
            if self.settings.window.always_on_top {
                ctx.send_viewport_cmd(ViewportCommand::WindowLevel(WindowLevel::AlwaysOnTop));
            }
        }
        let before = self.settings.clone();
        self.drain(&ctx);
        self.shortcuts(&ctx);

        let p = self.palette();
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, color(p.background));

        let content_font = mono(self.text_size(FONT_SIZE));
        let console_row = ctx.fonts_mut(|f| f.row_height(&content_font)) + CONSOLE_ROW_GAP;
        let bars = layout::bars(
            self.settings.appearance.font_size,
            console_row,
            rect.height(),
        );

        Panel::top("titulo")
            .exact_size(TITLE_HEIGHT)
            .frame(Frame::new().fill(color(p.panel)))
            .show_inside(ui, |ui| self.title_bar(ui));
        Panel::top("menus")
            .exact_size(MENU_HEIGHT)
            .frame(
                Frame::new()
                    .fill(color(p.panel))
                    .inner_margin(Margin::symmetric(8, 0))
                    .stroke(Stroke::new(1.0_f32, color(p.border))),
            )
            .show_inside(ui, |ui| self.menu_row(ui));
        Panel::bottom("sonando")
            .exact_size(bars.now)
            .frame(
                Frame::new()
                    .fill(color(p.panel))
                    .stroke(Stroke::new(1.0_f32, color(p.border))),
            )
            .show_inside(ui, |ui| self.now_bar(ui));
        Panel::bottom("entrada")
            .exact_size(bars.input)
            .frame(Frame::new().inner_margin(Margin::symmetric(14, 0)))
            .show_inside(ui, |ui| self.input_row(ui));
        self.sync_viz();
        if let Some(width) = self.viz.panel_width(self.settings.viz_width, rect.width()) {
            let panel = Panel::right("visualizacion")
                .exact_size(width)
                .resizable(false)
                .frame(
                    Frame::new()
                        .fill(color(p.background))
                        .inner_margin(Margin::same(VIZ_MARGIN))
                        .stroke(Stroke::new(1.0_f32, color(p.border))),
                )
                .show_inside(ui, |ui| {
                    let track = self
                        .now
                        .as_ref()
                        .map_or(Track::Nothing, |now| Track::Loaded {
                            state: now.state,
                            cover: now.cover.as_deref(),
                        });
                    if self.viz.show(ui, ui.max_rect(), track, &p) {
                        ctx.request_repaint_after(Duration::from_millis(1000 / config::viz::FPS));
                    }
                });
            self.viz_grip(ui, panel.response.rect, width);
        }
        CentralPanel::default()
            .frame(Frame::new().inner_margin(Margin {
                left: 14,
                right: 6,
                top: CONSOLE_MARGIN_TOP,
                bottom: CONSOLE_MARGIN_BOTTOM,
            }))
            .show_inside(ui, |ui| self.console(ui));
        self.menus.dialogs(&ctx, &mut self.settings);

        ui.painter().rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0_f32, color(p.border)),
            StrokeKind::Inside,
        );
        self.resize_grips(ui, rect);

        let mut save = true;
        for command in std::mem::take(&mut self.commands) {
            save &= self.run_command(&ctx, command);
        }
        if self.settings != before {
            self.settings_changed(&ctx, &before, save);
        }
        self.save_if_due(&ctx);

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

fn window_level(always_on_top: bool) -> WindowLevel {
    if always_on_top {
        WindowLevel::AlwaysOnTop
    } else {
        WindowLevel::Normal
    }
}

/// Posición (px físicos) y tamaño (puntos lógicos de Windows) de la
/// ventana, o `None` si está maximizada o minimizada (no se anota: al
/// reabrir queda la de antes). No depende del tamaño de letra (spec 009).
fn window_geometry(ctx: &egui::Context) -> Option<Geometry> {
    let info = ctx.input(|i| i.viewport().clone());
    if info.maximized == Some(true) || info.minimized == Some(true) {
        return None;
    }
    let (outer, inner, native) = (
        info.outer_rect?,
        info.inner_rect?,
        info.native_pixels_per_point?,
    );
    let points_to_px = ctx.pixels_per_point();
    let to_logical = points_to_px / native;
    // Coordenadas de pantalla: entran de sobra en i32.
    #[allow(clippy::cast_possible_truncation)]
    Some(Geometry {
        x: (outer.min.x * points_to_px).round() as i32,
        y: (outer.min.y * points_to_px).round() as i32,
        width: (inner.width() * to_logical).round(),
        height: (inner.height() * to_logical).round(),
    })
}

/// Hora local "HH:MM:SS", para la hora de cada línea.
#[cfg(windows)]
fn local_clock() -> String {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY: SYSTEMTIME es un struct de C sin invariantes; GetLocalTime lo
    // llena entero.
    let time = unsafe {
        let mut time = std::mem::zeroed();
        GetLocalTime(&mut time);
        time
    };
    format!("{:02}:{:02}:{:02}", time.wHour, time.wMinute, time.wSecond)
}

/// Fuera de Windows, la hora UTC (sin crate de zonas horarias).
#[cfg(not(windows))]
fn local_clock() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600 % 24,
        secs / 60 % 60,
        secs % 60
    )
}

fn line_color(p: &Palette, kind: LineKind) -> Color32 {
    match kind {
        LineKind::Normal => color(p.text),
        LineKind::Track => color(p.accent),
        LineKind::Ok => color(p.accent),
        LineKind::Warn => color(p.warning),
        LineKind::Error => color(p.error),
        LineKind::Dim => color(p.secondary),
    }
}

/// "m:ss".
fn clock_text(time: Duration) -> String {
    let secs = time.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Indicador de volumen de la barra: "vol 70 %", o "mute" en amarillo.
fn volume_chip(p: &Palette, volume: Volume) -> (String, Color32) {
    if volume.muted() {
        ("mute".into(), color(p.warning))
    } else {
        (format!("vol {} %", volume.level()), color(p.secondary))
    }
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
fn title_button(ui: &mut Ui, p: &Palette, rect: Rect, id: &str, icon: TitleIcon) -> egui::Response {
    let response = ui.interact(rect, Id::new(("boton-titulo", id)), Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        let fill = match icon {
            TitleIcon::Close => color(p.close_hover),
            _ => color(p.border),
        };
        painter.rect_filled(rect, 0.0, fill);
    }
    let stroke = Stroke::new(1.2_f32, color(p.text));
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
            painter.rect_filled(front, 0.0, color(p.panel));
            painter.rect_stroke(front, 0.0, stroke, StrokeKind::Middle);
        }
        TitleIcon::Close => {
            painter.line_segment([c + Vec2::new(-s, -s), c + Vec2::new(s, s)], stroke);
            painter.line_segment([c + Vec2::new(-s, s), c + Vec2::new(s, -s)], stroke);
        }
    }
    response
}

/// Un botón de la barra "sonando" (spec 012).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerButton {
    Prev,
    PlayPause,
    Next,
    Shuffle,
    Like,
}

impl PlayerButton {
    /// Lo que se le manda al motor: lo mismo que el comando o el atajo.
    fn input(self) -> Input {
        match self {
            PlayerButton::Prev => Input::Prev,
            PlayerButton::PlayPause => Input::TogglePause,
            PlayerButton::Next => Input::Next,
            PlayerButton::Shuffle => Input::Shuffle,
            PlayerButton::Like => Input::ToggleLike,
        }
    }

    /// El atajo de ventana que hace lo mismo, para el tooltip.
    fn window_action(self) -> Option<WindowAction> {
        match self {
            PlayerButton::Prev => Some(WindowAction::Prev),
            PlayerButton::PlayPause => Some(WindowAction::TogglePause),
            PlayerButton::Next => Some(WindowAction::Next),
            PlayerButton::Shuffle => Some(WindowAction::Shuffle),
            PlayerButton::Like => None,
        }
    }
}

/// Cómo se dibuja un botón de "sonando".
struct ButtonLook {
    /// Hay algo sonando: si no, se ve atenuado y no responde.
    active: bool,
    /// Encendido (shuffle sí, ♥ lleno): color de acento.
    on: bool,
    /// Para ⏯: sonando muestra ⏸, si no ▶.
    playing: bool,
    shuffle_font: FontId,
    corner: f32,
}

/// Dibuja un botón de "sonando" en `area`. `true` si se lo clickeó estando
/// activo. Los íconos se dibujan con formas (no dependen de la fuente),
/// salvo el de shuffle, que es el 🔀 de siempre.
fn player_button(
    ui: &Ui,
    p: &Palette,
    area: Rect,
    kind: PlayerButton,
    look: &ButtonLook,
    tip: &str,
) -> bool {
    let sense = if look.active {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(area, Id::new(("boton-sonando", kind as u8)), sense);
    let painter = ui.painter();
    if look.active && response.hovered() {
        painter.rect_filled(area, look.corner, color(p.border));
    }
    let tint = if !look.active {
        color(p.secondary).gamma_multiply(0.35)
    } else if look.on {
        color(p.accent)
    } else if kind == PlayerButton::Shuffle || kind == PlayerButton::Like {
        color(p.secondary)
    } else {
        color(p.text_strong)
    };
    let c = area.center();
    let k = area.width() / 2.0;
    let at = |x: f32, y: f32| c + Vec2::new(x * k, y * k);
    let triangle = |points: Vec<Pos2>| Shape::convex_polygon(points, tint, Stroke::NONE);
    match kind {
        PlayerButton::PlayPause if look.playing => {
            for x in [-0.22, 0.22] {
                let bar = Rect::from_center_size(at(x, 0.0), Vec2::new(0.2 * k, 0.8 * k));
                painter.rect_filled(bar, 1.0, tint);
            }
        }
        PlayerButton::PlayPause => {
            painter.add(triangle(vec![
                at(-0.3, -0.42),
                at(0.48, 0.0),
                at(-0.3, 0.42),
            ]));
        }
        PlayerButton::Next | PlayerButton::Prev => {
            // Dibujado hacia la derecha; "anterior" es el espejo.
            let dir = if kind == PlayerButton::Next {
                1.0
            } else {
                -1.0
            };
            let (a, b, d) = (
                at(-0.4 * dir, -0.36),
                at(0.2 * dir, 0.0),
                at(-0.4 * dir, 0.36),
            );
            let points = if dir > 0.0 {
                vec![a, b, d]
            } else {
                vec![a, d, b]
            };
            painter.add(triangle(points));
            let bar = Rect::from_center_size(at(0.32 * dir, 0.0), Vec2::new(0.14 * k, 0.72 * k));
            painter.rect_filled(bar, 1.0, tint);
        }
        PlayerButton::Like => {
            for x in [-0.22, 0.22] {
                painter.circle_filled(at(x, -0.12), 0.25 * k, tint);
            }
            painter.add(triangle(vec![
                at(-0.46, -0.04),
                at(0.46, -0.04),
                at(0.0, 0.5),
            ]));
        }
        PlayerButton::Shuffle => {
            painter.text(
                c,
                Align2::CENTER_CENTER,
                "🔀",
                look.shuffle_font.clone(),
                tint,
            );
        }
    }
    let response = if look.active {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    };
    let clicked = look.active && response.clicked();
    response.on_hover_text(tip);
    clicked
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

    fn app_with(settings: Settings) -> (DesktopApp, tokio::sync::mpsc::UnboundedReceiver<Input>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (_out_tx, out_rx) = mpsc::channel();
        let startup = Startup {
            settings,
            settings_warnings: Vec::new(),
            warnings: Vec::new(),
            data_dir: None,
            hotkeys: None,
            theme: Theme::default(),
            tap: AudioTap::new(config::viz::TAP_CAPACITY),
        };
        (DesktopApp::new(tx, out_rx, startup), rx)
    }

    /// App de prueba, sin lo que manda al motor al arrancar.
    fn app() -> (DesktopApp, tokio::sync::mpsc::UnboundedReceiver<Input>) {
        let (app, mut rx) = app_with(Settings::default());
        while rx.try_recv().is_ok() {}
        (app, rx)
    }

    #[test]
    fn al_arrancar_manda_reproduccion_y_atajos() {
        let mut settings = Settings::default();
        settings.playback.volume_step = 10;
        let (_app, mut rx) = app_with(settings.clone());
        assert_eq!(rx.try_recv().ok(), Some(Input::Playback(settings.playback)));
        let Ok(Input::Shortcuts { window, global }) = rx.try_recv() else {
            panic!("sin atajos");
        };
        assert!(
            window
                .iter()
                .any(|l| l.starts_with("Ctrl+→ ") && l.ends_with(" siguiente"))
        );
        assert!(window.iter().any(|l| l.starts_with("Ctrl+Shift+F12 ")));
        assert!(global.is_empty(), "sin hilo de atajos en el test");
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
    fn el_scrollback_y_el_historial_tienen_el_tope_de_los_ajustes() {
        let (mut app, _rx) = app();
        let ctx = egui::Context::default();
        for i in 0..config::SCROLLBACK_LINES + 10 {
            app.push(ConsoleLine::Out(LineKind::Normal, i.to_string()));
        }
        assert_eq!(app.lines.len(), config::SCROLLBACK_LINES);

        // Achicarlo descarta las más viejas.
        let before = app.settings.clone();
        app.settings.console.scrollback = config::SCROLLBACK_MIN;
        app.settings.console.history = config::HISTORY_MIN;
        for i in 0..config::HISTORY_MIN + 5 {
            app.history.push_back(i.to_string());
        }
        app.settings_changed(&ctx, &before, false);
        assert_eq!(app.lines.len(), config::SCROLLBACK_MIN);
        let Some((_, ConsoleLine::Out(_, last))) = app.lines.back() else {
            panic!("sin líneas");
        };
        assert_eq!(*last, (config::SCROLLBACK_LINES + 9).to_string());
        assert_eq!(app.history.len(), config::HISTORY_MIN);
    }

    #[test]
    fn cambiar_reproduccion_o_atajos_avisa_al_motor() {
        let (mut app, mut rx) = app();
        let ctx = egui::Context::default();
        let before = app.settings.clone();
        app.settings.playback.volume_step = 20;
        app.settings.assign(
            Target::Window(WindowAction::Stop),
            Some("Ctrl+F5".parse().unwrap()),
        );
        app.settings_changed(&ctx, &before, false);
        let sent: Vec<Input> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert!(
            sent.iter()
                .any(|i| matches!(i, Input::Playback(p) if p.volume_step == 20))
        );
        assert!(sent.iter().any(|i| matches!(
            i,
            Input::Shortcuts { window, .. } if window.iter().any(|l| l.starts_with("Ctrl+F5 "))
        )));
        // Sin cambios no manda nada.
        let before = app.settings.clone();
        app.settings_changed(&ctx, &before, false);
        assert!(rx.try_recv().is_err());
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

    /// Contexto con las fuentes del tema, como al abrir la app.
    fn themed(app: &DesktopApp) -> egui::Context {
        let ctx = egui::Context::default();
        let _ = Theme::default().apply(&ctx, &app.settings.appearance);
        ctx
    }

    /// Un frame de la ventana, sin ventana de verdad.
    fn frame(ctx: &egui::Context, app: &mut DesktopApp, events: Vec<egui::Event>) {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::from(config::WINDOW_SIZE),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run_ui(raw, |ui| app.ui(ui));
    }

    /// Click (apretar y soltar) en el centro de `area`.
    fn click(ctx: &egui::Context, app: &mut DesktopApp, area: Rect) {
        let pos = area.center();
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        frame(ctx, app, vec![egui::Event::PointerMoved(pos), button(true)]);
        frame(ctx, app, vec![button(false)]);
    }

    fn sonando() -> NowPlaying {
        NowPlaying {
            title: "Tema".into(),
            artists: "Artista".into(),
            position: "[1/3] ".into(),
            duration: Duration::from_secs(200),
            clock: Default::default(),
            state: PlayState::Playing,
            shuffle: false,
            queued: 0,
            cover: None,
            liked: Some(false),
        }
    }

    fn area_of(app: &DesktopApp, kind: PlayerButton) -> Rect {
        app.player_buttons
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, r)| *r)
            .unwrap()
    }

    #[test]
    fn cada_boton_manda_lo_mismo_que_su_comando_y_la_entrada_sigue_con_foco() {
        let (mut app, mut rx) = app();
        app.now = Some(sonando());
        let ctx = themed(&app);
        frame(&ctx, &mut app, Vec::new());
        for (kind, expected) in [
            (PlayerButton::Prev, Input::Prev),
            (PlayerButton::PlayPause, Input::TogglePause),
            (PlayerButton::Next, Input::Next),
            (PlayerButton::Shuffle, Input::Shuffle),
            (PlayerButton::Like, Input::ToggleLike),
        ] {
            let area = area_of(&app, kind);
            click(&ctx, &mut app, area);
            let sent: Vec<Input> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
            assert_eq!(sent, [expected], "{kind:?}");
            frame(&ctx, &mut app, Vec::new());
            assert!(ctx.memory(|m| m.has_focus(app.input_id)), "{kind:?}");
        }
    }

    #[test]
    fn sin_nada_sonando_los_botones_no_mandan_nada() {
        let (mut app, mut rx) = app();
        let ctx = themed(&app);
        frame(&ctx, &mut app, Vec::new());
        assert_eq!(app.player_buttons.len(), 5);
        for (_, area) in app.player_buttons.clone() {
            click(&ctx, &mut app, area);
        }
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn los_botones_no_se_pisan_con_letra_maxima_en_la_ventana_minima() {
        let (mut app, _rx) = app();
        app.settings.appearance.font_size = config::FONT_SIZE_MAX;
        app.now = Some(NowPlaying {
            title: "Un título larguísimo que no entra de ninguna manera".into(),
            queued: 12,
            ..sonando()
        });
        app.volume = Some(Volume::default());
        let ctx = themed(&app);
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::from(config::WINDOW_MIN_SIZE),
            )),
            ..Default::default()
        };
        let _ = ctx.run_ui(raw, |ui| app.ui(ui));
        let areas: Vec<Rect> = app.player_buttons.iter().map(|(_, r)| *r).collect();
        for (i, a) in areas.iter().enumerate() {
            assert!(
                a.min.x >= 0.0 && a.max.x <= config::WINDOW_MIN_SIZE[0],
                "{i}"
            );
            for b in &areas[i + 1..] {
                assert!(!a.intersects(*b), "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn tooltip_con_el_atajo_de_ventana() {
        let (app, _rx) = app();
        let tip = app.button_tip(PlayerButton::Next, true, false);
        assert!(tip.starts_with("Siguiente  (Ctrl+"), "{tip}");
        assert_eq!(
            app.button_tip(PlayerButton::PlayPause, true, false),
            "Pausar  (Ctrl+Espacio)"
        );
        assert_eq!(
            app.button_tip(PlayerButton::Like, true, true),
            "Quitar de Tus me gusta"
        );
    }

    #[test]
    fn hora_con_formato() {
        let clock = local_clock();
        assert_eq!(clock.len(), 8, "{clock}");
        assert_eq!(clock.as_bytes()[2], b':');
    }
}
