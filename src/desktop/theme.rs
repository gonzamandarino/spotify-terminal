//! Look de la app de escritorio: fuente de la consola de Windows y colores de
//! `config::theme` aplicados al estilo de egui.

use std::{path::PathBuf, sync::Arc};

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Visuals};

use crate::config::theme;

/// Familia de la fuente en negrita (título, tema que suena).
pub(super) const BOLD: &str = "bold";

pub(super) fn color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// Fuente de la carpeta de fuentes del sistema (`%WINDIR%\Fonts`), o
/// `None` si no se puede leer.
fn system_font(file: &str) -> Option<FontData> {
    let windows = std::env::var_os("WINDIR").map_or_else(|| r"C:\Windows".into(), PathBuf::from);
    let bytes = std::fs::read(windows.join("Fonts").join(file)).ok()?;
    Some(FontData::from_owned(bytes))
}

pub(super) fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

pub(super) fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD.into()))
}

/// Instala la fuente y el estilo. Llamar una vez, antes del primer frame.
///
/// - Post: el texto usa Consolas, la fuente de la consola de Windows,
///   leída de la carpeta de fuentes del sistema (su licencia no permite
///   incluirla en el repo). Si no está, queda la monoespaciada de egui; si
///   falta solo la negrita, se usa la normal. Los símbolos que Consolas no
///   tiene (♫, 🔀...) salen de las fuentes de egui. El cursor de texto no
///   titila: cada parpadeo sería un redibujo completo por CPU (T5 del spec
///   004: ~6 % de un núcleo en reposo).
pub(super) fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let regular = system_font(theme::CONSOLE_FONT);
    let bold = system_font(theme::CONSOLE_FONT_BOLD);
    let mut bold_family = fallbacks;
    if let Some(regular) = regular {
        fonts.font_data.insert("consola".into(), Arc::new(regular));
        for family in [FontFamily::Monospace, FontFamily::Proportional] {
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, "consola".into());
        }
        bold_family.insert(0, "consola".into());
    }
    if let Some(bold) = bold {
        fonts
            .font_data
            .insert("consola-bold".into(), Arc::new(bold));
        bold_family.insert(0, "consola-bold".into());
    }
    fonts
        .families
        .insert(FontFamily::Name(BOLD.into()), bold_family);
    ctx.set_fonts(fonts);

    ctx.global_style_mut(|style| {
        let mut visuals = Visuals::dark();
        visuals.panel_fill = color(theme::BACKGROUND);
        visuals.window_fill = color(theme::PANEL);
        visuals.extreme_bg_color = color(theme::BACKGROUND);
        visuals.override_text_color = Some(color(theme::TEXT));
        visuals.selection.bg_fill = color(theme::ACCENT).gamma_multiply(0.35);
        visuals.selection.stroke = Stroke::new(1.0_f32, color(theme::TEXT_STRONG));
        visuals.text_cursor.stroke = Stroke::new(2.0_f32, color(theme::ACCENT));
        visuals.text_cursor.blink = false;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, color(theme::BORDER));
        style.visuals = visuals;
        for text_style in [TextStyle::Body, TextStyle::Monospace, TextStyle::Button] {
            style.text_styles.insert(text_style, mono(theme::FONT_SIZE));
        }
        style
            .text_styles
            .insert(TextStyle::Small, mono(theme::FONT_SIZE - 3.0));
        style.spacing.scroll.bar_width = 6.0;
        style.spacing.scroll.floating = true;
    });
}
