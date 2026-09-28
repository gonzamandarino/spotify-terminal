//! Salida de audio que sigue al dispositivo por defecto del sistema
//! (spec 010, AC-13).
//!
//! La salida de librespot (rodio) se queda con el dispositivo que había al
//! abrirla. Si se desconecta o Windows lo invalida, deja de pedir audio y
//! librespot espera a que se vacíe su cola para siempre: no suena más y el
//! hilo del reproductor queda colgado (pausa, stop y cerrar la app también).
//! Esta salida reabre el dispositivo por defecto cuando cambia o se rompe, y
//! nunca espera más que `config::output::STALL`.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Instant,
};

use cpal::traits::{DeviceTrait, HostTrait};
use librespot_playback::{
    NUM_CHANNELS, SAMPLE_RATE,
    audio_backend::{Sink, SinkError, SinkResult},
    convert::Converter,
    decoder::AudioPacket,
};

use crate::config::output::{DEVICE_CHECK, POLL, QUEUE_PACKETS, STALL};

/// De dónde salen los dispositivos. Real: [`System`]; en los tests, uno
/// falso.
pub(crate) trait Devices {
    type Out: Output;
    /// Nombre del dispositivo de salida por defecto, si hay.
    fn default_name(&self) -> Option<String>;
    /// Abre el dispositivo por defecto, sonando.
    fn open(&self) -> Result<Self::Out, String>;
}

/// Un dispositivo abierto.
pub(crate) trait Output {
    /// Nombre del dispositivo, para ver si dejó de ser el de por defecto.
    fn name(&self) -> Option<&str>;
    /// `true` si el dispositivo avisó un error (se desconectó, etc.).
    fn failed(&self) -> bool;
    /// Paquetes en cola, sin tocar todavía.
    fn queued(&self) -> usize;
    fn append(&self, samples: Vec<f32>);
    fn play(&self);
    fn pause(&self);
}

/// Salida que sigue al dispositivo por defecto.
///
/// Invariante: `write`, `start` y `stop` vuelven en a lo sumo `STALL` (+
/// lo que tarde abrir un dispositivo): el hilo de audio de librespot nunca
/// queda colgado de un dispositivo que dejó de sonar.
pub(crate) struct DeviceSink<D: Devices = System> {
    devices: D,
    out: Option<D::Out>,
    /// Próxima vez que se mira si cambió el dispositivo por defecto.
    next_check: Instant,
}

impl DeviceSink {
    /// Salida al dispositivo por defecto del sistema. No lo abre hasta que
    /// hace falta (`start` o `write`).
    pub(crate) fn system() -> Self {
        DeviceSink::new(System(cpal::default_host()))
    }
}

impl<D: Devices> DeviceSink<D> {
    fn new(devices: D) -> Self {
        DeviceSink {
            devices,
            out: None,
            next_check: Instant::now(),
        }
    }

    /// El dispositivo abierto, reabriendo el de por defecto si no había,
    /// si el abierto falló o si el de por defecto ahora es otro (esto
    /// último se mira cada `DEVICE_CHECK`).
    ///
    /// - Errores: no hay dispositivo o no se pudo abrir → `ConnectionRefused`
    ///   (librespot pausa; al reanudar se vuelve a intentar).
    fn output(&mut self) -> SinkResult<&D::Out> {
        let now = Instant::now();
        let stale = match &self.out {
            None => true,
            Some(out) if out.failed() => true,
            Some(out) if now >= self.next_check => {
                self.next_check = now + DEVICE_CHECK;
                self.devices.default_name().as_deref() != out.name()
            }
            Some(_) => false,
        };
        if stale {
            // Primero se suelta el viejo: algunos dispositivos no se dejan
            // abrir dos veces.
            self.out = None;
            self.out = Some(self.devices.open().map_err(SinkError::ConnectionRefused)?);
            self.next_check = now + DEVICE_CHECK;
        }
        self.out
            .as_ref()
            .ok_or_else(|| SinkError::NotConnected("sin dispositivo de salida".into()))
    }
}

impl<D: Devices> Sink for DeviceSink<D> {
    fn start(&mut self) -> SinkResult<()> {
        self.output()?.play();
        Ok(())
    }

    /// Deja sonar lo que quedó en cola (como librespot) y pausa, sin
    /// esperar más que `STALL` a un dispositivo que no avanza.
    fn stop(&mut self) -> SinkResult<()> {
        if let Some(out) = &self.out {
            let deadline = Instant::now() + STALL;
            while out.queued() > 0 && !out.failed() && Instant::now() < deadline {
                thread::sleep(POLL);
            }
            out.pause();
        }
        Ok(())
    }

    /// - Post: el paquete quedó en la cola del dispositivo por defecto, o
    ///   se perdió porque el dispositivo dejó de sonar (se reabre en el
    ///   próximo). La cola no pasa de `QUEUE_PACKETS`.
    fn write(&mut self, packet: AudioPacket, converter: &mut Converter) -> SinkResult<()> {
        let samples = packet
            .samples()
            .map_err(|e| SinkError::OnWrite(e.to_string()))?;
        let samples = converter.f64_to_f32(samples);
        let out = self.output()?;
        out.append(samples);
        let deadline = Instant::now() + STALL;
        let mut stalled = false;
        while out.queued() > QUEUE_PACKETS {
            if out.failed() || Instant::now() >= deadline {
                stalled = true;
                break;
            }
            thread::sleep(POLL);
        }
        if stalled {
            self.out = None;
        }
        Ok(())
    }
}

/// Dispositivos de verdad (cpal + rodio).
pub(crate) struct System(cpal::Host);

impl Devices for System {
    type Out = RodioOut;

    fn default_name(&self) -> Option<String> {
        self.0.default_output_device()?.name().ok()
    }

    fn open(&self) -> Result<RodioOut, String> {
        let device = self
            .0
            .default_output_device()
            .ok_or("no hay dispositivo de salida de audio")?;
        let name = device.name().ok();
        let failed = Arc::new(AtomicBool::new(false));
        let flag = failed.clone();
        // Corre en el hilo de cpal: solo anota.
        let on_error = move |_: cpal::StreamError| flag.store(true, Ordering::Relaxed);
        // Como librespot: estéreo a 44,1 kHz si el dispositivo lo acepta;
        // si no, a su frecuencia (rodio convierte); si no, lo que tenga.
        let default = device.default_output_config().map_err(|e| e.to_string())?;
        let config = device
            .supported_output_configs()
            .map_err(|e| e.to_string())?
            .find(|c| c.channels() == cpal::ChannelCount::from(NUM_CHANNELS))
            .and_then(|c| {
                c.try_with_sample_rate(cpal::SampleRate(SAMPLE_RATE))
                    .or_else(|| c.try_with_sample_rate(default.sample_rate()))
            })
            .unwrap_or(default);
        let mut stream = rodio::OutputStreamBuilder::default()
            .with_device(device)
            .with_config(&config.config())
            .with_sample_format(cpal::SampleFormat::F32)
            .with_error_callback(on_error)
            .open_stream_or_fallback()
            .map_err(|e| e.to_string())?;
        stream.log_on_drop(false);
        let sink = rodio::Sink::connect_new(stream.mixer());
        Ok(RodioOut {
            sink,
            name,
            failed,
            _stream: stream,
        })
    }
}

/// Un dispositivo abierto con rodio.
pub(crate) struct RodioOut {
    sink: rodio::Sink,
    name: Option<String>,
    failed: Arc<AtomicBool>,
    // Al soltarlo se cierra el dispositivo: va último.
    _stream: rodio::OutputStream,
}

impl Output for RodioOut {
    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }

    fn queued(&self) -> usize {
        self.sink.len()
    }

    fn append(&self, samples: Vec<f32>) {
        self.sink.append(rodio::buffer::SamplesBuffer::new(
            rodio::ChannelCount::from(NUM_CHANNELS),
            SAMPLE_RATE,
            samples,
        ));
    }

    fn play(&self) {
        self.sink.play();
    }

    fn pause(&self) {
        self.sink.pause();
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc, time::Duration};

    use super::*;

    /// Estado compartido de los dispositivos falsos.
    #[derive(Default)]
    struct World {
        default: Option<String>,
        opened: Vec<String>,
        /// El abierto no toca nada (la cola no baja).
        stuck: bool,
    }

    #[derive(Clone, Default)]
    struct Fake(Rc<RefCell<World>>);

    struct FakeOut {
        name: String,
        world: Rc<RefCell<World>>,
        queued: RefCell<usize>,
        failed: Rc<RefCell<bool>>,
    }

    impl Devices for Fake {
        type Out = FakeOut;

        fn default_name(&self) -> Option<String> {
            self.0.borrow().default.clone()
        }

        fn open(&self) -> Result<FakeOut, String> {
            let name = self.default_name().ok_or("sin dispositivo")?;
            self.0.borrow_mut().opened.push(name.clone());
            Ok(FakeOut {
                name,
                world: self.0.clone(),
                queued: RefCell::new(0),
                failed: Rc::default(),
            })
        }
    }

    impl Output for FakeOut {
        fn name(&self) -> Option<&str> {
            Some(&self.name)
        }
        fn failed(&self) -> bool {
            *self.failed.borrow()
        }
        fn queued(&self) -> usize {
            // Un dispositivo sano toca un paquete cada vez que se mira.
            let mut queued = self.queued.borrow_mut();
            if !self.world.borrow().stuck {
                *queued = queued.saturating_sub(1);
            }
            *queued
        }
        fn append(&self, _: Vec<f32>) {
            *self.queued.borrow_mut() += 1;
        }
        fn play(&self) {}
        fn pause(&self) {}
    }

    fn world(default: &str) -> (Fake, DeviceSink<Fake>) {
        let fake = Fake::default();
        fake.0.borrow_mut().default = Some(default.into());
        (fake.clone(), DeviceSink::new(fake))
    }

    fn write(sink: &mut DeviceSink<Fake>) -> SinkResult<()> {
        sink.write(
            AudioPacket::Samples(vec![0.0; 4]),
            &mut Converter::new(None),
        )
    }

    fn opened(fake: &Fake) -> Vec<String> {
        fake.0.borrow().opened.clone()
    }

    #[test]
    fn abre_recien_al_sonar_y_una_sola_vez() {
        let (fake, mut sink) = world("parlantes");
        assert!(opened(&fake).is_empty());
        sink.start().unwrap();
        for _ in 0..50 {
            write(&mut sink).unwrap();
        }
        assert_eq!(opened(&fake), ["parlantes"]);
    }

    #[test]
    fn se_pasa_al_nuevo_dispositivo_por_defecto() {
        let (fake, mut sink) = world("parlantes");
        sink.start().unwrap();
        write(&mut sink).unwrap();
        fake.0.borrow_mut().default = Some("auriculares".into());
        // Antes de `DEVICE_CHECK` no se mira.
        write(&mut sink).unwrap();
        assert_eq!(opened(&fake), ["parlantes"]);
        sink.next_check = Instant::now();
        write(&mut sink).unwrap();
        assert_eq!(opened(&fake), ["parlantes", "auriculares"]);
    }

    #[test]
    fn un_dispositivo_que_falla_se_reabre() {
        let (fake, mut sink) = world("parlantes");
        write(&mut sink).unwrap();
        if let Some(out) = &sink.out {
            *out.failed.borrow_mut() = true;
        }
        write(&mut sink).unwrap();
        assert_eq!(opened(&fake), ["parlantes", "parlantes"]);
    }

    #[test]
    fn un_dispositivo_que_no_avanza_no_cuelga_la_escritura() {
        let (fake, mut sink) = world("parlantes");
        fake.0.borrow_mut().stuck = true;
        let started = Instant::now();
        // El que pasa la cola del tope espera.
        for _ in 0..=QUEUE_PACKETS {
            write(&mut sink).unwrap();
        }
        // Se cortó a los `STALL` y se suelta para reabrir.
        assert!(started.elapsed() < STALL + Duration::from_millis(500));
        assert!(sink.out.is_none());
        let started = Instant::now();
        sink.stop().unwrap();
        assert!(started.elapsed() < Duration::from_millis(100));
        write(&mut sink).unwrap();
        assert_eq!(opened(&fake), ["parlantes", "parlantes"]);
    }

    #[test]
    fn stop_no_espera_para_siempre() {
        let (fake, mut sink) = world("parlantes");
        write(&mut sink).unwrap();
        fake.0.borrow_mut().stuck = true;
        let started = Instant::now();
        sink.stop().unwrap();
        assert!(started.elapsed() < STALL + Duration::from_millis(500));
    }

    #[test]
    fn sin_dispositivo_da_error_y_se_reintenta_despues() {
        let (fake, mut sink) = world("parlantes");
        fake.0.borrow_mut().default = None;
        assert!(matches!(sink.start(), Err(SinkError::ConnectionRefused(_))));
        assert!(write(&mut sink).is_err());
        fake.0.borrow_mut().default = Some("auriculares".into());
        sink.start().unwrap();
        assert_eq!(opened(&fake), ["auriculares"]);
    }
}
