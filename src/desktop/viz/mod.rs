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
    levels: Vec<f32>,
    spectrum: Spectrum,
    /// Alto de cada barra, con caída suave.
    bars: Vec<f32>,
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
            samples: Vec::with_capacity(viz::FFT_SIZE),
            levels: vec![0.0; viz::BARS],
            spectrum: Spectrum::new(
                viz::FFT_SIZE,
                viz::BARS,
                (viz::BAR_MIN_HZ, viz::BAR_MAX_HZ),
                librespot_playback::SAMPLE_RATE as f32,
                viz::BAR_FLOOR_DB,
            ),
            bars: vec![0.0; viz::BARS],
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

    /// Ancho del panel en una ventana con `available` puntos de ancho, o
    /// `None` si no se muestra (Ninguna, o la consola quedaría más angosta
    /// que `MIN_CONSOLE_WIDTH`).
    pub(super) fn panel_width(&self, available: f32) -> Option<f32> {
        panel_width(self.mode, available)
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
                    self.samples.clear();
                    self.samples.resize(viz::FFT_SIZE, 0.0);
                } else {
                    self.snapshot();
                }
                painter.add(Shape::line(
                    wave_points(&self.samples, rect),
                    Stroke::new(1.5_f32, accent),
                ));
            }
            VizMode::Bars => {
                if matches!(track, Track::Nothing) {
                    self.bars.fill(0.0);
                } else if playing {
                    self.snapshot();
                    self.spectrum.bands(&self.samples, &mut self.levels);
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
                vinyl(&painter, rect, self.angle, cover, palette);
            }
        }
        playing
    }

    /// Las últimas `FFT_SIZE` muestras, atrasadas `LATENCY`.
    fn snapshot(&mut self) {
        let delay = (viz::LATENCY.as_secs_f32() * librespot_playback::SAMPLE_RATE as f32) as usize;
        self.tap.snapshot(&mut self.samples, viz::FFT_SIZE, delay);
    }
}

fn panel_width(mode: VizMode, available: f32) -> Option<f32> {
    (mode != VizMode::None && available - viz::PANEL_WIDTH >= viz::MIN_CONSOLE_WIDTH)
        .then_some(viz::PANEL_WIDTH)
}

/// Sube de golpe a lo nuevo; baja como mucho `BAR_FALL_PER_SEC` × `dt`.
fn fall(bars: &mut [f32], levels: &[f32], dt: f32) {
    for (bar, &level) in bars.iter_mut().zip(levels) {
        *bar = level.max(*bar - viz::BAR_FALL_PER_SEC * dt).clamp(0.0, 1.0);
    }
}

/// `WAVE_POINTS` puntos de `samples` (salteados) a lo ancho de `rect`,
/// con 0 en el medio y ±1 en los bordes.
fn wave_points(samples: &[f32], rect: Rect) -> Vec<Pos2> {
    let step = (samples.len() / viz::WAVE_POINTS).max(1);
    let half = rect.height() / 2.0 * 0.9;
    let last = (viz::WAVE_POINTS - 1).max(1) as f32;
    (0..viz::WAVE_POINTS)
        .map(|i| {
            let sample = samples
                .get(i * step)
                .copied()
                .unwrap_or(0.0)
                .clamp(-1.0, 1.0);
            Pos2::new(
                rect.left() + rect.width() * i as f32 / last,
                rect.center().y - sample * half,
            )
        })
        .collect()
}

/// Disco con surcos y, en el centro, la tapa girada `angle` (o el acento
/// si no hay tapa).
fn vinyl(
    painter: &egui::Painter,
    rect: Rect,
    angle: f32,
    cover: Option<egui::TextureId>,
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
            let mut mesh = Mesh::with_texture(texture);
            mesh.vertices.push(epaint::Vertex {
                pos: center,
                uv: Pos2::new(0.5, 0.5),
                color: Color32::WHITE,
            });
            for k in 0..=VINYL_SEGMENTS {
                let theta = k as f32 / VINYL_SEGMENTS as f32 * TAU;
                mesh.vertices.push(epaint::Vertex {
                    pos: center + Vec2::angled(theta) * label,
                    // La imagen gira con el disco.
                    uv: Pos2::new(0.5, 0.5) + Vec2::angled(theta - angle) * 0.5,
                    color: Color32::WHITE,
                });
                if k > 0 {
                    mesh.add_triangle(0, k, k + 1);
                }
            }
            painter.add(Shape::mesh(mesh));
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

/// Proporciones del vinilo respecto de su radio.
const LABEL: f32 = 0.62;
const HOLE: f32 = 0.03;
const VINYL_GROOVES: usize = 7;
/// Triángulos del círculo de la tapa.
const VINYL_SEGMENTS: u32 = 64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_panel_solo_con_modo_y_lugar() {
        let wide = viz::PANEL_WIDTH + viz::MIN_CONSOLE_WIDTH;
        assert_eq!(panel_width(VizMode::None, 2000.0), None);
        assert_eq!(panel_width(VizMode::Bars, wide), Some(viz::PANEL_WIDTH));
        assert_eq!(panel_width(VizMode::Vinyl, wide - 1.0), None);
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

    #[test]
    fn onda_plana_con_silencio_y_senoidal_con_un_tono() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 100.0));
        let flat = wave_points(&vec![0.0; viz::FFT_SIZE], rect);
        assert_eq!(flat.len(), viz::WAVE_POINTS);
        assert!(flat.iter().all(|p| (p.y - 50.0).abs() < 1e-4));
        let tone: Vec<f32> = (0..viz::FFT_SIZE)
            .map(|i| (TAU * i as f32 / 128.0).sin())
            .collect();
        let points = wave_points(&tone, rect);
        let (min, max) = points.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
            (lo.min(p.y), hi.max(p.y))
        });
        assert!(min < 10.0 && max > 90.0, "{min} {max}");
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
