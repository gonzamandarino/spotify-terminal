//! Ajustes de la app de escritorio (spec 007): lo que el usuario cambia
//! desde la barra de menús, guardado en `config::SETTINGS_FILE` dentro de
//! la carpeta de datos. Sin egui ni Windows: se testea sin ventana.
//!
//! El archivo guarda solo lo que difiere del default (así un default nuevo
//! le llega al usuario) y se puede editar a mano: cada clave se valida por
//! separado y una inválida no arrastra a las demás.

use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::Path,
    time::Duration,
};

use librespot_playback::config::Bitrate;
use serde_json::{Map, Value, json};

use super::combo::{Combo, Key};
use crate::{
    app::engine::{GlobalAction, PlaybackSettings},
    config,
};

/// Colores de la app (RGB).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Palette {
    pub(crate) background: [u8; 3],
    pub(crate) panel: [u8; 3],
    pub(crate) border: [u8; 3],
    pub(crate) accent: [u8; 3],
    pub(crate) text: [u8; 3],
    pub(crate) text_strong: [u8; 3],
    pub(crate) secondary: [u8; 3],
    pub(crate) warning: [u8; 3],
    pub(crate) error: [u8; 3],
    /// Fondo del botón de cerrar al pasar el mouse.
    pub(crate) close_hover: [u8; 3],
}

/// Un color de `Palette`, para recorrerlos (menú, archivo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorSlot {
    Background,
    Panel,
    Border,
    Accent,
    Text,
    TextStrong,
    Secondary,
    Warning,
    Error,
    CloseHover,
}

impl ColorSlot {
    pub(crate) const ALL: [ColorSlot; 10] = [
        ColorSlot::Background,
        ColorSlot::Panel,
        ColorSlot::Border,
        ColorSlot::Accent,
        ColorSlot::Text,
        ColorSlot::TextStrong,
        ColorSlot::Secondary,
        ColorSlot::Warning,
        ColorSlot::Error,
        ColorSlot::CloseHover,
    ];

    /// Clave en el archivo.
    fn key(self) -> &'static str {
        match self {
            ColorSlot::Background => "fondo",
            ColorSlot::Panel => "paneles",
            ColorSlot::Border => "bordes",
            ColorSlot::Accent => "acento",
            ColorSlot::Text => "texto",
            ColorSlot::TextStrong => "texto_fuerte",
            ColorSlot::Secondary => "secundario",
            ColorSlot::Warning => "avisos",
            ColorSlot::Error => "errores",
            ColorSlot::CloseHover => "cerrar_hover",
        }
    }

    /// Nombre en el menú.
    pub(crate) fn label(self) -> &'static str {
        match self {
            ColorSlot::Background => "Fondo",
            ColorSlot::Panel => "Paneles",
            ColorSlot::Border => "Bordes",
            ColorSlot::Accent => "Acento (prompt, tema que suena, éxito)",
            ColorSlot::Text => "Texto",
            ColorSlot::TextStrong => "Texto resaltado",
            ColorSlot::Secondary => "Texto secundario",
            ColorSlot::Warning => "Avisos",
            ColorSlot::Error => "Errores",
            ColorSlot::CloseHover => "Botón cerrar (mouse encima)",
        }
    }
}

impl Palette {
    pub(crate) fn get(&self, slot: ColorSlot) -> [u8; 3] {
        let mut copy = *self;
        *copy.get_mut(slot)
    }

    pub(crate) fn get_mut(&mut self, slot: ColorSlot) -> &mut [u8; 3] {
        match slot {
            ColorSlot::Background => &mut self.background,
            ColorSlot::Panel => &mut self.panel,
            ColorSlot::Border => &mut self.border,
            ColorSlot::Accent => &mut self.accent,
            ColorSlot::Text => &mut self.text,
            ColorSlot::TextStrong => &mut self.text_strong,
            ColorSlot::Secondary => &mut self.secondary,
            ColorSlot::Warning => &mut self.warning,
            ColorSlot::Error => &mut self.error,
            ColorSlot::CloseHover => &mut self.close_hover,
        }
    }
}

/// Lo que puede hacer un atajo de la ventana (solo con la ventana
/// enfocada). `Esc`, `↑`, `↓`, `Tab` y `Enter` no están: son de edición y
/// quedan fijos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum WindowAction {
    TogglePause,
    Next,
    Prev,
    Stop,
    Shuffle,
    VolumeUp,
    VolumeDown,
    ClearConsole,
    FontBigger,
    FontSmaller,
    FontReset,
}

impl WindowAction {
    pub(crate) const ALL: [WindowAction; 11] = [
        WindowAction::TogglePause,
        WindowAction::Next,
        WindowAction::Prev,
        WindowAction::Stop,
        WindowAction::Shuffle,
        WindowAction::VolumeUp,
        WindowAction::VolumeDown,
        WindowAction::ClearConsole,
        WindowAction::FontBigger,
        WindowAction::FontSmaller,
        WindowAction::FontReset,
    ];

    fn key(self) -> &'static str {
        match self {
            WindowAction::TogglePause => "pausa",
            WindowAction::Next => "siguiente",
            WindowAction::Prev => "anterior",
            WindowAction::Stop => "stop",
            WindowAction::Shuffle => "shuffle",
            WindowAction::VolumeUp => "subir_volumen",
            WindowAction::VolumeDown => "bajar_volumen",
            WindowAction::ClearConsole => "limpiar_consola",
            WindowAction::FontBigger => "agrandar_letra",
            WindowAction::FontSmaller => "achicar_letra",
            WindowAction::FontReset => "letra_por_defecto",
        }
    }

    /// Para el menú y `help`.
    pub(crate) fn label(self) -> &'static str {
        match self {
            WindowAction::TogglePause => "pausa / reanudar",
            WindowAction::Next => "siguiente",
            WindowAction::Prev => "anterior",
            WindowAction::Stop => "stop",
            WindowAction::Shuffle => "shuffle sí / no",
            WindowAction::VolumeUp => "subir volumen",
            WindowAction::VolumeDown => "bajar volumen",
            WindowAction::ClearConsole => "limpiar consola",
            WindowAction::FontBigger => "agrandar letra",
            WindowAction::FontSmaller => "achicar letra",
            WindowAction::FontReset => "tamaño de letra por defecto",
        }
    }
}

fn global_key(action: GlobalAction) -> &'static str {
    match action {
        GlobalAction::TogglePause => "pausa",
        GlobalAction::Next => "siguiente",
        GlobalAction::Prev => "anterior",
        GlobalAction::Stop => "stop",
        GlobalAction::VolumeUp => "subir_volumen",
        GlobalAction::VolumeDown => "bajar_volumen",
    }
}

/// De quién es un atajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    Window(WindowAction),
    Global(GlobalAction),
}

impl Target {
    /// "atajo de ventana «siguiente»".
    pub(crate) fn label(self) -> String {
        match self {
            Target::Window(action) => format!("el atajo de ventana «{}»", action.label()),
            Target::Global(action) => format!("el atajo global «{}»", action.describe()),
        }
    }
}

/// Resultado de `check_combo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComboCheck {
    Ok,
    /// No se puede usar nunca para ese atajo; el texto dice por qué.
    Reserved(&'static str),
    /// Ya la usa otro atajo de la app.
    UsedBy(Target),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Appearance {
    pub(crate) palette: Palette,
    /// Nombre de `config::FONT_CATALOG` o `config::BUILTIN_FONT`.
    pub(crate) font: String,
    pub(crate) font_size: f32,
    /// Título y tema que suena en negrita.
    pub(crate) bold_titles: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConsoleSettings {
    pub(crate) prompt: String,
    pub(crate) scrollback: usize,
    pub(crate) history: usize,
    /// Hora al principio de cada línea de salida.
    pub(crate) timestamps: bool,
}

/// Posición (px físicos) y tamaño (puntos lógicos) de la ventana.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Geometry {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WindowSettings {
    pub(crate) always_on_top: bool,
    pub(crate) remember_geometry: bool,
    /// Última posición y tamaño; `None` = los de `config`.
    pub(crate) geometry: Option<Geometry>,
}

/// Todos los ajustes.
///
/// Invariante (la cumplen `default()` y `from_json`, y la mantienen
/// `assign` y `restore`): cada valor está dentro de su rango de `config`,
/// ningún atajo es reservado para su tipo (`check_combo`), y ninguna
/// combinación la usan dos atajos (de ventana o globales).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Settings {
    pub(crate) appearance: Appearance,
    pub(crate) window_keys: BTreeMap<WindowAction, Option<Combo>>,
    pub(crate) global_keys: BTreeMap<GlobalAction, Option<Combo>>,
    pub(crate) playback: PlaybackSettings,
    pub(crate) console: ConsoleSettings,
    pub(crate) window: WindowSettings,
}

impl Default for Settings {
    fn default() -> Settings {
        let default_combo = |action| {
            config::WINDOW_SHORTCUTS
                .iter()
                .find(|(_, a)| *a == action)
                .map(|(combo, _)| *combo)
        };
        let default_global = |action| {
            config::GLOBAL_SHORTCUTS
                .iter()
                .find(|(_, a)| *a == action)
                .map(|(combo, _)| *combo)
        };
        Settings {
            appearance: Appearance {
                palette: config::SPOTIFY_DARK,
                font: config::DEFAULT_FONT.into(),
                font_size: config::theme::FONT_SIZE,
                bold_titles: true,
            },
            window_keys: WindowAction::ALL
                .iter()
                .map(|&a| (a, default_combo(a)))
                .collect(),
            global_keys: GlobalAction::ALL
                .iter()
                .map(|&a| (a, default_global(a)))
                .collect(),
            playback: PlaybackSettings::default(),
            console: ConsoleSettings {
                prompt: config::PROMPT_DEFAULT.into(),
                scrollback: config::SCROLLBACK_LINES,
                history: config::HISTORY_LEN,
                timestamps: false,
            },
            window: WindowSettings {
                always_on_top: false,
                remember_geometry: true,
                geometry: None,
            },
        }
    }
}

/// Grupo de ajustes: una sección del archivo y un "Restaurar …" del menú.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Section {
    Colors,
    Font,
    WindowKeys,
    GlobalKeys,
    Playback,
    Console,
    Window,
}

impl Section {
    const ALL: [Section; 7] = [
        Section::Colors,
        Section::Font,
        Section::WindowKeys,
        Section::GlobalKeys,
        Section::Playback,
        Section::Console,
        Section::Window,
    ];

    fn key(self) -> &'static str {
        match self {
            Section::Colors => "tema",
            Section::Font => "fuente",
            Section::WindowKeys => "atajos_ventana",
            Section::GlobalKeys => "atajos_globales",
            Section::Playback => "reproduccion",
            Section::Console => "consola",
            Section::Window => "ventana",
        }
    }
}

impl Settings {
    pub(crate) fn combo(&self, target: Target) -> Option<Combo> {
        match target {
            Target::Window(action) => self.window_keys.get(&action).copied().flatten(),
            Target::Global(action) => self.global_keys.get(&action).copied().flatten(),
        }
    }

    fn slot(&mut self, target: Target) -> &mut Option<Combo> {
        match target {
            Target::Window(action) => self.window_keys.entry(action).or_default(),
            Target::Global(action) => self.global_keys.entry(action).or_default(),
        }
    }

    fn targets(&self) -> impl Iterator<Item = (Target, Option<Combo>)> + '_ {
        let window = self
            .window_keys
            .iter()
            .map(|(&a, &c)| (Target::Window(a), c));
        let global = self
            .global_keys
            .iter()
            .map(|(&a, &c)| (Target::Global(a), c));
        window.chain(global)
    }

    /// Si `combo` se puede usar para `target`.
    ///
    /// - Post: `Reserved` si no se puede usar nunca para ese tipo de atajo
    ///   (el atajo fijo de restaurar; en la ventana, las combinaciones de
    ///   edición y las que no tienen Ctrl ni Alt, salvo F1-F12; un global
    ///   sin Ctrl ni Alt). Si no, `UsedBy` si otro atajo ya la tiene. Si
    ///   no, `Ok`. La combinación actual de `target` cuenta como libre.
    /// - No debe: cambiar nada.
    pub(crate) fn check_combo(&self, target: Target, combo: Combo) -> ComboCheck {
        if let Some(reason) = reserved(target, combo) {
            return ComboCheck::Reserved(reason);
        }
        match self
            .targets()
            .find(|&(other, c)| other != target && c == Some(combo))
        {
            Some((other, _)) => ComboCheck::UsedBy(other),
            None => ComboCheck::Ok,
        }
    }

    /// Pone `combo` (o ninguno) a `target`. Si otro atajo la tenía, se la
    /// saca (queda sin atajo) y lo devuelve, para avisar.
    ///
    /// - Pre: `combo` no es `Reserved` para `target` (ver `check_combo`;
    ///   el menú pregunta antes).
    pub(crate) fn assign(&mut self, target: Target, combo: Option<Combo>) -> Option<Target> {
        let taken = combo.and_then(|combo| match self.check_combo(target, combo) {
            ComboCheck::UsedBy(other) => Some(other),
            _ => None,
        });
        if let Some(other) = taken {
            *self.slot(other) = None;
        }
        *self.slot(target) = combo;
        taken
    }

    /// Vuelve una sección a fábrica. Al restaurar atajos, si uno de fábrica
    /// choca con uno cambiado de la otra sección, el de la otra queda sin
    /// atajo (para mantener la invariante).
    pub(crate) fn restore(&mut self, section: Section) {
        let defaults = Settings::default();
        match section {
            Section::Colors => self.appearance.palette = defaults.appearance.palette,
            Section::Font => {
                let palette = self.appearance.palette;
                self.appearance = defaults.appearance;
                self.appearance.palette = palette;
            }
            Section::WindowKeys | Section::GlobalKeys => {
                let restored: Vec<(Target, Option<Combo>)> = defaults
                    .targets()
                    .filter(|(target, _)| {
                        matches!(
                            (section, target),
                            (Section::WindowKeys, Target::Window(_))
                                | (Section::GlobalKeys, Target::Global(_))
                        )
                    })
                    .collect();
                for &(target, _) in &restored {
                    *self.slot(target) = None;
                }
                for (target, combo) in restored {
                    self.assign(target, combo);
                }
            }
            Section::Playback => self.playback = defaults.playback,
            Section::Console => self.console = defaults.console,
            Section::Window => self.window = defaults.window,
        }
    }

    /// Los ajustes de un archivo ya leído.
    ///
    /// - Post: nunca falla. Lo que no es un objeto → todo de fábrica. Cada
    ///   clave inválida (tipo, formato o rango) o desconocida → esa clave
    ///   de fábrica y un aviso que la nombra; el resto se respeta. Si dos
    ///   atajos quedan con la misma combinación, gana el escrito en el
    ///   archivo (si ambos lo están, el de ventana, o el de la primera
    ///   acción) y el otro queda sin atajo, con aviso. Cumple la invariante
    ///   de `Settings`.
    pub(crate) fn from_json(root: &Value) -> (Settings, Vec<String>) {
        let mut settings = Settings::default();
        let mut warnings = Vec::new();
        let Some(root) = root.as_object() else {
            warnings.push(warning(
                "no es un objeto JSON: se usan los ajustes de fábrica",
            ));
            return (settings, warnings);
        };
        let fields = fields();
        let mut explicit = Vec::new();
        for (section_key, section) in root {
            if !Section::ALL.iter().any(|s| s.key() == section_key) {
                warnings.push(warning(&format!(
                    "clave desconocida `{section_key}`, se ignora"
                )));
                continue;
            }
            let Some(section) = section.as_object() else {
                warnings.push(warning(&format!(
                    "`{section_key}` no es un objeto: se usa de fábrica"
                )));
                continue;
            };
            for (key, value) in section {
                let Some(field) = fields
                    .iter()
                    .find(|f| f.section.key() == section_key && f.key == *key)
                else {
                    warnings.push(warning(&format!(
                        "clave desconocida `{section_key}.{key}`, se ignora"
                    )));
                    continue;
                };
                match (field.set)(&mut settings, value) {
                    Ok(()) => explicit.extend(field.target),
                    Err(e) => warnings.push(warning(&format!(
                        "`{section_key}.{key}`: {e}; se usa el de fábrica"
                    ))),
                }
            }
        }
        fix_conflicts(&mut settings, &explicit, &mut warnings);
        (settings, warnings)
    }

    /// Lo que se guarda: solo las claves que difieren de fábrica.
    pub(crate) fn to_json(&self) -> Value {
        let defaults = Settings::default();
        let mut root = Map::new();
        for field in fields() {
            let value = (field.get)(self);
            if value == (field.get)(&defaults) {
                continue;
            }
            if let Value::Object(section) = root
                .entry(field.section.key())
                .or_insert_with(|| Value::Object(Map::new()))
            {
                section.insert(field.key, value);
            }
        }
        Value::Object(root)
    }
}

/// Lee los ajustes de `dir`.
///
/// - Post: nunca falla (ver `Settings::from_json`). Sin archivo → fábrica
///   sin avisos; ilegible o JSON roto → fábrica con un aviso.
/// - No debe: escribir nada (un archivo roto se deja como está hasta que
///   el usuario cambie algo, para no perder lo que escribió a mano).
pub(crate) fn load(dir: &Path) -> (Settings, Vec<String>) {
    let text = match fs::read_to_string(dir.join(config::SETTINGS_FILE)) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return (Settings::default(), Vec::new()),
        Err(e) => {
            let text = format!("no se pudo leer ({e}): se usan los ajustes de fábrica");
            return (Settings::default(), vec![warning(&text)]);
        }
    };
    // El Bloc de notas y PowerShell 5.1 guardan UTF-8 con BOM.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    match serde_json::from_str::<Value>(text) {
        Ok(root) => Settings::from_json(&root),
        Err(e) => {
            let text = format!(
                "no es JSON válido (línea {}): se usan los ajustes de fábrica",
                e.line()
            );
            (Settings::default(), vec![warning(&text)])
        }
    }
}

/// Guarda `settings` en `dir` (la crea si hace falta).
///
/// - Post: el archivo tiene solo lo que difiere de fábrica (`{}` si nada).
///   Se escribe a un temporal y se renombra: un corte a mitad deja el
///   archivo anterior entero, no uno cortado.
/// - Errores: los de I/O.
pub(crate) fn save(dir: &Path, settings: &Settings) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(&settings.to_json()).map_err(io::Error::other)?;
    let path = dir.join(config::SETTINGS_FILE);
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, &path)
}

fn warning(text: &str) -> String {
    format!("⚠ {}: {text}.", config::SETTINGS_FILE)
}

/// Por qué `combo` no se puede usar nunca para `target`, o `None`.
fn reserved(target: Target, combo: Combo) -> Option<&'static str> {
    if combo == config::RESTORE_ALL_SHORTCUT {
        return Some("es el atajo fijo de «restaurar todo»");
    }
    match target {
        Target::Window(_) if config::EDITING_SHORTCUTS.contains(&combo) => {
            Some("la usa la línea de entrada para editar")
        }
        Target::Window(_) if is_menu_access(combo) => Some("abre un menú de la barra"),
        Target::Window(_) if !combo.has_command_modifier() && !matches!(combo.key, Key::F(_)) => {
            Some("sin Ctrl ni Alt taparía lo que se escribe")
        }
        Target::Global(_) if !combo.has_command_modifier() => {
            Some("un atajo global sin Ctrl ni Alt taparía esa tecla en todas las apps")
        }
        _ => None,
    }
}

/// `Alt`+letra de un menú de la barra.
pub(crate) fn is_menu_access(combo: Combo) -> bool {
    combo.alt
        && !combo.ctrl
        && !combo.shift
        && matches!(combo.key, Key::Char(c) if config::MENU_ACCESS_KEYS.contains(&c))
}

/// Deja cada combinación en un solo atajo: primero los de `explicit` (los
/// escritos en el archivo), después el resto; dentro de cada grupo, los de
/// ventana antes que los globales, en el orden de sus acciones.
fn fix_conflicts(settings: &mut Settings, explicit: &[Target], warnings: &mut Vec<String>) {
    let all: Vec<Target> = settings.targets().map(|(t, _)| t).collect();
    let (mut order, rest): (Vec<Target>, Vec<Target>) =
        all.into_iter().partition(|t| explicit.contains(t));
    order.extend(rest);
    let mut owner: HashMap<Combo, Target> = HashMap::new();
    for target in order {
        let Some(combo) = settings.combo(target) else {
            continue;
        };
        match owner.get(&combo) {
            Some(first) => {
                warnings.push(warning(&format!(
                    "{combo} ya la usa {}: {} queda sin atajo",
                    first.label(),
                    target.label()
                )));
                *settings.slot(target) = None;
            }
            None => {
                owner.insert(combo, target);
            }
        }
    }
}

type Getter = Box<dyn Fn(&Settings) -> Value>;
type Setter = Box<dyn Fn(&mut Settings, &Value) -> Result<(), String>>;

/// Una clave del archivo: cómo se lee y se escribe.
struct Field {
    section: Section,
    key: String,
    /// Si es un atajo, de quién (para resolver choques).
    target: Option<Target>,
    get: Getter,
    set: Setter,
}

fn field(
    section: Section,
    key: &str,
    get: impl Fn(&Settings) -> Value + 'static,
    set: impl Fn(&mut Settings, &Value) -> Result<(), String> + 'static,
) -> Field {
    Field {
        section,
        key: key.into(),
        target: None,
        get: Box::new(get),
        set: Box::new(set),
    }
}

fn combo_field(section: Section, key: &str, target: Target) -> Field {
    Field {
        target: Some(target),
        ..field(
            section,
            key,
            move |s| {
                s.combo(target)
                    .map_or(Value::Null, |c| json!(c.to_string()))
            },
            move |s, v| {
                let combo = match v {
                    Value::Null => None,
                    Value::String(text) => {
                        let combo: Combo = text.parse()?;
                        if let Some(reason) = reserved(target, combo) {
                            return Err(format!("{combo} no se puede usar: {reason}"));
                        }
                        Some(combo)
                    }
                    _ => return Err("tiene que ser un texto como \"Ctrl+Alt+P\" o null".into()),
                };
                *s.slot(target) = combo;
                Ok(())
            },
        )
    }
}

/// Todas las claves del archivo, en el orden en que se guardan.
fn fields() -> Vec<Field> {
    let mut fields = Vec::new();
    for slot in ColorSlot::ALL {
        fields.push(field(
            Section::Colors,
            slot.key(),
            move |s| json!(hex(s.appearance.palette.get(slot))),
            move |s, v| {
                *s.appearance.palette.get_mut(slot) = parse_hex(v)?;
                Ok(())
            },
        ));
    }
    fields.push(field(
        Section::Font,
        "familia",
        |s| json!(s.appearance.font),
        |s, v| {
            s.appearance.font = parse_font(v)?;
            Ok(())
        },
    ));
    fields.push(field(
        Section::Font,
        "tamaño",
        |s| json!(s.appearance.font_size),
        |s, v| {
            let size = number_in(
                v,
                config::FONT_SIZE_MIN.into(),
                config::FONT_SIZE_MAX.into(),
            )?;
            // Dentro del rango de config (f32): no pierde nada.
            s.appearance.font_size = size as f32;
            Ok(())
        },
    ));
    fields.push(field(
        Section::Font,
        "negrita_en_titulos",
        |s| json!(s.appearance.bold_titles),
        |s, v| {
            s.appearance.bold_titles = boolean(v)?;
            Ok(())
        },
    ));
    for action in WindowAction::ALL {
        fields.push(combo_field(
            Section::WindowKeys,
            action.key(),
            Target::Window(action),
        ));
    }
    for action in GlobalAction::ALL {
        fields.push(combo_field(
            Section::GlobalKeys,
            global_key(action),
            Target::Global(action),
        ));
    }
    fields.push(field(
        Section::Playback,
        "paso_volumen",
        |s| json!(s.playback.volume_step),
        |s, v| {
            let step = integer_in(
                v,
                config::VOLUME_STEP_MIN.into(),
                config::VOLUME_STEP_MAX.into(),
            )?;
            s.playback.volume_step = u8::try_from(step).map_err(|e| e.to_string())?;
            Ok(())
        },
    ));
    fields.push(field(
        Section::Playback,
        "umbral_anterior_s",
        |s| json!(s.playback.previous_threshold.as_secs_f64()),
        |s, v| {
            let secs = number_in(v, 0.0, config::PREVIOUS_THRESHOLD_MAX_SECS as f64)?;
            s.playback.previous_threshold = Duration::from_secs_f64(secs);
            Ok(())
        },
    ));
    fields.push(field(
        Section::Playback,
        "calidad_kbps",
        |s| json!(bitrate_kbps(s.playback.bitrate)),
        |s, v| {
            let kbps = v.as_u64().unwrap_or(0);
            s.playback.bitrate = u16::try_from(kbps)
                .ok()
                .and_then(bitrate_from_kbps)
                .ok_or_else(|| format!("tiene que ser {}", kbps_choices()))?;
            Ok(())
        },
    ));
    fields.push(field(
        Section::Console,
        "prompt",
        |s| json!(s.console.prompt),
        |s, v| {
            s.console.prompt = parse_prompt(v)?;
            Ok(())
        },
    ));
    fields.push(field(
        Section::Console,
        "scrollback",
        |s| json!(s.console.scrollback),
        |s, v| {
            s.console.scrollback = usize_in(v, config::SCROLLBACK_MIN, config::SCROLLBACK_MAX)?;
            Ok(())
        },
    ));
    fields.push(field(
        Section::Console,
        "historial",
        |s| json!(s.console.history),
        |s, v| {
            s.console.history = usize_in(v, config::HISTORY_MIN, config::HISTORY_MAX)?;
            Ok(())
        },
    ));
    fields.push(field(
        Section::Console,
        "hora",
        |s| json!(s.console.timestamps),
        |s, v| {
            s.console.timestamps = boolean(v)?;
            Ok(())
        },
    ));
    fields.push(field(
        Section::Window,
        "siempre_visible",
        |s| json!(s.window.always_on_top),
        |s, v| {
            s.window.always_on_top = boolean(v)?;
            Ok(())
        },
    ));
    fields.push(field(
        Section::Window,
        "recordar_tamaño",
        |s| json!(s.window.remember_geometry),
        |s, v| {
            s.window.remember_geometry = boolean(v)?;
            Ok(())
        },
    ));
    fields.push(field(
        Section::Window,
        "geometria",
        |s| {
            s.window.geometry.map_or(
                Value::Null,
                |g| json!({"x": g.x, "y": g.y, "ancho": g.width, "alto": g.height}),
            )
        },
        |s, v| {
            s.window.geometry = parse_geometry(v)?;
            Ok(())
        },
    ));
    fields
}

fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

fn parse_hex(value: &Value) -> Result<[u8; 3], String> {
    let bad = || "tiene que ser un color como \"#1DB954\"".to_string();
    let digits = value
        .as_str()
        .and_then(|t| t.trim().strip_prefix('#'))
        .filter(|d| d.len() == 6 && d.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(bad)?;
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).map_err(|_| bad());
    Ok([byte(0)?, byte(2)?, byte(4)?])
}

fn parse_font(value: &Value) -> Result<String, String> {
    let name = value.as_str().map(str::trim).unwrap_or_default();
    config::FONT_CATALOG
        .iter()
        .map(|f| f.name)
        .chain([config::BUILTIN_FONT])
        .find(|known| known.eq_ignore_ascii_case(name))
        .map(str::to_string)
        .ok_or_else(|| {
            let known: Vec<&str> = config::FONT_CATALOG
                .iter()
                .map(|f| f.name)
                .chain([config::BUILTIN_FONT])
                .collect();
            format!("fuente desconocida; se puede usar: {}", known.join(", "))
        })
}

/// El prompt recortado, si sirve (1 a `config::PROMPT_MAX_CHARS`
/// caracteres).
pub(crate) fn valid_prompt(text: &str) -> Option<String> {
    parse_prompt(&json!(text)).ok()
}

fn parse_prompt(value: &Value) -> Result<String, String> {
    let prompt = value.as_str().map(str::trim).unwrap_or_default();
    let chars = prompt.chars().count();
    if chars == 0 || chars > config::PROMPT_MAX_CHARS {
        return Err(format!(
            "tiene que ser un texto de 1 a {} caracteres",
            config::PROMPT_MAX_CHARS
        ));
    }
    Ok(prompt.into())
}

fn parse_geometry(value: &Value) -> Result<Option<Geometry>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let bad = || "tiene que ser {\"x\", \"y\", \"ancho\", \"alto\"} o null".to_string();
    let object = value.as_object().ok_or_else(bad)?;
    let int = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_i64)
            .and_then(|n| i32::try_from(n).ok())
            .ok_or_else(bad)
    };
    let [min_width, min_height] = config::WINDOW_MIN_SIZE;
    let size = |key: &str, min: f32| {
        let n = object.get(key).ok_or_else(bad)?;
        // Tamaño de ventana en puntos: entra en f32.
        number_in(n, min.into(), f64::from(u16::MAX)).map(|n| n as f32)
    };
    Ok(Some(Geometry {
        x: int("x")?,
        y: int("y")?,
        width: size("ancho", min_width)?,
        height: size("alto", min_height)?,
    }))
}

fn boolean(value: &Value) -> Result<bool, String> {
    value
        .as_bool()
        .ok_or_else(|| "tiene que ser true o false".into())
}

fn number_in(value: &Value, min: f64, max: f64) -> Result<f64, String> {
    value
        .as_f64()
        .filter(|n| (min..=max).contains(n))
        .ok_or_else(|| format!("tiene que ser un número entre {min} y {max}"))
}

fn integer_in(value: &Value, min: u64, max: u64) -> Result<u64, String> {
    value
        .as_u64()
        .filter(|n| (min..=max).contains(n))
        .ok_or_else(|| format!("tiene que ser un entero entre {min} y {max}"))
}

fn usize_in(value: &Value, min: usize, max: usize) -> Result<usize, String> {
    let bad = || format!("tiene que ser un entero entre {min} y {max}");
    let n = integer_in(value, min as u64, max as u64).map_err(|_| bad())?;
    usize::try_from(n).map_err(|_| bad())
}

pub(crate) fn bitrate_kbps(bitrate: Bitrate) -> u16 {
    match bitrate {
        Bitrate::Bitrate96 => 96,
        Bitrate::Bitrate160 => 160,
        Bitrate::Bitrate320 => 320,
    }
}

pub(crate) fn bitrate_from_kbps(kbps: u16) -> Option<Bitrate> {
    let bitrate = match kbps {
        96 => Bitrate::Bitrate96,
        160 => Bitrate::Bitrate160,
        320 => Bitrate::Bitrate320,
        _ => return None,
    };
    config::BITRATES_KBPS.contains(&kbps).then_some(bitrate)
}

fn kbps_choices() -> String {
    let choices: Vec<String> = config::BITRATES_KBPS.iter().map(u16::to_string).collect();
    choices.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo(text: &str) -> Combo {
        text.parse().unwrap()
    }

    fn load_json(text: &str) -> (Settings, Vec<String>) {
        Settings::from_json(&serde_json::from_str(text).unwrap())
    }

    #[test]
    fn los_de_fabrica_cumplen_la_invariante_y_se_guardan_vacios() {
        let defaults = Settings::default();
        assert_eq!(defaults.to_json(), json!({}));
        let (again, warnings) = Settings::from_json(&defaults.to_json());
        assert_eq!(again, defaults);
        assert!(warnings.is_empty(), "{warnings:?}");
        // Sin choques ni reservadas.
        for (target, c) in defaults.targets() {
            if let Some(c) = c {
                assert_eq!(defaults.check_combo(target, c), ComboCheck::Ok, "{c}");
            }
        }
        assert_eq!(
            defaults.combo(Target::Window(WindowAction::Next)),
            Some(combo("Ctrl+→"))
        );
        assert_eq!(
            defaults.combo(Target::Global(GlobalAction::TogglePause)),
            Some(combo("Ctrl+Alt+P"))
        );
        assert_eq!(defaults.combo(Target::Window(WindowAction::Stop)), None);
        for (_, preset) in config::THEME_PRESETS {
            assert_ne!(preset.text, preset.background);
        }
    }

    #[test]
    fn solo_se_guarda_lo_que_cambia_e_ida_y_vuelta() {
        let mut s = Settings::default();
        s.appearance.palette.accent = [0x12, 0xAB, 0xEF];
        s.appearance.font = "Courier New".into();
        s.appearance.font_size = 18.0;
        s.playback.volume_step = 10;
        s.playback.bitrate = Bitrate::Bitrate320;
        s.playback.previous_threshold = Duration::from_millis(1500);
        s.console.prompt = ">".into();
        s.console.timestamps = true;
        s.window.geometry = Some(Geometry {
            x: -8,
            y: 20,
            width: 800.0,
            height: 600.0,
        });
        s.assign(Target::Window(WindowAction::Stop), Some(combo("Ctrl+F5")));
        s.assign(Target::Global(GlobalAction::Next), None);
        let saved = s.to_json();
        assert_eq!(
            saved,
            json!({
                "tema": {"acento": "#12ABEF"},
                "fuente": {"familia": "Courier New", "tamaño": 18.0},
                "atajos_ventana": {"stop": "Ctrl+F5"},
                "atajos_globales": {"siguiente": null},
                "reproduccion": {"paso_volumen": 10, "umbral_anterior_s": 1.5, "calidad_kbps": 320},
                "consola": {"prompt": ">", "hora": true},
                "ventana": {"geometria": {"x": -8, "y": 20, "ancho": 800.0, "alto": 600.0}},
            })
        );
        let (again, warnings) = Settings::from_json(&saved);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(again, s);
    }

    #[test]
    fn cada_clave_invalida_vuelve_a_fabrica_con_aviso_y_el_resto_se_respeta() {
        let (s, warnings) = load_json(
            r##"{
                "tema": {"fondo": "#12345", "acento": "#ff0000", "brillo": 3},
                "fuente": {"familia": "Comic Sans", "tamaño": 99, "negrita_en_titulos": false},
                "atajos_ventana": {"siguiente": "Ctrl+Hyper", "anterior": "P", "stop": "Ctrl+C",
                                   "shuffle": "Ctrl+Shift+F12", "pausa": 7},
                "atajos_globales": {"stop": "Shift+F4"},
                "reproduccion": {"paso_volumen": 0, "calidad_kbps": 128, "umbral_anterior_s": 5},
                "consola": {"prompt": "", "scrollback": 50, "historial": 500},
                "ventana": {"geometria": {"x": 0, "y": 0, "ancho": 10, "alto": 400}},
                "extra": {}
            }"##,
        );
        let d = Settings::default();
        // Lo válido se respeta.
        assert_eq!(s.appearance.palette.accent, [0xFF, 0, 0]);
        assert!(!s.appearance.bold_titles);
        assert_eq!(s.playback.previous_threshold, Duration::from_secs(5));
        assert_eq!(s.console.history, 500);
        // Lo inválido queda de fábrica.
        assert_eq!(
            s.appearance.palette.background,
            d.appearance.palette.background
        );
        assert_eq!(s.appearance.font, d.appearance.font);
        assert_eq!(s.appearance.font_size, d.appearance.font_size);
        assert_eq!(s.window_keys, d.window_keys);
        assert_eq!(s.global_keys, d.global_keys);
        assert_eq!(s.playback.volume_step, d.playback.volume_step);
        assert_eq!(s.playback.bitrate, d.playback.bitrate);
        assert_eq!(s.console.prompt, d.console.prompt);
        assert_eq!(s.console.scrollback, d.console.scrollback);
        assert_eq!(s.window.geometry, None);
        // Un aviso por clave, nombrándola.
        for key in [
            "tema.fondo",
            "tema.brillo",
            "fuente.familia",
            "fuente.tamaño",
            "atajos_ventana.siguiente",
            "atajos_ventana.anterior",
            "atajos_ventana.stop",
            "atajos_ventana.shuffle",
            "atajos_ventana.pausa",
            "atajos_globales.stop",
            "reproduccion.paso_volumen",
            "reproduccion.calidad_kbps",
            "consola.prompt",
            "consola.scrollback",
            "ventana.geometria",
            "`extra`",
        ] {
            assert!(
                warnings.iter().any(|w| w.contains(key)),
                "sin aviso para {key}: {warnings:#?}"
            );
        }
        assert_eq!(warnings.len(), 16, "{warnings:#?}");
    }

    #[test]
    fn archivo_roto_o_ausente() {
        let dir = std::env::temp_dir().join(format!("spt-ajustes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let (s, warnings) = load(&dir);
        assert_eq!(s, Settings::default());
        assert!(warnings.is_empty());

        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(config::SETTINGS_FILE);
        fs::write(&path, "{ \"tema\": ").unwrap();
        let (s, warnings) = load(&dir);
        assert_eq!(s, Settings::default());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("no es JSON válido"));
        // No se pisa al cargar.
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ \"tema\": ");

        // Con BOM (Bloc de notas, PowerShell 5.1) se lee igual.
        fs::write(&path, "\u{feff}{\"consola\": {\"hora\": true}}").unwrap();
        let (s, warnings) = load(&dir);
        assert!(s.console.timestamps);
        assert!(warnings.is_empty(), "{warnings:?}");

        let (s, warnings) = load_json("[1, 2]");
        assert_eq!(s, Settings::default());
        assert_eq!(warnings.len(), 1);

        let mut changed = Settings::default();
        changed.console.history = 42;
        save(&dir, &changed).unwrap();
        assert_eq!(load(&dir), (changed, Vec::new()));
        assert!(!dir.join("ajustes.json.tmp").exists());
        save(&dir, &Settings::default()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn choques_en_el_archivo_gana_el_escrito() {
        // Pausa toma Ctrl+→ (de fábrica de "siguiente"): siguiente queda sin.
        let (s, warnings) = load_json(r#"{"atajos_ventana": {"pausa": "Ctrl+→"}}"#);
        assert_eq!(
            s.combo(Target::Window(WindowAction::TogglePause)),
            Some(combo("Ctrl+→"))
        );
        assert_eq!(s.combo(Target::Window(WindowAction::Next)), None);
        assert_eq!(warnings.len(), 1, "{warnings:?}");

        // Ventana y global con la misma, ambas escritas: gana la de ventana.
        let (s, warnings) = load_json(
            r#"{"atajos_ventana": {"stop": "Ctrl+Alt+S"}, "atajos_globales": {"stop": "Ctrl+Alt+S"}}"#,
        );
        assert_eq!(
            s.combo(Target::Window(WindowAction::Stop)),
            Some(combo("Ctrl+Alt+S"))
        );
        assert_eq!(s.combo(Target::Global(GlobalAction::Stop)), None);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn chequeo_de_combinaciones() {
        let s = Settings::default();
        let pause = Target::Window(WindowAction::TogglePause);
        let global = Target::Global(GlobalAction::Stop);
        assert!(matches!(
            s.check_combo(pause, combo("P")),
            ComboCheck::Reserved(_)
        ));
        assert!(matches!(
            s.check_combo(pause, combo("Shift+Enter")),
            ComboCheck::Reserved(_)
        ));
        assert!(matches!(
            s.check_combo(pause, combo("Ctrl+V")),
            ComboCheck::Reserved(_)
        ));
        assert!(matches!(
            s.check_combo(pause, combo("Alt+T")),
            ComboCheck::Reserved(_)
        ));
        assert_eq!(s.check_combo(pause, combo("Alt+Shift+T")), ComboCheck::Ok);
        assert!(matches!(
            s.check_combo(pause, config::RESTORE_ALL_SHORTCUT),
            ComboCheck::Reserved(_)
        ));
        assert!(matches!(
            s.check_combo(global, combo("F5")),
            ComboCheck::Reserved(_)
        ));
        assert_eq!(s.check_combo(pause, combo("F5")), ComboCheck::Ok);
        assert_eq!(
            s.check_combo(pause, combo("Ctrl+→")),
            ComboCheck::UsedBy(Target::Window(WindowAction::Next))
        );
        assert_eq!(
            s.check_combo(pause, combo("Ctrl+Alt+P")),
            ComboCheck::UsedBy(Target::Global(GlobalAction::TogglePause))
        );
        // La propia cuenta como libre.
        assert_eq!(s.check_combo(pause, combo("Ctrl+Espacio")), ComboCheck::Ok);
        assert_eq!(
            s.check_combo(global, combo("Ctrl+Alt+Enter")),
            ComboCheck::Ok
        );
    }

    #[test]
    fn asignar_saca_la_combinacion_al_otro() {
        let mut s = Settings::default();
        let pause = Target::Window(WindowAction::TogglePause);
        let next = Target::Window(WindowAction::Next);
        assert_eq!(s.assign(pause, Some(combo("Ctrl+→"))), Some(next));
        assert_eq!(s.combo(pause), Some(combo("Ctrl+→")));
        assert_eq!(s.combo(next), None);
        assert_eq!(s.assign(pause, None), None);
        assert_eq!(s.combo(pause), None);
    }

    #[test]
    fn restaurar_una_seccion_o_todo() {
        let mut s = Settings::default();
        s.appearance.palette = config::THEME_PRESETS[1].1;
        s.appearance.font_size = 20.0;
        s.console.history = 42;
        s.playback.volume_step = 10;
        // Un global toma la de fábrica de "siguiente" (ventana)...
        s.assign(Target::Window(WindowAction::Next), None);
        s.assign(Target::Global(GlobalAction::Next), Some(combo("Ctrl+→")));

        s.restore(Section::Font);
        assert_eq!(s.appearance.font_size, config::theme::FONT_SIZE);
        assert_eq!(s.appearance.palette, config::THEME_PRESETS[1].1);
        s.restore(Section::Colors);
        assert_eq!(s.appearance.palette, config::SPOTIFY_DARK);
        assert_eq!(s.console.history, 42);

        // ...al restaurar los de ventana, "siguiente" la recupera y el
        // global queda sin (invariante: sin choques).
        s.restore(Section::WindowKeys);
        assert_eq!(
            s.combo(Target::Window(WindowAction::Next)),
            Some(combo("Ctrl+→"))
        );
        assert_eq!(s.combo(Target::Global(GlobalAction::Next)), None);

        s = Settings::default();
        s.console.history = 42;
        s.restore(Section::Console);
        s.restore(Section::Playback);
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn calidades() {
        for &kbps in config::BITRATES_KBPS {
            let bitrate = bitrate_from_kbps(kbps).unwrap();
            assert_eq!(bitrate_kbps(bitrate), kbps);
        }
        assert_eq!(bitrate_from_kbps(128), None);
    }
}
