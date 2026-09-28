//! Altos de las barras de la ventana según el tamaño de letra (spec 009).
//! Sin egui: se prueba sin ventana.

use crate::config::{
    layout::{
        CONSOLE_MARGIN_BOTTOM, CONSOLE_MARGIN_TOP, CONSOLE_MIN_ROWS, INPUT_HEIGHT,
        INPUT_TIGHT_HEIGHT, MENU_HEIGHT, NOW_HEIGHT, NOW_TIGHT_HEIGHT, TITLE_HEIGHT,
    },
    theme::FONT_SIZE,
};

/// Altos de las barras que siguen a la letra, en puntos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Bars {
    /// Barra "sonando".
    pub(super) now: f32,
    /// Línea de entrada.
    pub(super) input: f32,
}

/// Cuánto más grande (o chica) es `font_size` que la letra por defecto: el
/// texto del contenido (consola, entrada, "sonando") se multiplica por esto.
pub(super) fn scale(font_size: f32) -> f32 {
    font_size / FONT_SIZE
}

/// Altos de "sonando" y de la entrada con letra `font_size`, en una ventana
/// de `available` puntos de alto; `console_row` es el alto de un renglón de
/// la consola con esa letra.
///
/// - Post: con letra por defecto o más chica, y lugar para
///   `CONSOLE_MIN_ROWS` renglones, los altos de config (los de 0.1.0). Con
///   letra más grande crecen en proporción. Si no queda lugar para esos
///   renglones, las barras bajan hasta lo justo para su texto
///   (`*_TIGHT_HEIGHT` en proporción) y la consola se achica.
/// - Invariante: título + menús + las dos barras nunca pasan `available`
///   (si ni lo justo entra, las barras se achican aunque corten el texto:
///   nunca se superponen ni salen de la ventana).
pub(super) fn bars(font_size: f32, console_row: f32, available: f32) -> Bars {
    let s = scale(font_size);
    let (want_now, want_input) = (NOW_HEIGHT * s.max(1.0), INPUT_HEIGHT * s.max(1.0));
    let (least_now, least_input) = (NOW_TIGHT_HEIGHT * s, INPUT_TIGHT_HEIGHT * s);
    let console = CONSOLE_MIN_ROWS * console_row
        + f32::from(CONSOLE_MARGIN_TOP)
        + f32::from(CONSOLE_MARGIN_BOTTOM);
    let free = (available - TITLE_HEIGHT - MENU_HEIGHT).max(0.0);
    let room = (free - console).max(0.0);
    let (want, least) = (want_now + want_input, least_now + least_input);
    // Qué parte de lo que sobra sobre lo justo se le da a las barras.
    let share = if want <= room {
        1.0
    } else {
        ((room - least) / (want - least)).clamp(0.0, 1.0)
    };
    let mut now = least_now + share * (want_now - least_now);
    let mut input = least_input + share * (want_input - least_input);
    let total = now + input;
    if total > free {
        let fit = free / total;
        now *= fit;
        input *= fit;
    }
    Bars { now, input }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FONT_SIZE_MAX, FONT_SIZE_MIN, WINDOW_MIN_SIZE, WINDOW_SIZE};

    /// Renglón de consola aproximado (egui: ~1,2 × la letra, más el
    /// espacio entre renglones).
    fn row(font_size: f32) -> f32 {
        font_size * 1.2 + 4.0
    }

    fn fixed() -> f32 {
        TITLE_HEIGHT + MENU_HEIGHT
    }

    #[test]
    fn con_la_letra_por_defecto_quedan_los_altos_de_siempre() {
        for height in [WINDOW_MIN_SIZE[1], WINDOW_SIZE[1], 1200.0] {
            let bars = bars(FONT_SIZE, row(FONT_SIZE), height);
            assert_eq!(
                bars,
                Bars {
                    now: NOW_HEIGHT,
                    input: INPUT_HEIGHT
                },
                "{height}"
            );
        }
    }

    #[test]
    fn con_letra_chica_no_bajan_de_los_de_siempre() {
        let bars = bars(FONT_SIZE_MIN, row(FONT_SIZE_MIN), WINDOW_MIN_SIZE[1]);
        assert_eq!(bars.now, NOW_HEIGHT);
        assert_eq!(bars.input, INPUT_HEIGHT);
    }

    #[test]
    fn con_letra_grande_crecen_si_hay_lugar() {
        let bars = bars(FONT_SIZE_MAX, row(FONT_SIZE_MAX), 1200.0);
        let s = scale(FONT_SIZE_MAX);
        assert!((bars.now - NOW_HEIGHT * s).abs() < 0.01);
        assert!((bars.input - INPUT_HEIGHT * s).abs() < 0.01);
    }

    #[test]
    fn con_letra_grande_en_la_ventana_minima_no_se_superponen() {
        let height = WINDOW_MIN_SIZE[1];
        let bars = bars(FONT_SIZE_MAX, row(FONT_SIZE_MAX), height);
        let s = scale(FONT_SIZE_MAX);
        // Entra lo justo para el texto y todo dentro de la ventana, con
        // lugar para la consola.
        assert!(bars.now >= NOW_TIGHT_HEIGHT * s - 0.01);
        assert!(bars.input >= INPUT_TIGHT_HEIGHT * s - 0.01);
        assert!(fixed() + bars.now + bars.input < height);
    }

    #[test]
    fn nunca_salen_de_la_ventana() {
        for height in [0.0, 50.0, 100.0, 150.0, 200.0] {
            for size in [FONT_SIZE_MIN, FONT_SIZE, FONT_SIZE_MAX] {
                let bars = bars(size, row(size), height);
                let free = (height - fixed()).max(0.0);
                assert!(
                    bars.now + bars.input <= free + 0.01,
                    "{size} pt en {height}"
                );
                assert!(bars.now >= 0.0 && bars.input >= 0.0);
            }
        }
    }
}
