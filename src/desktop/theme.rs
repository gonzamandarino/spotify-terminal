//! Look de la app de escritorio: fuente y colores de los ajustes (spec 007)
//! aplicados al estilo de egui. Se puede reaplicar en cualquier frame.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Visuals};

use super::settings::Appearance;
use crate::config::{self, FontEntry, layout::UI_FONT_SIZE};

/// Familia de la fuente en negrita (título, tema que suena).
pub(super) const BOLD: &str = "bold";

pub(super) fn color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// Carpeta de fuentes del sistema (`%WINDIR%\Fonts`).
fn fonts_dir() -> PathBuf {
    std::env::var_os("WINDIR")
        .map_or_else(|| r"C:\Windows".into(), PathBuf::from)
        .join("Fonts")
}

/// Fuente de la carpeta de fuentes del sistema, o `None` si no se puede
/// leer.
fn system_font(dir: &Path, file: &str) -> Option<FontData> {
    let bytes = std::fs::read(dir.join(file)).ok()?;
    Some(FontData::from_owned(bytes))
}

pub(super) fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

pub(super) fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD.into()))
}

/// Fuentes que se pueden elegir en esta PC: las de `config::FONT_CATALOG`
/// instaladas, y la de egui (siempre, al final).
///
/// - No debe: leer las fuentes enteras (solo mira si el archivo existe).
pub(super) fn installed_fonts() -> Vec<&'static str> {
    let dir = fonts_dir();
    config::FONT_CATALOG
        .iter()
        .filter(|f| dir.join(f.regular).is_file())
        .map(|f| f.name)
        .chain([config::BUILTIN_FONT])
        .collect()
}

/// Lo que ya está aplicado, para no recargar la fuente si no cambió.
#[derive(Default)]
pub(super) struct Theme {
    font: Option<String>,
}

impl Theme {
    /// Aplica `appearance`: fuente y colores.
    ///
    /// - Post: la fuente se relee del disco solo si cambió (la anterior se
    ///   suelta: cambiar varias veces no acumula memoria). Los estilos de
    ///   texto de egui (menús, diálogos) quedan en `UI_FONT_SIZE`, fijo:
    ///   `appearance.font_size` no se aplica acá sino en el texto de la
    ///   consola, la entrada y "sonando" (`app`, spec 009). No toca el zoom
    ///   de egui: queda en 1 (el `Ctrl++` / `Ctrl+-` de egui se apaga para
    ///   que sean atajos propios). Los símbolos que la fuente no tiene (♫,
    ///   🔀...) salen de las fuentes de egui. Con fondo claro, los widgets
    ///   de egui usan su estilo claro. El cursor de texto no titila: cada
    ///   parpadeo sería un redibujo completo por CPU (T5 del spec 004: ~6 %
    ///   de un núcleo en reposo).
    /// - Devuelve: un aviso si la fuente elegida no está instalada (queda
    ///   la de egui). Sin negrita propia no hay aviso: usa la normal.
    pub(super) fn apply(&mut self, ctx: &egui::Context, appearance: &Appearance) -> Option<String> {
        let mut warning = None;
        if self.font.as_deref() != Some(appearance.font.as_str()) {
            warning = set_font(ctx, &appearance.font);
            self.font = Some(appearance.font.clone());
        }
        let p = &appearance.palette;
        ctx.global_style_mut(|style| {
            let mut visuals = if is_light(p.background) {
                Visuals::light()
            } else {
                Visuals::dark()
            };
            visuals.panel_fill = color(p.background);
            visuals.window_fill = color(p.panel);
            visuals.window_stroke = Stroke::new(1.0_f32, color(p.border));
            visuals.extreme_bg_color = color(p.background);
            visuals.override_text_color = Some(color(p.text));
            visuals.selection.bg_fill = color(p.accent).gamma_multiply(0.35);
            visuals.selection.stroke = Stroke::new(1.0_f32, color(p.text_strong));
            visuals.text_cursor.stroke = Stroke::new(2.0_f32, color(p.accent));
            visuals.text_cursor.blink = false;
            visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, color(p.border));
            visuals.widgets.hovered.weak_bg_fill = color(p.border);
            visuals.widgets.open.weak_bg_fill = color(p.border);
            style.visuals = visuals;
            for text_style in [TextStyle::Body, TextStyle::Monospace, TextStyle::Button] {
                style.text_styles.insert(text_style, mono(UI_FONT_SIZE));
            }
            style
                .text_styles
                .insert(TextStyle::Small, mono(UI_FONT_SIZE - 3.0));
            style.spacing.scroll.bar_width = 6.0;
            style.spacing.scroll.floating = true;
        });
        // Ctrl++ / Ctrl+- son atajos propios (configurables), no el zoom
        // de egui.
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        warning
    }
}

/// Fondo claro: luminancia aproximada por encima de la mitad.
fn is_light(rgb: [u8; 3]) -> bool {
    let [r, g, b] = rgb.map(u32::from);
    (299 * r + 587 * g + 114 * b) / 1000 > 128
}

/// Instala `name` (de `config::FONT_CATALOG`, o la de egui) para todo el
/// texto; la negrita usa la negrita propia si hay, si no la normal. Devuelve
/// un aviso si no se pudo leer.
fn set_font(ctx: &egui::Context, name: &str) -> Option<String> {
    let mut fonts = FontDefinitions::default();
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut bold_family = fallbacks;
    let mut warning = None;
    if name != config::BUILTIN_FONT {
        let dir = fonts_dir();
        let entry: Option<&FontEntry> = config::FONT_CATALOG.iter().find(|f| f.name == name);
        let regular = entry.and_then(|f| system_font(&dir, f.regular));
        match regular {
            Some(regular) => {
                fonts.font_data.insert("elegida".into(), Arc::new(regular));
                for family in [FontFamily::Monospace, FontFamily::Proportional] {
                    fonts
                        .families
                        .entry(family)
                        .or_default()
                        .insert(0, "elegida".into());
                }
                let bold = entry
                    .and_then(|f| f.bold)
                    .and_then(|file| system_font(&dir, file));
                match bold {
                    Some(bold) => {
                        fonts
                            .font_data
                            .insert("elegida-negrita".into(), Arc::new(bold));
                        bold_family.insert(0, "elegida-negrita".into());
                    }
                    None => bold_family.insert(0, "elegida".into()),
                }
            }
            None => {
                warning = Some(format!(
                    "⚠ La fuente {name} no está instalada: se usa la de la app."
                ));
            }
        }
    }
    fonts
        .families
        .insert(FontFamily::Name(BOLD.into()), bold_family);
    ctx.set_fonts(fonts);
    warning
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fondos_claros_y_oscuros() {
        for (name, palette) in config::THEME_PRESETS {
            assert_eq!(is_light(palette.background), *name == "Claro", "{name}");
        }
    }

    #[test]
    fn la_de_egui_siempre_esta_y_al_final() {
        let fonts = installed_fonts();
        assert_eq!(fonts.last(), Some(&config::BUILTIN_FONT));
    }
}
