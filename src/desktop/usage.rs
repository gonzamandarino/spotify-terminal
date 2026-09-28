//! Panel de consumo (spec 014): CPU y RAM de la propia app, en texto y en
//! gráficos de los últimos `HISTORY` períodos, arriba en la columna
//! derecha. Pasar el máximo de los ajustes se marca en el color de error;
//! no cambia nada más.
//!
//! Solo se mide mientras el panel se dibuja: apagado o con la ventana
//! minimizada no se llama a nada y no hay historia.

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Ui, pos2, vec2};

use super::{
    settings::{Palette, UsageSettings},
    theme::color,
};
use crate::config::usage::{EARLY, GAP, GAP_RESET, GRAPH_HEIGHT, HISTORY, PERIOD, SCALE_HEADROOM};

const BYTES_PER_MB: f32 = 1024.0 * 1024.0;

/// Una medición del proceso.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Sample {
    /// CPU usado desde que arrancó el proceso (kernel + usuario).
    pub(super) cpu: Duration,
    pub(super) at: Instant,
    /// Working set, en bytes.
    pub(super) ram: u64,
}

/// Mide el propio proceso.
///
/// - Post: `None` si el sistema no la da (o fuera de Windows).
/// - No debe: bloquear ni reservar memoria (son dos llamadas al sistema).
#[cfg(windows)]
pub(super) fn sample() -> Option<Sample> {
    use windows_sys::Win32::{
        Foundation::FILETIME,
        System::{
            ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::{GetCurrentProcess, GetProcessTimes},
        },
    };

    let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
    let zero = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut creation, mut exit, mut kernel, mut user) = (zero, zero, zero, zero);
    let mut memory = PROCESS_MEMORY_COUNTERS {
        cb: 0,
        PageFaultCount: 0,
        PeakWorkingSetSize: 0,
        WorkingSetSize: 0,
        QuotaPeakPagedPoolUsage: 0,
        QuotaPagedPoolUsage: 0,
        QuotaPeakNonPagedPoolUsage: 0,
        QuotaNonPagedPoolUsage: 0,
        PagefileUsage: 0,
        PeakPagefileUsage: 0,
    };
    let size = u32::try_from(std::mem::size_of::<PROCESS_MEMORY_COUNTERS>()).ok()?;
    memory.cb = size;
    // SAFETY: `GetCurrentProcess` es un pseudo-handle que no se cierra;
    // los punteros son a variables locales vivas durante las llamadas, y
    // `memory.cb` es su tamaño.
    let ok = unsafe {
        let process = GetCurrentProcess();
        GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) != 0
            && K32GetProcessMemoryInfo(process, &mut memory, size) != 0
    };
    ok.then(|| Sample {
        // FILETIME cuenta de a 100 ns.
        cpu: Duration::from_nanos((ticks(kernel) + ticks(user)).saturating_mul(100)),
        at: Instant::now(),
        ram: memory.WorkingSetSize as u64,
    })
}

#[cfg(not(windows))]
pub(super) fn sample() -> Option<Sample> {
    None
}

/// CPU usado entre dos mediciones, en % de un núcleo (pasa de 100 con
/// varios núcleos ocupados).
///
/// - Pre: `next` es posterior a `prev`.
/// - Post: `≥ 0`; con intervalo cero, 0 (no divide por cero).
pub(super) fn cpu_percent(prev: &Sample, next: &Sample) -> f32 {
    let wall = next.at.saturating_duration_since(prev.at).as_secs_f32();
    if wall <= 0.0 {
        return 0.0;
    }
    next.cpu.saturating_sub(prev.cpu).as_secs_f32() / wall * 100.0
}

/// Tope del eje de un gráfico: el mayor entre `max` y el pico de
/// `history`, por `SCALE_HEADROOM`.
///
/// - Post: `> 0` si `max > 0`; `max` y todo `history` quedan por debajo.
pub(super) fn scale(history: impl IntoIterator<Item = f32>, max: f32) -> f32 {
    history.into_iter().fold(max, f32::max) * SCALE_HEADROOM
}

/// Si `value` pasó su máximo (igual todavía no).
pub(super) fn over(value: f32, max: f32) -> bool {
    value > max
}

/// Alto del panel (sin su margen) con los valores y gráficos de
/// `settings`, si cada renglón de texto mide `row`.
///
/// - Post: 0 si no hay nada que mostrar; si no, un renglón por valor,
///   `GRAPH_HEIGHT` por gráfico y `GAP` entre cada uno.
pub(super) fn height(settings: &UsageSettings, row: f32) -> f32 {
    let rows = usize::from(settings.values.cpu()) + usize::from(settings.values.ram());
    let graphs = usize::from(settings.graph.cpu()) + usize::from(settings.graph.ram());
    let items = rows + graphs;
    if items == 0 {
        return 0.0;
    }
    // A lo sumo 4: entran en f32.
    #[allow(clippy::cast_precision_loss)]
    {
        rows as f32 * row + graphs as f32 * GRAPH_HEIGHT + (items - 1) as f32 * GAP
    }
}

/// Mediciones y su historia, entre cuadros.
pub(super) struct Usage {
    /// Última medición (para el % de CPU de la próxima).
    last: Option<Sample>,
    /// Cuándo se intentó medir por última vez (aunque `sample` no dé nada).
    checked: Option<Instant>,
    cpu: Option<f32>,
    ram_mb: Option<f32>,
    cpu_history: VecDeque<f32>,
    ram_history: VecDeque<f32>,
}

impl Usage {
    pub(super) fn new() -> Self {
        Usage {
            last: None,
            checked: None,
            cpu: None,
            ram_mb: None,
            cpu_history: VecDeque::with_capacity(HISTORY),
            ram_history: VecDeque::with_capacity(HISTORY),
        }
    }

    /// Mide si ya toca (pasó `PERIOD`, menos `EARLY`, desde la anterior).
    ///
    /// - Post: si pasó más de `GAP_RESET` sin medir (ventana minimizada),
    ///   la historia se borra antes. Devuelve cuánto falta para la
    ///   próxima medición: la ventana pide redibujar para entonces.
    pub(super) fn tick(&mut self, now: Instant) -> Duration {
        if let Some(checked) = self.checked {
            let elapsed = now.saturating_duration_since(checked);
            if elapsed + EARLY < PERIOD {
                return PERIOD - elapsed;
            }
        }
        self.checked = Some(now);
        match sample() {
            Some(sample) => self.record(sample),
            None => self.clear(),
        }
        PERIOD
    }

    /// Suma una medición.
    ///
    /// - Post: RAM desde la primera; CPU desde la segunda (antes, `None`).
    ///   Cada historia guarda como mucho `HISTORY` valores, el más nuevo
    ///   al final. Si `sample` llega más de `GAP_RESET` después de la
    ///   anterior, se descarta todo lo anterior y cuenta como primera.
    pub(super) fn record(&mut self, sample: Sample) {
        if self
            .last
            .is_some_and(|last| sample.at.saturating_duration_since(last.at) > GAP_RESET)
        {
            self.clear();
        }
        // Un working set entra de sobra en f32 (con pérdida de bytes).
        #[allow(clippy::cast_precision_loss)]
        let ram = sample.ram as f32 / BYTES_PER_MB;
        push(&mut self.ram_history, ram);
        self.ram_mb = Some(ram);
        if let Some(last) = &self.last {
            let cpu = cpu_percent(last, &sample);
            push(&mut self.cpu_history, cpu);
            self.cpu = Some(cpu);
        }
        self.last = Some(sample);
    }

    /// Olvida todo: la próxima medición es la primera.
    ///
    /// - No debe: reservar ni soltar memoria (se llama en cada cuadro con
    ///   el panel oculto).
    pub(super) fn clear(&mut self) {
        self.last = None;
        self.cpu = None;
        self.ram_mb = None;
        self.cpu_history.clear();
        self.ram_history.clear();
    }

    /// Como `clear`, y olvida también cuándo se midió: al volver a mostrar
    /// el panel mide enseguida.
    pub(super) fn reset(&mut self) {
        self.clear();
        self.checked = None;
    }

    /// Dibuja los valores y gráficos de `settings` en `rect`, con texto en
    /// `font`.
    ///
    /// - Post: un valor mayor a su máximo va en `palette.error`, en el
    ///   texto y en los tramos del gráfico que lo pasan; el máximo se
    ///   dibuja como línea horizontal. Sin medición, `—`.
    /// - No debe: medir (eso es `tick`).
    pub(super) fn show(
        &self,
        ui: &Ui,
        rect: Rect,
        settings: &UsageSettings,
        palette: &Palette,
        font: &FontId,
    ) {
        let painter = ui.painter_at(rect);
        let row = ui.fonts_mut(|f| f.row_height(font));
        #[allow(clippy::cast_precision_loss)]
        let max_ram = settings.max_ram_mb as f32;
        let normal = color(palette.text);
        let error = color(palette.error);
        let mut y = rect.top();
        let mut text =
            |label: &str, value: Option<f32>, max: f32, format: &dyn Fn(f32) -> String| {
                let (shown, tint) = value.map_or(("—".to_string(), normal), |v| {
                    (format(v), if over(v, max) { error } else { normal })
                });
                painter.text(
                    pos2(rect.left(), y),
                    Align2::LEFT_TOP,
                    format!("{label} {shown}"),
                    font.clone(),
                    tint,
                );
                y += row + GAP;
            };
        if settings.values.cpu() {
            text("CPU", self.cpu, settings.max_cpu, &|v| {
                format!("{v:.1} %").replace('.', ",")
            });
        }
        if settings.values.ram() {
            text("RAM", self.ram_mb, max_ram, &|v| format!("{v:.0} MB"));
        }
        let draw = |y: f32, label: &str, history: &VecDeque<f32>, max: f32| {
            let area = Rect::from_min_size(pos2(rect.left(), y), vec2(rect.width(), GRAPH_HEIGHT));
            graph(ui, area, label, history, max, palette, font);
        };
        if settings.graph.cpu() {
            draw(y, "CPU", &self.cpu_history, settings.max_cpu);
            y += GRAPH_HEIGHT + GAP;
        }
        if settings.graph.ram() {
            draw(y, "RAM", &self.ram_history, max_ram);
        }
    }
}

/// Un gráfico de línea de `history` en `area`, el más nuevo a la derecha,
/// con el máximo como línea horizontal y `label` arriba a la izquierda.
fn graph(
    ui: &Ui,
    area: Rect,
    label: &str,
    history: &VecDeque<f32>,
    max: f32,
    palette: &Palette,
    font: &FontId,
) {
    let painter = ui.painter_at(area);
    painter.rect_stroke(
        area,
        0.0,
        Stroke::new(1.0_f32, color(palette.border)),
        egui::StrokeKind::Inside,
    );
    let top = scale(history.iter().copied(), max);
    let y_of = |v: f32| area.bottom() - (v / top).clamp(0.0, 1.0) * area.height();
    let limit = y_of(max);
    painter.add(egui::Shape::dashed_line(
        &[pos2(area.left(), limit), pos2(area.right(), limit)],
        Stroke::new(1.0_f32, color(palette.secondary)),
        4.0,
        3.0,
    ));
    // `HISTORY` es chico: entra en f32.
    #[allow(clippy::cast_precision_loss)]
    let step = area.width() / (HISTORY - 1) as f32;
    let n = history.len();
    let point = |i: usize, v: f32| -> Pos2 {
        #[allow(clippy::cast_precision_loss)]
        let back = (n - 1 - i) as f32;
        pos2(area.right() - back * step, y_of(v))
    };
    for (i, (&a, &b)) in history.iter().zip(history.iter().skip(1)).enumerate() {
        let tint: Color32 = if over(a, max) || over(b, max) {
            color(palette.error)
        } else {
            color(palette.accent)
        };
        painter.line_segment([point(i, a), point(i + 1, b)], Stroke::new(1.5_f32, tint));
    }
    // El rótulo con fondo propio, para que no se pise con la línea del
    // máximo.
    let galley = painter.layout_no_wrap(label.into(), font.clone(), color(palette.secondary));
    let at = area.left_top() + vec2(2.0, 2.0);
    painter.rect_filled(
        Rect::from_min_size(at, galley.size() + vec2(4.0, 0.0)),
        0.0,
        color(palette.background),
    );
    painter.galley(at + vec2(2.0, 0.0), galley, color(palette.secondary));
}

fn push(history: &mut VecDeque<f32>, value: f32) {
    if history.len() == HISTORY {
        history.pop_front();
    }
    history.push_back(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::settings::{UsageGraph, UsageValues};

    fn at(start: Instant, ms: u64, cpu_ms: u64, ram_mb: u64) -> Sample {
        Sample {
            cpu: Duration::from_millis(cpu_ms),
            at: start + Duration::from_millis(ms),
            ram: ram_mb * 1024 * 1024,
        }
    }

    #[test]
    fn cpu_en_porcentaje_de_un_nucleo() {
        let t = Instant::now();
        assert_eq!(cpu_percent(&at(t, 0, 100, 1), &at(t, 1000, 100, 1)), 0.0);
        assert!((cpu_percent(&at(t, 0, 0, 1), &at(t, 1000, 500, 1)) - 50.0).abs() < 1e-3);
        // Varios núcleos ocupados: pasa de 100.
        assert!((cpu_percent(&at(t, 0, 0, 1), &at(t, 1000, 2500, 1)) - 250.0).abs() < 1e-3);
        // Intervalo cero: no divide por cero.
        assert_eq!(cpu_percent(&at(t, 0, 0, 1), &at(t, 0, 500, 1)), 0.0);
    }

    #[test]
    fn cpu_desde_la_segunda_medicion() {
        let t = Instant::now();
        let mut usage = Usage::new();
        usage.record(at(t, 0, 0, 40));
        assert_eq!(usage.cpu, None);
        assert_eq!(usage.ram_mb, Some(40.0));
        usage.record(at(t, 1000, 20, 41));
        assert!((usage.cpu.unwrap() - 2.0).abs() < 1e-3);
        assert_eq!(usage.ram_mb, Some(41.0));
        assert_eq!(usage.cpu_history.len(), 1);
        assert_eq!(usage.ram_history.len(), 2);
    }

    #[test]
    fn la_historia_guarda_las_ultimas() {
        let t = Instant::now();
        let mut usage = Usage::new();
        for i in 0..(HISTORY as u64 + 10) {
            usage.record(at(t, i * 1000, 0, i));
        }
        assert_eq!(usage.ram_history.len(), HISTORY);
        assert_eq!(usage.cpu_history.len(), HISTORY);
        assert_eq!(usage.ram_history.back(), Some(&(HISTORY as f32 + 9.0)));
        assert_eq!(usage.ram_history.front(), Some(&10.0));
    }

    #[test]
    fn un_hueco_largo_borra_la_historia() {
        let t = Instant::now();
        let mut usage = Usage::new();
        usage.record(at(t, 0, 0, 40));
        usage.record(at(t, 1000, 10, 40));
        let later = GAP_RESET.as_millis() as u64 + 2000;
        usage.record(at(t, later, 900, 50));
        assert_eq!(usage.cpu, None, "no promedia el hueco");
        assert_eq!(usage.ram_history.len(), 1);
        assert!(usage.cpu_history.is_empty());
        usage.clear();
        assert!(usage.ram_history.is_empty() && usage.ram_mb.is_none());
    }

    #[test]
    fn mide_una_vez_por_periodo() {
        let t = Instant::now();
        let mut usage = Usage::new();
        assert_eq!(usage.tick(t), PERIOD);
        let wait = usage.tick(t + Duration::from_millis(300));
        assert_eq!(wait, PERIOD - Duration::from_millis(300));
        // Un poco antes del período cuenta igual.
        assert_eq!(usage.tick(t + PERIOD - EARLY), PERIOD);
    }

    #[test]
    fn escala_con_el_maximo_siempre_visible() {
        assert!((scale([1.0, 2.0], 10.0) - 10.0 * SCALE_HEADROOM).abs() < 1e-4);
        assert!((scale([1.0, 30.0], 10.0) - 30.0 * SCALE_HEADROOM).abs() < 1e-4);
        assert!((scale([], 5.0) - 5.0 * SCALE_HEADROOM).abs() < 1e-4);
    }

    #[test]
    fn pasar_el_maximo_es_mayor_estricto() {
        assert!(!over(10.0, 10.0));
        assert!(over(10.01, 10.0));
        assert!(!over(3.0, 10.0));
    }

    #[test]
    fn alto_segun_valores_y_graficos() {
        let mut s = UsageSettings::default();
        assert_eq!(height(&s, 20.0), 2.0 * 20.0 + GAP);
        s.values = UsageValues::Cpu;
        assert_eq!(height(&s, 20.0), 20.0);
        s.graph = UsageGraph::Both;
        assert_eq!(height(&s, 20.0), 20.0 + 2.0 * GRAPH_HEIGHT + 2.0 * GAP);
        s.values = UsageValues::Ram;
        s.graph = UsageGraph::Cpu;
        assert_eq!(height(&s, 20.0), 20.0 + GRAPH_HEIGHT + GAP);
    }

    #[cfg(windows)]
    #[test]
    fn mide_el_proceso_en_windows() {
        let a = sample().expect("medición");
        assert!(a.ram > 1024 * 1024);
        let b = sample().expect("medición");
        assert!(b.cpu >= a.cpu);
    }
}
