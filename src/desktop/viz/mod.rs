//! Visualización de lo que suena, en un panel a la derecha de la consola
//! (spec 010): onda, barras (espectro) o vinilo con la tapa del disco.
//!
//! Solo se anima mientras suena: quieto no pide cuadros y la ventana vuelve
//! a su consumo de reposo.

mod spectrum;

use std::{f32::consts::TAU, time::Instant};

use egui::{Color32, ColorImage, Mesh, Pos2, Rect, Shape, Stroke, TextureHandle, Ui, Vec2, epaint};

use self::spectrum::Spectrum;
use super::{
    settings::{Palette, VizMode},
    theme::color,
};
use crate::{app::queue::PlayState, config::viz, spotify::tap::AudioTap};

/// Lo que hace falta del tema que suena para dibujarlo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Track<'a> {
    /// Nada cargado (stop, recién abierta).
    Nothing,
    /// Un tema, sonando o no, con la URL de su tapa si tiene.
    Loaded {
        state: PlayState,
        cover: Option<&'a str>,
    },
}

/// Estado de la visualización entre cuadros.
pub(super) struct Visualizer {
    tap: AudioTap,
    mode: VizMode,
    /// Buffers reusados en cada cuadro (sin reservar memoria al dibujar).
    samples: Vec<f32>,
    db: Vec<f32>,
    levels: Vec<f32>,
    spectrum: Spectrum,
    /// Lo más fuerte reciente, en dB: la altura de las barras es relativa
    /// a esto.
    reference: f32,
    /// Alto de cada barra, con caída suave.
    bars: Vec<f32>,
    /// Puntos de la onda (-1 a 1), mezclados con los del cuadro anterior.
    wave: Vec<f32>,
    /// Ángulo del vinilo, en radianes.
    angle: f32,
    /// Último cuadro animado, para avanzar según el tiempo real.
    last: Option<Instant>,
    /// Tapa del tema que suena: URL y textura. Al reemplazarla se suelta
    /// la anterior.
    cover: Option<(String, TextureHandle)>,
}

impl Visualizer {
    /// Sin visualización (`VizMode::None`) y con `tap` apagado.
    pub(super) fn new(tap: AudioTap) -> Self {
        tap.set_enabled(false);
        Visualizer {
            tap,
            mode: VizMode::None,
            samples: Vec::with_capacity(viz::READ_SAMPLES),
            db: vec![0.0; viz::BARS],
            levels: vec![0.0; viz::BARS],
            spectrum: Spectrum::new(
                viz::FFT_SIZE,
                viz::BARS,
                (viz::BAR_MIN_HZ, viz::BAR_MAX_HZ),
                librespot_playback::SAMPLE_RATE as f32,
                viz::BAR_TILT_DB_PER_OCT,
            ),
            reference: viz::BAR_REF_MIN_DB,
            bars: vec![0.0; viz::BARS],
            wave: vec![0.0; viz::WAVE_POINTS],
            angle: 0.0,
            last: None,
            cover: None,
        }
    }

    /// Cambia lo que se dibuja.
    ///
    /// - Post: el tap copia muestras solo con Onda o Barras (con Vinilo o
    ///   Ninguna no se paga la copia). Sin Vinilo se suelta la tapa.
    pub(super) fn set_mode(&mut self, mode: VizMode) {
        self.mode = mode;
        self.tap
            .set_enabled(matches!(mode, VizMode::Wave | VizMode::Bars));
        self.bars.fill(0.0);
        self.wave.fill(0.0);
        self.reference = viz::BAR_REF_MIN_DB;
        self.last = None;
        if mode != VizMode::Vinyl {
            self.cover = None;
        }
    }

    pub(super) fn mode(&self) -> VizMode {
        self.mode
    }

    /// Si hacen falta las tapas (solo con Vinilo).
    pub(super) fn wants_covers(&self) -> bool {
        self.mode == VizMode::Vinyl
    }

    /// La tapa de `url`, ya decodificada. Reemplaza (y suelta) la anterior.
    pub(super) fn set_cover(&mut self, ctx: &egui::Context, url: String, image: ColorImage) {
        if !self.wants_covers() {
            return;
        }
        let texture = ctx.load_texture("tapa", image, egui::TextureOptions::LINEAR);
        self.cover = Some((url, texture));
    }

    /// Ancho de la columna derecha (visualización y, si `usage`, el panel
    /// de consumo de spec 014), en una ventana con `available` puntos de
    /// ancho, si se pidió `wanted`. `None` si no se muestra.
    pub(super) fn panel_width(&self, usage: bool, wanted: f32, available: f32) -> Option<f32> {
        panel_width(self.mode, usage, wanted, available)
    }

    /// Dibuja el modo elegido en `rect`.
    ///
    /// - Post: sonando, avanza la animación según el tiempo desde el último
    ///   cuadro. En pausa queda quieto lo último; sin tema, onda plana,
    ///   barras en cero y vinilo sin tapa.
    /// - Devuelve: `true` si hace falta otro cuadro (solo sonando): la
    ///   ventana lo pide a `FPS`.
    pub(super) fn show(
        &mut self,
        ui: &Ui,
        rect: Rect,
        track: Track<'_>,
        palette: &Palette,
    ) -> bool {
        let playing = matches!(
            track,
            Track::Loaded {
                state: PlayState::Playing,
                ..
            }
        );
        let now = Instant::now();
        let dt = if playing {
            self.last.map_or(0.0, |last| (now - last).as_secs_f32())
        } else {
            0.0
        };
        self.last = playing.then_some(now);
        let painter = ui.painter_at(rect);
        let accent = color(palette.accent);
        match self.mode {
            VizMode::None => return false,
            VizMode::Wave => {
                if matches!(track, Track::Nothing) {
                    self.wave.fill(0.0);
                } else if playing {
                    self.snapshot(viz::WAVE_SEARCH + viz::WAVE_SAMPLES);
                    let start = trigger(&self.samples, viz::WAVE_SEARCH);
                    let shown = self
                        .samples
                        .get(start..start + viz::WAVE_SAMPLES)
                        .unwrap_or_default();
                    blend_wave(&mut self.wave, shown, viz::WAVE_BLEND);
                }
                painter.add(Shape::line(
                    wave_points(&self.wave, rect),
                    Stroke::new(1.5_f32, accent),
                ));
            }
            VizMode::Bars => {
                if matches!(track, Track::Nothing) {
                    self.bars.fill(0.0);
                } else if playing {
                    self.snapshot(viz::FFT_SIZE);
                    self.spectrum.bands(&self.samples, &mut self.db);
                    spectrum::normalize(
                        &self.db,
                        &mut self.reference,
                        dt,
                        (
                            viz::BAR_RANGE_DB,
                            viz::BAR_REF_FALL_DB_PER_SEC,
                            viz::BAR_REF_MIN_DB,
                        ),
                        &mut self.levels,
                    );
                    fall(&mut self.bars, &self.levels, dt);
                }
                let gap = 2.0;
                let width = rect.width() / self.bars.len() as f32;
                for (i, &level) in self.bars.iter().enumerate() {
                    let left = rect.left() + i as f32 * width;
                    let top = rect.bottom() - (level * rect.height()).max(1.0);
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(left + gap / 2.0, top),
                            Pos2::new(left + width - gap / 2.0, rect.bottom()),
                        ),
                        1.0,
                        accent,
                    );
                }
            }
            VizMode::Vinyl => {
                self.angle = (self.angle + dt * viz::VINYL_RPM / 60.0 * TAU) % TAU;
                let cover = match track {
                    Track::Loaded {
                        cover: Some(url), ..
                    } => self
                        .cover
                        .as_ref()
                        .filter(|(have, _)| have == url)
                        .map(|(_, texture)| texture.id()),
                    _ => None,
                };
                // Un pixel de la pantalla, para suavizar el borde de la tapa.
                let pixel = 1.0 / ui.ctx().pixels_per_point();
                vinyl(&painter, rect, self.angle, cover, pixel, palette);
            }
        }
        playing
    }

    /// Las últimas `count` muestras, atrasadas `LATENCY`.
    fn snapshot(&mut self, count: usize) {
        let delay = (viz::LATENCY.as_secs_f32() * librespot_playback::SAMPLE_RATE as f32) as usize;
        self.tap.snapshot(&mut self.samples, count, delay);
    }
}

/// `wanted` recortado a `[PANEL_WIDTH_MIN, PANEL_WIDTH_MAX]` y a lo que
/// deja libre la consola (`MIN_CONSOLE_WIDTH`); `None` con Ninguna y sin
/// consumo, o si ni el mínimo entra.
fn panel_width(mode: VizMode, usage: bool, wanted: f32, available: f32) -> Option<f32> {
    let room = available - viz::MIN_CONSOLE_WIDTH;
    ((mode != VizMode::None || usage) && room >= viz::PANEL_WIDTH_MIN).then(|| {
        wanted
            .clamp(viz::PANEL_WIDTH_MIN, viz::PANEL_WIDTH_MAX)
            .min(room)
    })
}

/// Lo que queda de `area` para la visualización debajo de `used` puntos
/// (el panel de consumo y su separación, spec 014).
///
/// - Post: `None` si quedan menos de `MIN_VIZ_HEIGHT`: no se dibuja ni se
///   anima.
pub(super) fn below(area: Rect, used: f32) -> Option<Rect> {
    let top = area.top() + used;
    (area.bottom() - top >= viz::MIN_VIZ_HEIGHT)
        .then(|| Rect::from_min_max(Pos2::new(area.left(), top), area.max))
}

/// Sube de golpe a lo nuevo; baja como mucho `BAR_FALL_PER_SEC` × `dt`.
fn fall(bars: &mut [f32], levels: &[f32], dt: f32) {
    for (bar, &level) in bars.iter_mut().zip(levels) {
        *bar = level.max(*bar - viz::BAR_FALL_PER_SEC * dt).clamp(0.0, 1.0);
    }
}

/// Dónde empieza lo que se muestra de la onda: el primer cruce por cero
/// hacia arriba (de la señal pasada por un pasabajos) entre las primeras
/// `search` muestras, así una onda periódica queda quieta en pantalla. Sin
/// cruce, 0.
///
/// - Post: `≤ search` y `≤ samples.len()`.
fn trigger(samples: &[f32], search: usize) -> usize {
    let mut low = 0.0_f32;
    let mut armed = false;
    for (i, &x) in samples.iter().enumerate().take(search) {
        let before = low;
        low += viz::WAVE_TRIGGER_SMOOTH * (x - low);
        if low < -viz::WAVE_TRIGGER_HYST {
            armed = true;
        } else if armed && before < 0.0 && low >= 0.0 {
            return i;
        }
    }
    0
}

/// Pasa `samples` a los puntos de `wave` (cada uno, el promedio de su
/// tramo) y deja `blend` del valor anterior.
fn blend_wave(wave: &mut [f32], samples: &[f32], blend: f32) {
    let step = (samples.len() / wave.len().max(1)).max(1);
    for (point, chunk) in wave.iter_mut().zip(samples.chunks(step)) {
        let mean = chunk.iter().sum::<f32>() / chunk.len() as f32;
        *point = blend * *point + (1.0 - blend) * mean.clamp(-1.0, 1.0);
    }
}

/// Los puntos de `wave` a lo ancho de `rect`, con 0 en el medio y ±1 en
/// los bordes.
fn wave_points(wave: &[f32], rect: Rect) -> Vec<Pos2> {
    let half = rect.height() / 2.0 * 0.9;
    let last = (wave.len().max(2) - 1) as f32;
    wave.iter()
        .enumerate()
        .map(|(i, &y)| {
            Pos2::new(
                rect.left() + rect.width() * i as f32 / last,
                rect.center().y - y * half,
            )
        })
        .collect()
}

/// Disco con surcos y, en el centro, la tapa girada `angle` (o el acento
/// si no hay tapa). `pixel`: tamaño de un pixel en puntos.
fn vinyl(
    painter: &egui::Painter,
    rect: Rect,
    angle: f32,
    cover: Option<egui::TextureId>,
    pixel: f32,
    palette: &Palette,
) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0;
    painter.circle_filled(center, radius, color(palette.panel));
    let groove = Stroke::new(1.0_f32, color(palette.border));
    for k in 1..=VINYL_GROOVES {
        painter.circle_stroke(
            center,
            radius * (LABEL + (1.0 - LABEL) * k as f32 / (VINYL_GROOVES + 1) as f32),
            groove,
        );
    }
    let label = radius * LABEL;
    match cover {
        Some(texture) => {
            painter.add(Shape::mesh(label_mesh(
                texture, center, label, angle, pixel,
            )));
        }
        None => {
            painter.circle_filled(center, label, color(palette.accent).gamma_multiply(0.35));
            // Una marca, para que se vea girar.
            let mark = center + Vec2::angled(angle) * label * 0.7;
            painter.circle_filled(mark, label * 0.08, color(palette.accent));
        }
    }
    painter.circle_filled(center, radius * HOLE, color(palette.background));
}

/// La tapa recortada en un círculo de radio `radius`, girada `angle`, con
/// el borde suavizado: un anillo de `feather` de ancho que pasa de opaco a
/// transparente (como egui con sus propias figuras). Sin él, el borde de la
/// malla sale serruchado.
fn label_mesh(
    texture: egui::TextureId,
    center: Pos2,
    radius: f32,
    angle: f32,
    feather: f32,
) -> Mesh {
    let mut mesh = Mesh::with_texture(texture);
    let uv = |dir: Vec2, r: f32| Pos2::new(0.5, 0.5) + dir * (0.5 * r / radius);
    mesh.vertices.push(epaint::Vertex {
        pos: center,
        uv: Pos2::new(0.5, 0.5),
        color: Color32::WHITE,
    });
    let inner = (radius - feather / 2.0).max(0.0);
    let outer = radius + feather / 2.0;
    for k in 0..=VINYL_SEGMENTS {
        let theta = k as f32 / VINYL_SEGMENTS as f32 * TAU;
        let dir = Vec2::angled(theta);
        // La imagen gira con el disco.
        let tex_dir = Vec2::angled(theta - angle);
        mesh.vertices.push(epaint::Vertex {
            pos: center + dir * inner,
            uv: uv(tex_dir, inner),
            color: Color32::WHITE,
        });
        mesh.vertices.push(epaint::Vertex {
            pos: center + dir * outer,
            uv: uv(tex_dir, radius),
            color: Color32::TRANSPARENT,
        });
        if k > 0 {
            let (in_prev, out_prev) = (2 * k - 1, 2 * k);
            let (in_now, out_now) = (2 * k + 1, 2 * k + 2);
            mesh.add_triangle(0, in_prev, in_now);
            mesh.add_triangle(in_prev, out_prev, out_now);
            mesh.add_triangle(in_prev, out_now, in_now);
        }
    }
    mesh
}

/// Proporciones del vinilo respecto de su radio.
const LABEL: f32 = 0.62;
const HOLE: f32 = 0.03;
const VINYL_GROOVES: usize = 7;
/// Lados del círculo de la tapa: con 128 el contorno no se ve poligonal
/// ni con el panel grande.
const VINYL_SEGMENTS: u32 = 128;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_panel_solo_con_modo_y_lugar() {
        let wide = viz::PANEL_WIDTH + viz::MIN_CONSOLE_WIDTH;
        assert_eq!(
            panel_width(VizMode::None, false, viz::PANEL_WIDTH, 2000.0),
            None
        );
        assert_eq!(
            panel_width(VizMode::Bars, false, viz::PANEL_WIDTH, wide),
            Some(viz::PANEL_WIDTH)
        );
        // Angosta: el panel se achica para dejarle lugar a la consola...
        assert_eq!(
            panel_width(VizMode::Vinyl, false, viz::PANEL_WIDTH, wide - 50.0),
            Some(viz::PANEL_WIDTH - 50.0)
        );
        // ...y si ni el mínimo entra, no se muestra.
        let tight = viz::PANEL_WIDTH_MIN + viz::MIN_CONSOLE_WIDTH;
        assert_eq!(
            panel_width(VizMode::Wave, false, viz::PANEL_WIDTH, tight),
            Some(viz::PANEL_WIDTH_MIN)
        );
        assert_eq!(
            panel_width(VizMode::Wave, false, viz::PANEL_WIDTH, tight - 1.0),
            None
        );
        // Lo pedido se recorta al rango de config.
        assert_eq!(
            panel_width(VizMode::Bars, false, 1.0, 5000.0),
            Some(viz::PANEL_WIDTH_MIN)
        );
        assert_eq!(
            panel_width(VizMode::Bars, false, 1e6, 1e6),
            Some(viz::PANEL_WIDTH_MAX)
        );
    }

    #[test]
    fn la_columna_tambien_con_consumo_solo() {
        let wide = viz::PANEL_WIDTH + viz::MIN_CONSOLE_WIDTH;
        assert_eq!(
            panel_width(VizMode::None, true, viz::PANEL_WIDTH, wide),
            Some(viz::PANEL_WIDTH)
        );
        assert_eq!(
            panel_width(VizMode::Bars, true, viz::PANEL_WIDTH, wide),
            Some(viz::PANEL_WIDTH)
        );
        // Angosta: mismas reglas que sin consumo.
        let tight = viz::PANEL_WIDTH_MIN + viz::MIN_CONSOLE_WIDTH;
        assert_eq!(
            panel_width(VizMode::None, true, viz::PANEL_WIDTH, tight - 1.0),
            None
        );
    }

    #[test]
    fn la_visualizacion_se_achica_bajo_el_consumo() {
        let area = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(280.0, 400.0));
        assert_eq!(
            below(area, 100.0),
            Some(Rect::from_min_max(Pos2::new(10.0, 120.0), area.max))
        );
        // Justo el mínimo entra; uno menos, no.
        let used = 400.0 - viz::MIN_VIZ_HEIGHT;
        assert!(below(area, used).is_some());
        assert_eq!(below(area, used + 1.0), None);
    }

    #[test]
    fn las_barras_suben_de_golpe_y_bajan_suave() {
        let mut bars = [0.0, 0.8];
        fall(&mut bars, &[0.5, 0.0], 0.1);
        assert_eq!(bars[0], 0.5);
        assert!((bars[1] - (0.8 - viz::BAR_FALL_PER_SEC * 0.1)).abs() < 1e-6);
        fall(&mut bars, &[0.0, 0.0], 10.0);
        assert_eq!(bars, [0.0, 0.0]);
    }

    /// Un tono de `period` muestras, desde la muestra `offset`.
    fn tone(len: usize, period: f32, offset: usize) -> Vec<f32> {
        (offset..offset + len)
            .map(|i| 0.8 * (TAU * i as f32 / period).sin())
            .collect()
    }

    /// Lo que mostraría la onda (sin mezclar con el cuadro anterior).
    fn shown(samples: &[f32]) -> Vec<f32> {
        let start = trigger(samples, viz::WAVE_SEARCH);
        let mut wave = vec![0.0; viz::WAVE_POINTS];
        blend_wave(&mut wave, &samples[start..start + viz::WAVE_SAMPLES], 0.0);
        wave
    }

    #[test]
    fn onda_plana_con_silencio_y_senoidal_con_un_tono() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 100.0));
        let len = viz::WAVE_SEARCH + viz::WAVE_SAMPLES;
        let flat = wave_points(&shown(&vec![0.0; len]), rect);
        assert_eq!(flat.len(), viz::WAVE_POINTS);
        assert!(flat.iter().all(|p| (p.y - 50.0).abs() < 1e-4));
        let points = wave_points(&shown(&tone(len, 128.0, 0)), rect);
        let (min, max) = points.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
            (lo.min(p.y), hi.max(p.y))
        });
        assert!(min < 20.0 && max > 80.0, "{min} {max}");
    }

    #[test]
    fn la_onda_de_un_tono_no_tiembla_entre_cuadros() {
        let len = viz::WAVE_SEARCH + viz::WAVE_SAMPLES;
        // Cuadros que arrancan en cualquier fase del tono (a 15 fps entran
        // 2940 muestras por cuadro).
        let first = shown(&tone(len, 147.0, 0));
        for offset in [2940, 5880, 37, 101] {
            let other = shown(&tone(len, 147.0, offset));
            let worst = first
                .iter()
                .zip(&other)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f32, f32::max);
            assert!(worst < 0.05, "desde {offset}: {worst}");
        }
    }

    #[test]
    fn la_onda_mezcla_con_el_cuadro_anterior() {
        let mut wave = [1.0, 1.0];
        blend_wave(&mut wave, &[0.0, 0.0, 0.0, 0.0], 0.25);
        assert_eq!(wave, [0.25, 0.25]);
    }

    #[test]
    fn el_borde_de_la_tapa_se_desvanece() {
        let mesh = label_mesh(egui::TextureId::default(), Pos2::ZERO, 50.0, 0.3, 1.0);
        // Centro + un par (adentro opaco, afuera transparente) por lado.
        assert_eq!(mesh.vertices.len(), 1 + 2 * (VINYL_SEGMENTS as usize + 1));
        assert_eq!(mesh.indices.len(), 3 * 3 * VINYL_SEGMENTS as usize);
        for pair in mesh.vertices[1..].chunks(2) {
            assert_eq!(pair[0].color, Color32::WHITE);
            assert_eq!(pair[1].color, Color32::TRANSPARENT);
            assert!((pair[0].pos.to_vec2().length() - 49.5).abs() < 1e-3);
            assert!((pair[1].pos.to_vec2().length() - 50.5).abs() < 1e-3);
            // La imagen no se sale de la textura.
            for v in pair {
                assert!((v.uv - Pos2::new(0.5, 0.5)).length() <= 0.5 + 1e-4);
            }
        }
    }

    fn visualizer() -> Visualizer {
        Visualizer::new(AudioTap::new(viz::TAP_CAPACITY))
    }

    #[test]
    fn solo_pide_cuadros_mientras_suena() {
        let ctx = egui::Context::default();
        let mut v = visualizer();
        let palette = crate::config::SPOTIFY_DARK;
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0));
        for mode in [VizMode::Wave, VizMode::Bars, VizMode::Vinyl] {
            v.set_mode(mode);
            let mut asks = |track| {
                let mut wants = false;
                let _ = ctx.run_ui(Default::default(), |ui| {
                    wants = v.show(ui, rect, track, &palette);
                });
                wants
            };
            let loaded = |state| Track::Loaded { state, cover: None };
            assert!(asks(loaded(PlayState::Playing)), "{mode:?}");
            assert!(!asks(loaded(PlayState::Paused)), "{mode:?}");
            assert!(!asks(loaded(PlayState::Loading)), "{mode:?}");
            assert!(!asks(Track::Nothing), "{mode:?}");
        }
    }

    #[test]
    fn la_copia_de_muestras_solo_con_onda_o_barras() {
        let tap = AudioTap::new(4);
        let mut v = Visualizer::new(tap.clone());
        for (mode, copies) in [
            (VizMode::None, false),
            (VizMode::Wave, true),
            (VizMode::Bars, true),
            (VizMode::Vinyl, false),
        ] {
            v.set_mode(mode);
            assert_eq!(tap.push(&[0.5, 0.5], 1.0), copies, "{mode:?}");
        }
    }
}
