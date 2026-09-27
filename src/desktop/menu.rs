//! Barra de menús de la app de escritorio (spec 007), en un renglón bajo
//! la barra de título: Personalización (con los submenús Tema, Fuente,
//! Atajos, Consola y Ventana, spec 009), Reproducción y Ajustes, más los
//! diálogos de colores y de atajos.
//!
//! Edita los `Settings` que le pasa la ventana y devuelve lo que no es un
//! ajuste (`Command`). No aplica nada por su cuenta: la ventana compara los
//! ajustes antes y después del frame y aplica lo que cambió.

use std::time::Duration;

use egui::{
    Button, Color32, Context, Event, FocusDirection, Grid, Id, Key, MenuBar, Popup,
    PopupCloseBehavior, RichText, Slider, Stroke, TextFormat, Ui,
    containers::menu::{MenuConfig, MenuState, SubMenu, SubMenuButton},
    text::LayoutJob,
};

use super::{
    combo::Combo,
    settings::{
        self, ColorSlot, ComboCheck, Section, Settings, Target, WindowAction, bitrate_from_kbps,
        bitrate_kbps,
    },
    theme::color,
};
use crate::{app::engine::GlobalAction, config};

/// Lo que pidió el usuario en el menú que no es cambiar un ajuste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Command {
    /// Abrir `ajustes.json` con la app asociada.
    OpenFile,
    /// Releer `ajustes.json` y aplicarlo.
    Reload,
    RestoreAll,
    ClearConsole,
    /// Volver la ventana al tamaño inicial.
    ResetGeometry,
}

/// Títulos de los menús; la letra subrayada es `config::MENU_ACCESS_KEYS`.
const TITLES: [&str; 3] = ["Personalización", "Reproducción", "Ajustes"];

/// Submenús de Personalización, en orden.
const SUBMENUS: [&str; 5] = ["Tema", "Fuente", "Atajos", "Consola", "Ventana"];

/// Ancho mínimo de un menú desplegado.
const MENU_MIN_WIDTH: f32 = 260.0;

/// Estado de la barra entre frames.
#[derive(Default)]
pub(super) struct Menus {
    /// Menú pedido con `Alt`+letra, a abrir en este frame.
    open: Option<usize>,
    /// Dar el foco al primer ítem del menú recién abierto con el teclado,
    /// para seguir con flechas y Enter.
    focus_first: bool,
    /// Lo mismo para el submenú recién abierto con el teclado (→ o Enter).
    focus_sub: bool,
    colors_open: bool,
    keys_open: bool,
    /// Atajo que espera la próxima combinación que se apriete.
    recording: Option<Target>,
    /// Mensaje del diálogo de atajos; `true` = error.
    notice: Option<(String, bool)>,
    /// Combinación pedida que ya usa otro atajo: (para, combinación, dueño).
    confirm: Option<(Target, Combo, Target)>,
    /// Lo que se está escribiendo en "Símbolo del prompt".
    prompt: Option<String>,
}

impl Menus {
    /// `true` si el teclado es de los menús (un menú abierto, un selector
    /// de color o una combinación grabándose): la ventana no atiende sus
    /// atajos ni le da el foco a la línea de entrada.
    pub(super) fn wants_keyboard(&self, ctx: &Context) -> bool {
        self.recording.is_some() || Popup::is_any_open(ctx)
    }

    /// `true` si hay un diálogo (colores o atajos) abierto.
    pub(super) fn has_dialog(&self) -> bool {
        self.colors_open || self.keys_open
    }

    pub(super) fn close_dialogs(&mut self) {
        self.colors_open = false;
        self.keys_open = false;
        self.confirm = None;
        self.notice = None;
    }

    /// Atiende el teclado antes que el resto de la ventana: `Alt`+letra
    /// abre un menú, y si se está grabando una combinación, la toma (la
    /// tecla no llega a nadie más).
    pub(super) fn keyboard(&mut self, ctx: &Context, settings: &mut Settings) {
        if let Some(target) = self.recording {
            self.record(ctx, settings, target);
            return;
        }
        for (index, &letter) in config::MENU_ACCESS_KEYS.iter().enumerate() {
            let Some(key) = Key::from_name(&letter.to_string()) else {
                continue;
            };
            let pressed = ctx.input_mut(|i| {
                let pressed = i.consume_key(egui::Modifiers::ALT, key);
                if pressed {
                    // Windows manda también la letra como texto: que no
                    // llegue a la línea de entrada.
                    i.events.retain(|e| !matches!(e, Event::Text(_)));
                }
                pressed
            });
            if pressed {
                self.open = Some(index);
            }
        }
    }

    fn record(&mut self, ctx: &Context, settings: &mut Settings, target: Target) {
        let pressed = ctx.input(|i| {
            i.events.iter().find_map(|event| match event {
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => Some((*key, *modifiers)),
                _ => None,
            })
        });
        let Some((key, modifiers)) = pressed else {
            return;
        };
        ctx.input_mut(|i| {
            i.consume_key(modifiers, key);
            i.events.retain(|e| !matches!(e, Event::Text(_)));
        });
        self.recording = None;
        if key == Key::Escape && !modifiers.any() {
            self.notice = None;
            return;
        }
        let Some(combo) = Combo::from_egui(modifiers, key) else {
            self.notice = Some(("Esa tecla no se puede usar en un atajo.".into(), true));
            return;
        };
        match settings.check_combo(target, combo) {
            ComboCheck::Ok => {
                settings.assign(target, Some(combo));
                self.notice = Some((format!("✓ {combo} → {}", target_name(target)), false));
            }
            ComboCheck::Reserved(reason) => {
                self.notice = Some((format!("{combo} no se puede usar: {reason}."), true));
            }
            ComboCheck::UsedBy(other) => {
                self.notice = None;
                self.confirm = Some((target, combo, other));
            }
        }
    }

    /// Dibuja la barra en `ui` (su renglón). Devuelve lo pedido.
    pub(super) fn bar(
        &mut self,
        ui: &mut Ui,
        settings: &mut Settings,
        fonts: &[&str],
    ) -> Vec<Command> {
        let mut commands = Vec::new();
        let config = MenuConfig::new().close_behavior(PopupCloseBehavior::CloseOnClickOutside);
        MenuBar::new().config(config).ui(ui, |ui| {
            for (index, title) in TITLES.iter().enumerate() {
                let text = underlined(ui, title, config::MENU_ACCESS_KEYS[index]);
                let open = self.open == Some(index);
                let (response, _) = egui::containers::menu::MenuButton::new(text).ui(ui, |ui| {
                    ui.set_min_width(MENU_MIN_WIDTH);
                    let mut focus = std::mem::take(&mut self.focus_first);
                    let mut first = |response: egui::Response| {
                        if std::mem::take(&mut focus) {
                            response.request_focus();
                        }
                        response
                    };
                    match index {
                        0 => self.personalization_menu(
                            ui,
                            settings,
                            fonts,
                            &mut commands,
                            &mut first,
                        ),
                        1 => playback_menu(ui, settings, &mut first),
                        _ => settings_menu(ui, &mut commands, &mut first),
                    }
                });
                if open {
                    self.open = None;
                    Popup::open_id(ui.ctx(), Popup::default_response_id(&response));
                    // El foco al primer ítem, en el próximo frame (el menú
                    // se dibuja recién ahí).
                    self.focus_first = true;
                    ui.ctx().request_repaint();
                }
            }
        });
        commands
    }

    /// Personalización: un submenú por cada uno de `SUBMENUS`. Los menús de
    /// egui abren un submenú con el mouse o Enter; acá se suma → para
    /// abrirlo (con el foco en su primer ítem) y ← para cerrarlo y volver
    /// a su botón, como en los menús de Windows. En un slider o en el
    /// texto del prompt, ← y → siguen siendo de ellos (`uses_arrows`).
    fn personalization_menu(
        &mut self,
        ui: &mut Ui,
        settings: &mut Settings,
        fonts: &[&str],
        commands: &mut Vec<Command>,
        first: &mut impl FnMut(egui::Response) -> egui::Response,
    ) {
        let (right, left) = ui.input(|i| {
            (
                i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::ArrowLeft),
            )
        });
        let ctx = ui.ctx().clone();
        let focused = ctx.memory(|m| m.focused());
        let mut focus = std::mem::take(&mut self.focus_sub);
        for (index, name) in SUBMENUS.iter().enumerate() {
            let (response, popup) = SubMenuButton::new(*name).ui(ui, |ui| {
                ui.set_min_width(MENU_MIN_WIDTH);
                let mut sub_first = |response: egui::Response| {
                    if std::mem::take(&mut focus) {
                        response.request_focus();
                    }
                    response
                };
                match index {
                    0 => self.theme_menu(ui, settings, &mut sub_first),
                    1 => font_menu(ui, settings, fonts, &mut sub_first),
                    2 => self.keys_menu(ui, settings, &mut sub_first),
                    3 => self.console_menu(ui, settings, commands, &mut sub_first),
                    _ => window_menu(ui, settings, commands, &mut sub_first),
                }
            });
            let response = if index == 0 {
                first(response)
            } else {
                response
            };
            let submenu = SubMenu::id_from_widget_id(response.id);
            if response.has_focus() {
                let open = MenuState::from_ui(ui, |state, _| state.open_item);
                if right && open != Some(submenu) {
                    // egui olvida el submenú abierto si no se dibujó el
                    // frame anterior: se lo marca como visto para que dure
                    // hasta el próximo, en el que se dibuja.
                    MenuState::mark_shown(&ctx, submenu);
                    MenuState::from_ui(ui, |state, _| state.open_item = Some(submenu));
                    // Sin el salto de foco de egui hacia la derecha.
                    ctx.memory_mut(|m| m.move_focus(FocusDirection::None));
                    self.focus_sub = true;
                    ctx.request_repaint();
                } else if response.clicked() && !response.clicked_by(egui::PointerButton::Primary) {
                    // Enter: egui lo abre; el foco, a su primer ítem.
                    self.focus_sub = true;
                    ctx.request_repaint();
                }
            }
            // ← con el foco dentro de este submenú: cerrarlo y volver a su
            // botón (no a donde egui encuentre algo a la izquierda).
            let inside = popup.as_ref().is_some_and(|popup| {
                focused
                    .filter(|&id| !uses_arrows(&ctx, id))
                    .and_then(|id| ctx.read_response(id))
                    .is_some_and(|r| popup.response.rect.contains_rect(r.rect))
            });
            if left && inside {
                MenuState::from_ui(ui, |state, _| state.open_item = None);
                ctx.memory_mut(|m| m.move_focus(FocusDirection::None));
                response.request_focus();
                ctx.request_repaint();
            }
        }
    }

    fn theme_menu(
        &mut self,
        ui: &mut Ui,
        settings: &mut Settings,
        first: &mut impl FnMut(egui::Response) -> egui::Response,
    ) {
        for (i, (name, palette)) in config::THEME_PRESETS.iter().enumerate() {
            let selected = settings.appearance.palette == *palette;
            let response = ui.add(Button::selectable(selected, *name));
            let response = if i == 0 { first(response) } else { response };
            if response.clicked() {
                settings.appearance.palette = *palette;
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Editar colores…").clicked() {
            self.colors_open = true;
            ui.close();
        }
        if ui.button("Restaurar colores").clicked() {
            settings.restore(Section::Colors);
            ui.close();
        }
    }

    fn keys_menu(
        &mut self,
        ui: &mut Ui,
        settings: &mut Settings,
        first: &mut impl FnMut(egui::Response) -> egui::Response,
    ) {
        if first(ui.button("Editar atajos…")).clicked() {
            self.keys_open = true;
            ui.close();
        }
        ui.separator();
        if ui.button("Restaurar atajos de la ventana").clicked() {
            settings.restore(Section::WindowKeys);
            ui.close();
        }
        if ui.button("Restaurar atajos globales").clicked() {
            settings.restore(Section::GlobalKeys);
            ui.close();
        }
    }

    fn console_menu(
        &mut self,
        ui: &mut Ui,
        settings: &mut Settings,
        commands: &mut Vec<Command>,
        first: &mut impl FnMut(egui::Response) -> egui::Response,
    ) {
        let console = &mut settings.console;
        ui.horizontal(|ui| {
            ui.label("Símbolo del prompt");
            let draft = self.prompt.get_or_insert_with(|| console.prompt.clone());
            let response = first(keeps_arrows(
                ui.add(
                    egui::TextEdit::singleline(draft)
                        .desired_width(80.0)
                        .char_limit(config::PROMPT_MAX_CHARS),
                ),
            ));
            if response.changed() {
                if let Some(prompt) = settings::valid_prompt(draft) {
                    console.prompt = prompt;
                }
            }
            if response.lost_focus() {
                self.prompt = None;
            }
        });
        keeps_arrows(
            ui.add(
                Slider::new(
                    &mut console.scrollback,
                    config::SCROLLBACK_MIN..=config::SCROLLBACK_MAX,
                )
                .logarithmic(true)
                .text("líneas guardadas"),
            ),
        );
        keeps_arrows(
            ui.add(
                Slider::new(
                    &mut console.history,
                    config::HISTORY_MIN..=config::HISTORY_MAX,
                )
                .logarithmic(true)
                .text("comandos en el historial"),
            ),
        );
        ui.checkbox(&mut console.timestamps, "Hora en cada línea");
        ui.separator();
        let clear = Button::new("Limpiar consola")
            .shortcut_text(shortcut_hint(settings, WindowAction::ClearConsole));
        if ui.add(clear).clicked() {
            commands.push(Command::ClearConsole);
            ui.close();
        }
        if ui.button("Restaurar consola").clicked() {
            settings.restore(Section::Console);
            self.prompt = None;
            ui.close();
        }
    }

    /// Diálogos abiertos desde los menús (colores, atajos).
    pub(super) fn dialogs(&mut self, ctx: &Context, settings: &mut Settings) {
        if self.colors_open {
            let mut open = true;
            egui::Window::new("Colores")
                .open(&mut open)
                .anchor(egui::Align2::CENTER_TOP, [0.0, 48.0])
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| colors_dialog(ui, settings));
            self.colors_open = open;
        }
        if self.keys_open {
            let mut open = true;
            egui::Window::new("Atajos")
                .open(&mut open)
                .anchor(egui::Align2::CENTER_TOP, [0.0, 48.0])
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| self.keys_dialog(ui, settings));
            if !open {
                self.close_dialogs();
            }
        }
    }

    fn keys_dialog(&mut self, ui: &mut Ui, settings: &mut Settings) {
        ui.label(
            RichText::new(
                "Clic en una combinación y apretá la nueva (Esc cancela). \
                 Los globales andan con la app minimizada.",
            )
            .small(),
        );
        ui.add_space(6.0);
        Grid::new("atajos")
            .num_columns(3)
            .spacing([18.0, 6.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(RichText::new("Acción").strong());
                ui.label(RichText::new("Ventana").strong());
                ui.label(RichText::new("Global").strong());
                ui.end_row();
                for action in WindowAction::ALL {
                    ui.label(action.label());
                    self.combo_cell(ui, settings, Target::Window(action));
                    match global_of(action) {
                        Some(global) => self.combo_cell(ui, settings, Target::Global(global)),
                        None => {
                            ui.label("");
                        }
                    }
                    ui.end_row();
                }
            });
        ui.add_space(6.0);
        if let Some((target, combo, owner)) = self.confirm {
            ui.label(format!(
                "{combo} ya la usa {}. ¿Pasarla a {}? El otro queda sin atajo.",
                owner.label(),
                target_name(target)
            ));
            ui.horizontal(|ui| {
                if ui.button("Reemplazar").clicked() {
                    settings.assign(target, Some(combo));
                    self.notice = Some((format!("✓ {combo} → {}", target_name(target)), false));
                    self.confirm = None;
                }
                if ui.button("Cancelar").clicked() {
                    self.confirm = None;
                }
            });
        }
        if let Some((text, error)) = &self.notice {
            let text_color = if *error {
                color(settings.appearance.palette.error)
            } else {
                color(settings.appearance.palette.accent)
            };
            ui.label(RichText::new(text).color(text_color));
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Restaurar de la ventana").clicked() {
                settings.restore(Section::WindowKeys);
            }
            if ui.button("Restaurar globales").clicked() {
                settings.restore(Section::GlobalKeys);
            }
        });
        ui.label(
            RichText::new(format!(
                "Restaurar todo: {} (fijo)",
                config::RESTORE_ALL_SHORTCUT
            ))
            .small(),
        );
    }

    fn combo_cell(&mut self, ui: &mut Ui, settings: &mut Settings, target: Target) {
        ui.horizontal(|ui| {
            let recording = self.recording == Some(target);
            let text = if recording {
                "apretá la combinación…".to_string()
            } else {
                settings
                    .combo(target)
                    .map_or_else(|| "—".to_string(), |c| c.to_string())
            };
            let button = Button::new(text)
                .selected(recording)
                .min_size([150.0, 0.0].into());
            if ui.add(button).clicked() {
                self.recording = if recording { None } else { Some(target) };
                self.confirm = None;
                self.notice = None;
                // Sin foco en ningún widget: la tecla es para grabar.
                if let Some(id) = ui.memory(|m| m.focused()) {
                    ui.memory_mut(|m| m.surrender_focus(id));
                }
            }
            if settings.combo(target).is_some()
                && ui
                    .add(Button::new("×").frame(false))
                    .on_hover_text("Quitar")
                    .clicked()
            {
                settings.assign(target, None);
            }
        });
    }
}

fn font_menu(
    ui: &mut Ui,
    settings: &mut Settings,
    fonts: &[&str],
    first: &mut impl FnMut(egui::Response) -> egui::Response,
) {
    for (i, name) in fonts.iter().enumerate() {
        let label = if *name == config::BUILTIN_FONT {
            "La de la app (egui)"
        } else {
            name
        };
        let selected = settings.appearance.font == *name;
        let response = ui.add(Button::selectable(selected, label));
        let response = if i == 0 { first(response) } else { response };
        if response.clicked() {
            settings.appearance.font = (*name).to_string();
        }
    }
    ui.separator();
    let size = &mut settings.appearance.font_size;
    keeps_arrows(
        ui.add(
            Slider::new(size, config::FONT_SIZE_MIN..=config::FONT_SIZE_MAX)
                .step_by(f64::from(config::FONT_SIZE_STEP))
                .suffix(" pt")
                .text("tamaño"),
        ),
    );
    for (action, text) in [
        (WindowAction::FontBigger, "Agrandar"),
        (WindowAction::FontSmaller, "Achicar"),
        (WindowAction::FontReset, "Tamaño por defecto"),
    ] {
        let button = Button::new(text).shortcut_text(shortcut_hint(settings, action));
        if ui.add(button).clicked() {
            apply_font_action(settings, action);
        }
    }
    ui.checkbox(
        &mut settings.appearance.bold_titles,
        "Negrita en título y tema que suena",
    );
    ui.separator();
    if ui.button("Restaurar fuente").clicked() {
        settings.restore(Section::Font);
        ui.close();
    }
}

/// Agrandar / Achicar / Tamaño por defecto, dentro del rango de config.
pub(super) fn apply_font_action(settings: &mut Settings, action: WindowAction) {
    let size = &mut settings.appearance.font_size;
    *size = match action {
        WindowAction::FontBigger => *size + config::FONT_SIZE_STEP,
        WindowAction::FontSmaller => *size - config::FONT_SIZE_STEP,
        _ => config::theme::FONT_SIZE,
    }
    .clamp(config::FONT_SIZE_MIN, config::FONT_SIZE_MAX);
}

fn playback_menu(
    ui: &mut Ui,
    settings: &mut Settings,
    first: &mut impl FnMut(egui::Response) -> egui::Response,
) {
    let playback = &mut settings.playback;
    first(
        ui.add(
            Slider::new(
                &mut playback.volume_step,
                config::VOLUME_STEP_MIN..=config::VOLUME_STEP_MAX,
            )
            .suffix(" %")
            .text("paso de volumen"),
        ),
    );
    let mut secs = playback.previous_threshold.as_secs_f32();
    let threshold = Slider::new(&mut secs, 0.0..=config::PREVIOUS_THRESHOLD_MAX_SECS as f32)
        .step_by(0.5)
        .suffix(" s")
        .text("«anterior» reinicia el tema después de");
    if ui.add(threshold).changed() {
        playback.previous_threshold = Duration::from_secs_f32(secs);
    }
    ui.separator();
    ui.label("Calidad de audio (desde el próximo play)");
    let current = bitrate_kbps(playback.bitrate);
    for &kbps in config::BITRATES_KBPS {
        if ui.radio(current == kbps, format!("{kbps} kbps")).clicked() {
            if let Some(bitrate) = bitrate_from_kbps(kbps) {
                playback.bitrate = bitrate;
            }
        }
    }
    ui.separator();
    if ui.button("Restaurar reproducción").clicked() {
        settings.restore(Section::Playback);
        ui.close();
    }
}

fn window_menu(
    ui: &mut Ui,
    settings: &mut Settings,
    commands: &mut Vec<Command>,
    first: &mut impl FnMut(egui::Response) -> egui::Response,
) {
    let window = &mut settings.window;
    first(ui.checkbox(
        &mut window.always_on_top,
        "Siempre visible (encima de las demás)",
    ));
    ui.checkbox(
        &mut window.remember_geometry,
        "Recordar tamaño y posición al cerrar",
    );
    ui.separator();
    if ui.button("Restaurar tamaño de la ventana").clicked() {
        commands.push(Command::ResetGeometry);
        ui.close();
    }
    if ui.button("Restaurar ventana").clicked() {
        settings.restore(Section::Window);
        commands.push(Command::ResetGeometry);
        ui.close();
    }
}

fn settings_menu(
    ui: &mut Ui,
    commands: &mut Vec<Command>,
    first: &mut impl FnMut(egui::Response) -> egui::Response,
) {
    let items = [
        (
            Button::new(format!("Abrir {}", config::SETTINGS_FILE)),
            Command::OpenFile,
        ),
        (Button::new("Recargar desde el archivo"), Command::Reload),
        (
            Button::new("Restaurar todo").shortcut_text(config::RESTORE_ALL_SHORTCUT.to_string()),
            Command::RestoreAll,
        ),
    ];
    for (i, (button, command)) in items.into_iter().enumerate() {
        if i == 2 {
            ui.separator();
        }
        let response = ui.add(button);
        let response = if i == 0 { first(response) } else { response };
        if response.clicked() {
            commands.push(command);
            ui.close();
        }
    }
}

fn colors_dialog(ui: &mut Ui, settings: &mut Settings) {
    ui.horizontal_wrapped(|ui| {
        for (name, palette) in config::THEME_PRESETS {
            if ui
                .add(Button::selectable(
                    settings.appearance.palette == *palette,
                    *name,
                ))
                .clicked()
            {
                settings.appearance.palette = *palette;
            }
        }
    });
    ui.add_space(6.0);
    Grid::new("colores")
        .num_columns(3)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            for slot in ColorSlot::ALL {
                ui.label(slot.label());
                ui.color_edit_button_srgb(settings.appearance.palette.get_mut(slot));
                let [r, g, b] = settings.appearance.palette.get(slot);
                ui.label(RichText::new(format!("#{r:02X}{g:02X}{b:02X}")).small());
                ui.end_row();
            }
        });
    ui.separator();
    if ui.button("Restaurar colores").clicked() {
        settings.restore(Section::Colors);
    }
}

/// Marca `response` (un slider o un campo de texto) como un widget que usa
/// ← y → para sí: esas flechas no cierran su submenú.
fn keeps_arrows(response: egui::Response) -> egui::Response {
    response
        .ctx
        .data_mut(|d| d.insert_temp(response.id.with(USES_ARROWS), true));
    response
}

fn uses_arrows(ctx: &Context, id: Id) -> bool {
    ctx.data(|d| d.get_temp::<bool>(id.with(USES_ARROWS)))
        .unwrap_or(false)
}

const USES_ARROWS: &str = "usa-flechas";

/// El atajo de ventana de `action` como texto, o nada.
fn shortcut_hint(settings: &Settings, action: WindowAction) -> String {
    settings
        .combo(Target::Window(action))
        .map(|c| c.to_string())
        .unwrap_or_default()
}

fn target_name(target: Target) -> &'static str {
    match target {
        Target::Window(action) => action.label(),
        Target::Global(action) => action.describe(),
    }
}

/// La acción global que corresponde a una de la ventana, si hay.
fn global_of(action: WindowAction) -> Option<GlobalAction> {
    GlobalAction::ALL.into_iter().find(|global| {
        matches!(
            (action, global),
            (WindowAction::TogglePause, GlobalAction::TogglePause)
                | (WindowAction::Next, GlobalAction::Next)
                | (WindowAction::Prev, GlobalAction::Prev)
                | (WindowAction::Stop, GlobalAction::Stop)
                | (WindowAction::VolumeUp, GlobalAction::VolumeUp)
                | (WindowAction::VolumeDown, GlobalAction::VolumeDown)
        )
    })
}

/// `title` con la primera `letter` subrayada (como los menús de Windows).
fn underlined(ui: &Ui, title: &str, letter: char) -> LayoutJob {
    let text_color: Color32 = ui.visuals().text_color();
    let font = egui::TextStyle::Button.resolve(ui.style());
    let plain = TextFormat::simple(font.clone(), text_color);
    let mut underline = plain.clone();
    underline.underline = Stroke::new(1.0_f32, text_color);
    let mut job = LayoutJob::default();
    let at = title
        .char_indices()
        .find(|(_, c)| c.eq_ignore_ascii_case(&letter))
        .map(|(i, c)| (i, i + c.len_utf8()));
    match at {
        Some((start, end)) => {
            job.append(&title[..start], 0.0, plain.clone());
            job.append(&title[start..end], 0.0, underline);
            job.append(&title[end..], 0.0, plain);
        }
        None => job.append(title, 0.0, plain),
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_menu_tiene_su_letra_en_el_titulo() {
        for (title, letter) in TITLES.iter().zip(config::MENU_ACCESS_KEYS) {
            assert!(
                title.chars().any(|c| c.eq_ignore_ascii_case(&letter)),
                "{title} sin {letter}"
            );
        }
        let mut letters = config::MENU_ACCESS_KEYS.to_vec();
        letters.dedup();
        assert_eq!(letters.len(), TITLES.len());
    }

    #[test]
    fn personalizacion_junta_los_menus_de_antes() {
        assert_eq!(TITLES[0], "Personalización");
        assert_eq!(SUBMENUS, ["Tema", "Fuente", "Atajos", "Consola", "Ventana"]);
        for sub in SUBMENUS {
            assert!(!TITLES.contains(&sub), "{sub} sigue en la barra");
        }
    }

    #[test]
    fn agrandar_y_achicar_sin_salirse_del_rango() {
        let mut s = Settings::default();
        apply_font_action(&mut s, WindowAction::FontBigger);
        assert_eq!(
            s.appearance.font_size,
            config::theme::FONT_SIZE + config::FONT_SIZE_STEP
        );
        for _ in 0..100 {
            apply_font_action(&mut s, WindowAction::FontBigger);
        }
        assert_eq!(s.appearance.font_size, config::FONT_SIZE_MAX);
        for _ in 0..100 {
            apply_font_action(&mut s, WindowAction::FontSmaller);
        }
        assert_eq!(s.appearance.font_size, config::FONT_SIZE_MIN);
        apply_font_action(&mut s, WindowAction::FontReset);
        assert_eq!(s.appearance.font_size, config::theme::FONT_SIZE);
    }

    #[test]
    fn cada_accion_global_tiene_su_fila() {
        for global in GlobalAction::ALL {
            assert!(
                WindowAction::ALL
                    .iter()
                    .any(|&a| global_of(a) == Some(global)),
                "{global:?}"
            );
        }
    }
}
