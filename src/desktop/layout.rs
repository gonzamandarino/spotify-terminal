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

/// Dónde va cada cosa en la fila de arriba de "sonando" (spec 012), en x.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PlayerRow {
    /// Centros de ⏮, ⏯ y ⏭.
    pub(super) controls: [f32; 3],
    /// Centro del botón de shuffle.
    pub(super) shuffle: f32,
    /// Borde derecho del chip de volumen, si entra.
    pub(super) volume: Option<f32>,
    /// Borde derecho de cada chip de `others` que entra (`None` = no entra).
    pub(super) others: Vec<Option<f32>>,
    /// Hasta dónde pueden llegar el título, los artistas y el ♥.
    pub(super) text_right: f32,
}

/// Medidas de los botones de "sonando", en puntos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ButtonSizes {
    /// Lado de un botón.
    pub(super) button: f32,
    /// Espacio entre ⏮ ⏯ ⏭.
    pub(super) gap: f32,
    /// Espacio entre los demás elementos de la fila.
    pub(super) space: f32,
}

/// Ubica los botones y los chips entre `left` (donde empieza el título) y
/// `right`, con ⏮ ⏯ ⏭ centrados en `center`. A la derecha, de afuera hacia adentro: el chip de
/// volumen (ancho `volume`), shuffle y los chips `others` (en orden, el
/// primero al lado de shuffle).
///
/// - Post (con `button`, `gap` y `space` de `sizes`): los botones siempre
///   entran enteros si `right - left ≥ 5 * button + 2 * gap + 4 * space`, sin superponerse
///   con nada; el ♥ tiene lugar (`text_right - left ≥ button + space`).
///   Si no entra todo, se dejan de mostrar primero los últimos de
///   `others`, después el resto y al final el volumen; si ni así entran
///   centrados, el grupo se corre hacia el lado que tiene lugar.
pub(super) fn player_row(
    left: f32,
    right: f32,
    center: f32,
    sizes: ButtonSizes,
    volume: Option<f32>,
    others: &[f32],
) -> PlayerRow {
    let ButtonSizes { button, gap, space } = sizes;
    let group = 3.0 * button + 2.0 * gap;
    // Lo mínimo a la izquierda del grupo: el ♥ (el título puede ser "…").
    let min_left = left + button + 2.0 * space;
    let mut group_left = (center - group / 2.0).max(min_left);
    // Lo que ocupa la derecha con el volumen (o no) y los primeros `n`
    // chips.
    let need = |with_volume: bool, n: usize| {
        let volume = if with_volume {
            volume.map_or(0.0, |w| w + space)
        } else {
            0.0
        };
        volume + button + others[..n].iter().map(|w| w + space).sum::<f32>()
    };
    let options = (0..=others.len())
        .rev()
        .map(|n| (true, n))
        .chain([(false, 0)]);
    let mut chosen = (false, 0);
    let mut fits = false;
    for (with_volume, n) in options {
        if right - need(with_volume, n) >= group_left + group + space {
            chosen = (with_volume, n);
            fits = true;
            break;
        }
    }
    if !fits {
        group_left = (right - need(false, 0) - space - group).max(min_left);
    }
    let (with_volume, n) = chosen;

    let mut cursor = right;
    let volume = match volume {
        Some(width) if with_volume => {
            let at = cursor;
            cursor -= width + space;
            Some(at)
        }
        _ => None,
    };
    let shuffle = cursor - button / 2.0;
    cursor -= button + space;
    let others = others
        .iter()
        .enumerate()
        .map(|(i, w)| {
            (i < n).then(|| {
                let at = cursor;
                cursor -= w + space;
                at
            })
        })
        .collect();
    let first = group_left + button / 2.0;
    PlayerRow {
        controls: [first, first + button + gap, first + 2.0 * (button + gap)],
        shuffle,
        volume,
        others,
        text_right: group_left - space,
    }
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

    /// Anchos de ejemplo con la letra por defecto: volumen, [posición…].
    const VOLUME: f32 = 60.0;
    const DEFAULT: ButtonSizes = ButtonSizes {
        button: 24.0,
        gap: 6.0,
        space: 14.0,
    };

    #[test]
    fn con_lugar_entra_todo_y_los_controles_quedan_centrados() {
        let row = player_row(40.0, 900.0, 470.0, DEFAULT, Some(VOLUME), &[45.0, 60.0]);
        assert_eq!(row.controls, [440.0, 470.0, 500.0]);
        assert_eq!(row.volume, Some(900.0));
        assert_eq!(row.shuffle, 900.0 - VOLUME - 14.0 - 12.0);
        let cola = 900.0 - VOLUME - 14.0 - 24.0 - 14.0;
        assert_eq!(row.others, [Some(cola), Some(cola - 45.0 - 14.0)]);
        assert_eq!(row.text_right, 428.0 - 14.0);
    }

    #[test]
    fn sin_lugar_se_van_primero_los_chips_y_despues_el_volumen() {
        // Ventana mínima, letra por defecto: no entra la posición.
        let row = player_row(40.0, 504.0, 260.0, DEFAULT, Some(VOLUME), &[45.0, 60.0]);
        assert_eq!(row.others[1], None);
        assert!(row.volume.is_some());
        // Letra máxima (× 2,3): solo quedan los botones.
        let s = 32.0 / 14.0;
        let (b, g, sp) = (24.0 * s, 6.0 * s, 14.0 * s);
        let row = player_row(
            71.0,
            504.0,
            260.0,
            ButtonSizes {
                button: b,
                gap: g,
                space: sp,
            },
            Some(137.0),
            &[100.0, 130.0],
        );
        assert_eq!(row.volume, None);
        assert_eq!(row.others, [None, None]);
        // Nada se pisa: ♥ | ⏮ ⏯ ⏭ | 🔀, todo dentro de la fila.
        assert!(row.text_right - 71.0 >= b);
        assert!(row.controls[0] - b / 2.0 >= row.text_right);
        assert!(row.controls[2] + b / 2.0 <= row.shuffle - b / 2.0);
        assert!(row.shuffle + b / 2.0 <= 504.0);
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
