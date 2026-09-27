//! Espectro para las barras (spec 010): FFT radix-2 escrita a mano (sin
//! `rustfft`: 1024 puntos 15 veces por segundo no justifican una
//! dependencia), ventana de Hann y bandas en escala logarítmica.

use std::f32::consts::PI;

/// Análisis de `size` muestras en `bars` bandas. Reserva sus buffers una
/// vez: `bands` no reserva memoria.
pub(super) struct Spectrum {
    hann: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
    /// Bins `[desde, hasta)` de cada banda.
    ranges: Vec<(usize, usize)>,
    /// Magnitud de un seno de amplitud 1 con la ventana de Hann: 0 dB.
    full_scale: f32,
    floor_db: f32,
}

impl Spectrum {
    /// - Pre: `size` potencia de 2 (≥ 4); `0 < min_hz < max_hz`;
    ///   `floor_db < 0`.
    /// - Post: `bars` bandas de `min_hz` a `max_hz` (recortado a Nyquist),
    ///   cada una con al menos un bin y sin pisarse con la anterior.
    pub(super) fn new(
        size: usize,
        bars: usize,
        (min_hz, max_hz): (f32, f32),
        sample_rate: f32,
        floor_db: f32,
    ) -> Self {
        let hann = (0..size)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / size as f32).cos())
            .collect();
        let half = size / 2;
        let max_hz = max_hz.min(sample_rate / 2.0);
        let bin = |hz: f32| -> usize {
            // Frecuencias positivas y chicas: entra en usize.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let bin = (hz * size as f32 / sample_rate).round() as usize;
            bin.clamp(1, half)
        };
        let ratio = (max_hz / min_hz).powf(1.0 / bars.max(1) as f32);
        let mut ranges = Vec::with_capacity(bars);
        let mut from = bin(min_hz);
        for i in 0..bars {
            let to = bin(min_hz * ratio.powi(i as i32 + 1))
                .max(from + 1)
                .min(half + 1);
            ranges.push((from.min(half), to));
            from = to;
        }
        Spectrum {
            hann,
            re: vec![0.0; size],
            im: vec![0.0; size],
            ranges,
            full_scale: size as f32 / 4.0,
            floor_db,
        }
    }

    /// Nivel de cada banda, de 0 (en `floor_db` o menos) a 1 (un seno de
    /// amplitud 1), según el pico de sus bins.
    ///
    /// - Pre: `samples.len()` = el `size` de `new`; `out.len()` = `bars`.
    /// - Post: cada valor en `[0, 1]`; silencio → todos 0.
    pub(super) fn bands(&mut self, samples: &[f32], out: &mut [f32]) {
        for (i, (re, im)) in self.re.iter_mut().zip(self.im.iter_mut()).enumerate() {
            *re = samples.get(i).copied().unwrap_or(0.0) * self.hann[i];
            *im = 0.0;
        }
        fft(&mut self.re, &mut self.im);
        for (level, &(from, to)) in out.iter_mut().zip(&self.ranges) {
            let peak = (from..to)
                .map(|k| (self.re[k] * self.re[k] + self.im[k] * self.im[k]).sqrt())
                .fold(0.0_f32, f32::max);
            *level = if peak <= 0.0 {
                0.0
            } else {
                let db = 20.0 * (peak / self.full_scale).log10();
                ((db - self.floor_db) / -self.floor_db).clamp(0.0, 1.0)
            };
        }
    }

    #[cfg(test)]
    fn band_of(&self, bin: usize) -> Option<usize> {
        self.ranges
            .iter()
            .position(|&(from, to)| (from..to).contains(&bin))
    }
}

/// FFT compleja en el lugar (Cooley-Tukey iterativa, radix-2).
///
/// - Pre: `re.len() == im.len()`, potencia de 2.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    // Orden de bits invertido.
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * PI / len as f32;
        let (w_im, w_re) = angle.sin_cos();
        for start in (0..n).step_by(len) {
            let (mut cur_re, mut cur_im) = (1.0_f32, 0.0_f32);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let t_re = re[b] * cur_re - im[b] * cur_im;
                let t_im = re[b] * cur_im + im[b] * cur_re;
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;
                let next_re = cur_re * w_re - cur_im * w_im;
                cur_im = cur_re * w_im + cur_im * w_re;
                cur_re = next_re;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::viz;

    const RATE: f32 = 44_100.0;

    fn spectrum() -> Spectrum {
        Spectrum::new(
            viz::FFT_SIZE,
            viz::BARS,
            (viz::BAR_MIN_HZ, viz::BAR_MAX_HZ),
            RATE,
            viz::BAR_FLOOR_DB,
        )
    }

    fn sine(hz: f32, amplitude: f32) -> Vec<f32> {
        (0..viz::FFT_SIZE)
            .map(|i| amplitude * (2.0 * PI * hz * i as f32 / RATE).sin())
            .collect()
    }

    #[test]
    fn la_fft_coincide_con_una_dft_directa() {
        let n = 64;
        let signal: Vec<f32> = (0..n).map(|i| ((i * 7 % 11) as f32 - 5.0) / 5.0).collect();
        let (mut re, mut im) = (signal.clone(), vec![0.0; n]);
        fft(&mut re, &mut im);
        for k in 0..n {
            let (mut dre, mut dim) = (0.0_f32, 0.0_f32);
            for (t, &x) in signal.iter().enumerate() {
                let angle = -2.0 * PI * (k * t) as f32 / n as f32;
                dre += x * angle.cos();
                dim += x * angle.sin();
            }
            assert!((re[k] - dre).abs() < 1e-3, "re[{k}]");
            assert!((im[k] - dim).abs() < 1e-3, "im[{k}]");
        }
    }

    #[test]
    fn silencio_da_todo_en_cero() {
        let mut s = spectrum();
        let mut out = [1.0; viz::BARS];
        s.bands(&vec![0.0; viz::FFT_SIZE], &mut out);
        assert_eq!(out, [0.0; viz::BARS]);
    }

    #[test]
    fn un_tono_de_1_khz_levanta_su_banda() {
        let mut s = spectrum();
        let mut out = [0.0; viz::BARS];
        s.bands(&sine(1000.0, 0.8), &mut out);
        let bin = (1000.0 * viz::FFT_SIZE as f32 / RATE).round() as usize;
        let band = s.band_of(bin).unwrap();
        let loudest = out
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert_eq!(loudest, band, "{out:?}");
        assert!(out[band] > 0.9, "{out:?}");
        // Lejos del tono, casi nada.
        assert!(out[0] < 0.3 && out[viz::BARS - 1] < 0.3, "{out:?}");
    }

    #[test]
    fn bandas_crecientes_sin_huecos_ni_pisadas() {
        let s = spectrum();
        assert_eq!(s.ranges.len(), viz::BARS);
        for pair in s.ranges.windows(2) {
            assert!(pair[0].0 < pair[0].1, "{pair:?}");
            assert_eq!(pair[0].1, pair[1].0, "{pair:?}");
        }
        assert!(s.ranges.last().unwrap().1 <= viz::FFT_SIZE / 2 + 1);
    }
}
