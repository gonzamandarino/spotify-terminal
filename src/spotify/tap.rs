//! Copia de las muestras que salen al audio, para dibujarlas (spec 010).
//!
//! El hilo de audio de librespot escribe (`push`) y la ventana lee
//! (`snapshot`). El que escribe nunca espera: la reproducción gana siempre.

use std::sync::{
    Arc, Mutex, TryLockError,
    atomic::{AtomicBool, Ordering},
};

use librespot_playback::{
    audio_backend::{Sink, SinkResult},
    convert::Converter,
    decoder::AudioPacket,
    mixer::VolumeGetter,
};

/// Últimas muestras que salieron al audio, en mono y sin el volumen
/// aplicado (la forma no depende del volumen, spec 010 AC-6).
///
/// Se clona barato: todos los clones comparten el mismo buffer.
#[derive(Clone)]
pub struct AudioTap(Arc<Shared>);

struct Shared {
    ring: Mutex<Ring>,
    /// Apagado (sin visualización), `push` no copia nada.
    on: AtomicBool,
}

struct Ring {
    /// Buffer circular de `capacity` muestras.
    samples: Vec<f32>,
    /// Dónde va la próxima.
    next: usize,
}

impl AudioTap {
    /// Buffer para las últimas `capacity` muestras, en silencio y
    /// apagado.
    ///
    /// - Pre: `capacity > 0`.
    pub fn new(capacity: usize) -> Self {
        AudioTap(Arc::new(Shared {
            ring: Mutex::new(Ring {
                samples: vec![0.0; capacity.max(1)],
                next: 0,
            }),
            on: AtomicBool::new(false),
        }))
    }

    /// Prende o apaga la copia. Apagado, `push` vuelve enseguida (sin
    /// visualización no se paga nada) y al prenderse el buffer arranca en
    /// silencio.
    pub fn set_enabled(&self, on: bool) {
        if on && !self.0.on.load(Ordering::Relaxed) {
            if let Ok(mut ring) = self.0.ring.lock() {
                ring.samples.fill(0.0);
            }
        }
        self.0.on.store(on, Ordering::Relaxed);
    }

    /// Suma un paquete de muestras estéreo intercaladas (L, R, L, R...) que
    /// salieron con el volumen `gain` (0–1, el factor que aplicó librespot).
    ///
    /// - Post: el buffer tiene al final el promedio L/R de cada par,
    ///   dividido por `gain` (con `gain` ~0, ceros: no se inventa señal).
    /// - Devuelve: `false` si está apagado o el buffer estaba tomado por el
    ///   lector, y el paquete no se copió.
    /// - No debe: esperar a nadie (corre en el hilo de audio) ni reservar
    ///   memoria.
    pub fn push(&self, interleaved: &[f64], gain: f64) -> bool {
        if !self.0.on.load(Ordering::Relaxed) {
            return false;
        }
        let mut ring = match self.0.ring.try_lock() {
            Ok(ring) => ring,
            // Envenenado: un lector entró en pánico. El audio sigue igual.
            Err(TryLockError::WouldBlock | TryLockError::Poisoned(_)) => return false,
        };
        let scale = if gain > f64::EPSILON { 1.0 / gain } else { 0.0 };
        let len = ring.samples.len();
        for pair in interleaved.chunks_exact(2) {
            // Precisión de f32 alcanza para dibujar.
            #[allow(clippy::cast_possible_truncation)]
            let mono = ((pair[0] + pair[1]) * 0.5 * scale) as f32;
            let next = ring.next;
            ring.samples[next] = mono;
            ring.next = (next + 1) % len;
        }
        true
    }

    /// Copia en `out` las `count` muestras que salieron hace `delay`
    /// muestras (0 = las últimas), de la más vieja a la más nueva.
    ///
    /// - Pre: `count + delay` ≤ la capacidad; si no, se recorta `delay`.
    /// - Post: `out.len() == count`.
    /// - No debe: tener tomado el buffer más que lo que dura la copia.
    pub fn snapshot(&self, out: &mut Vec<f32>, count: usize, delay: usize) {
        out.clear();
        let Ok(ring) = self.0.ring.lock() else {
            out.resize(count, 0.0);
            return;
        };
        let len = ring.samples.len();
        let count = count.min(len);
        let delay = delay.min(len - count);
        let start = (ring.next + len - delay - count) % len;
        out.extend((0..count).map(|i| ring.samples[(start + i) % len]));
    }
}

/// Salida de audio que copia cada paquete al [`AudioTap`] y lo pasa tal
/// cual a la salida de verdad.
pub(crate) struct TapSink {
    inner: Box<dyn Sink>,
    tap: AudioTap,
    volume: Box<dyn VolumeGetter + Send>,
}

impl TapSink {
    /// Envuelve `inner`. `volume` da el factor que librespot aplica a las
    /// muestras antes de mandarlas (para sacarlo en el tap).
    pub(crate) fn new(
        inner: Box<dyn Sink>,
        tap: AudioTap,
        volume: Box<dyn VolumeGetter + Send>,
    ) -> Self {
        TapSink { inner, tap, volume }
    }
}

impl Sink for TapSink {
    fn start(&mut self) -> SinkResult<()> {
        self.inner.start()
    }

    fn stop(&mut self) -> SinkResult<()> {
        self.inner.stop()
    }

    /// - Post: `inner` recibe el mismo paquete que habría recibido sin el
    ///   tap (spec 010, AC-9); el tap se lleva una copia si está libre.
    fn write(&mut self, packet: AudioPacket, converter: &mut Converter) -> SinkResult<()> {
        if let Ok(samples) = packet.samples() {
            self.tap.push(samples, self.volume.attenuation_factor());
        }
        self.inner.write(packet, converter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(tap: &AudioTap, count: usize, delay: usize) -> Vec<f32> {
        let mut out = Vec::new();
        tap.snapshot(&mut out, count, delay);
        out
    }

    fn on(capacity: usize) -> AudioTap {
        let tap = AudioTap::new(capacity);
        tap.set_enabled(true);
        tap
    }

    #[test]
    fn apagado_no_copia() {
        let tap = AudioTap::new(2);
        assert!(!tap.push(&[1.0, 1.0], 1.0));
        assert_eq!(snap(&tap, 2, 0), [0.0, 0.0]);
        tap.set_enabled(true);
        assert!(tap.push(&[1.0, 1.0], 1.0));
        // Al apagar y volver a prender, arranca en silencio.
        tap.set_enabled(false);
        tap.set_enabled(true);
        assert_eq!(snap(&tap, 2, 0), [0.0, 0.0]);
    }

    #[test]
    fn guarda_las_ultimas_en_mono_y_sin_volumen() {
        let tap = on(4);
        assert_eq!(snap(&tap, 4, 0), [0.0; 4]);
        // Pares (L, R) con volumen 0,5: mono = (L+R)/2 / 0,5.
        assert!(tap.push(&[0.1, 0.3, 0.2, 0.2, -0.4, 0.0], 0.5));
        assert_eq!(snap(&tap, 3, 0), [0.4, 0.4, -0.4]);
        // Da la vuelta: quedan las 4 últimas, de la más vieja a la nueva.
        assert!(tap.push(&[0.5, 0.5, 0.25, 0.25], 1.0));
        assert_eq!(snap(&tap, 4, 0), [0.4, -0.4, 0.5, 0.25]);
        assert_eq!(snap(&tap, 2, 1), [-0.4, 0.5]);
        // Pedir de más se recorta.
        assert_eq!(snap(&tap, 9, 9).len(), 4);
    }

    #[test]
    fn con_volumen_cero_no_inventa_senal() {
        let tap = on(2);
        tap.push(&[0.0001, 0.0001, 0.0, 0.0], 0.0);
        assert_eq!(snap(&tap, 2, 0), [0.0, 0.0]);
    }

    #[test]
    fn con_el_buffer_tomado_no_espera() {
        let tap = on(4);
        let held = tap.0.ring.lock().unwrap();
        assert!(!tap.push(&[1.0, 1.0], 1.0));
        drop(held);
        assert_eq!(snap(&tap, 1, 0), [0.0]);
    }

    /// Salida falsa que anota lo que recibe.
    struct Recorder(Arc<Mutex<Vec<f64>>>);

    impl Sink for Recorder {
        fn write(&mut self, packet: AudioPacket, _: &mut Converter) -> SinkResult<()> {
            self.0.lock().unwrap().extend(packet.samples().unwrap());
            Ok(())
        }
    }

    struct Half;

    impl VolumeGetter for Half {
        fn attenuation_factor(&self) -> f64 {
            0.5
        }
    }

    #[test]
    fn la_salida_recibe_el_mismo_paquete_aunque_el_tap_este_tomado() {
        let got = Arc::new(Mutex::new(Vec::new()));
        let tap = on(8);
        let mut sink = TapSink::new(Box::new(Recorder(got.clone())), tap.clone(), Box::new(Half));
        let mut converter = Converter::new(None);
        let packet = vec![0.1, -0.2, 0.3, 0.4];
        sink.write(AudioPacket::Samples(packet.clone()), &mut converter)
            .unwrap();
        {
            let _held = tap.0.ring.lock().unwrap();
            sink.write(AudioPacket::Samples(packet.clone()), &mut converter)
                .unwrap();
        }
        let mut twice = packet.clone();
        twice.extend(&packet);
        assert_eq!(*got.lock().unwrap(), twice);
        // Solo el primero llegó al tap, sin el volumen.
        let copy = snap(&tap, 4, 0);
        assert!((copy[2] - (-0.1)).abs() < 1e-6, "{copy:?}");
        assert!((copy[3] - 0.7).abs() < 1e-6, "{copy:?}");
    }
}
