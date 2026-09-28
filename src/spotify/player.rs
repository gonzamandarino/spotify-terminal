//! Reproductor: sesión de librespot + salida de audio local.
//!
//! La I/O de la sesión (conexión con Spotify, claves de audio, metadata)
//! corre en un hilo propio con su runtime (`AudioRuntime`), no en el de
//! `main`: si `main` se bloquea (login, disco, render de la UI), la
//! reproducción no se entera. Ver `docs/decisiones.md`.

use std::{
    future::Future,
    sync::{Arc, mpsc},
    thread,
};

use cpal::traits::HostTrait;
use librespot_core::{
    SpotifyUri,
    authentication::Credentials,
    config::SessionConfig,
    error::{Error as CoreError, ErrorKind},
    session::Session,
};
use librespot_metadata::{Album, Metadata, Playlist};
use librespot_playback::{
    audio_backend::Sink,
    config::{Bitrate, PlayerConfig},
    mixer::{Mixer, MixerConfig, softmixer::SoftMixer},
    player::{self, PlayerEventChannel},
};
use tokio::{runtime, sync::oneshot};

use crate::{
    app::volume::Volume,
    config,
    error::AppError,
    spotify::{
        auth::Token,
        output::DeviceSink,
        tap::{AudioTap, TapSink},
    },
};

/// Reproductor conectado a Spotify y a la salida de audio por defecto.
///
/// Invariante: la sesión está autenticada al crearse el `Player`; puede
/// perderse después (ver [`Player::session_lost`]).
pub struct Player {
    // El orden de los campos es el orden en que se sueltan: primero el
    // reproductor (espera a sus hilos), después la sesión y al final el
    // runtime que la atiende.
    //
    // `Arc` porque librespot devuelve el reproductor compartido entre su
    // hilo de audio y nosotros.
    inner: Arc<player::Player>,
    /// Volumen por software (curva logarítmica): se aplica a cada muestra
    /// antes de la salida, así que un cambio se oye enseguida.
    mixer: SoftMixer,
    session: Session,
    runtime: AudioRuntime,
}

/// Temas que resolvió [`Player::resolve_tracks`].
#[derive(Debug)]
pub struct Resolved {
    /// Temas reproducibles, en orden. Nunca vacío.
    pub tracks: Vec<SpotifyUri>,
    /// Elementos de la playlist que no se van a reproducir (archivos
    /// locales, o que Spotify no devolvió). 0 para temas y álbumes.
    pub skipped: usize,
}

impl Player {
    /// Abre la sesión de librespot con el token y prepara la salida de audio.
    ///
    /// - Pre: `token` vigente de tipo `TokenKind::Audio` (con otro
    ///   Client ID la sesión conecta pero no carga audio: spike T2).
    /// - Post: reproductor listo, sin nada cargado, con `volume` y calidad
    ///   `bitrate` (fija mientras viva: para otra, otro `connect`); la
    ///   sesión corre en su propio hilo. La salida es el dispositivo por
    ///   defecto del sistema y lo sigue si cambia o se desconecta
    ///   (`DeviceSink`, spec 010 AC-13); si no queda ninguno, librespot
    ///   pausa. Con `tap`, cada paquete que sale al audio se copia ahí sin
    ///   demorarlo (`TapSink`, spec 010).
    /// - Errores: sin dispositivo de salida → `NoAudioOutput` (se chequea
    ///   antes de conectar); Spotify rechaza el token → `SessionRejected`;
    ///   no se llega al servidor → `Network`.
    /// - No debe: empezar a reproducir.
    pub async fn connect(
        token: &Token,
        volume: Volume,
        bitrate: Bitrate,
        tap: Option<AudioTap>,
    ) -> Result<Player, AppError> {
        // Sin esto, rodio hace panic en el hilo del reproductor al abrir la
        // salida y el error llega tarde y confuso.
        if cpal::default_host().default_output_device().is_none() {
            return Err(AppError::NoAudioOutput);
        }

        let runtime = AudioRuntime::start()?;
        let credentials = Credentials::with_access_token(token.access_token());
        // `Session::new` guarda el runtime actual: tiene que crearse adentro.
        let session = runtime
            .run(async move {
                let session = Session::new(SessionConfig::default(), None);
                session.connect(credentials, false).await.map(|()| session)
            })
            .await?
            .map_err(|e| session_error(&e))?;

        let player_config = PlayerConfig {
            bitrate,
            ..PlayerConfig::default()
        };
        let mixer = SoftMixer::open(MixerConfig::default())
            .map_err(|e| AppError::Internal(format!("control de volumen: {e}")))?;
        // Arranca en 50 %: se fija antes de que pueda sonar nada.
        mixer.set_volume(volume.output());
        let tap_volume = mixer.get_soft_volume();
        let inner = player::Player::new(
            player_config,
            session.clone(),
            mixer.get_soft_volume(),
            move || {
                let sink: Box<dyn Sink> = Box::new(DeviceSink::system());
                match tap {
                    Some(tap) => Box::new(TapSink::new(sink, tap, tap_volume)),
                    None => sink,
                }
            },
        );
        Ok(Player {
            inner,
            mixer,
            session,
            runtime,
        })
    }

    /// Temas a reproducir, en orden, para lo que pidió el usuario.
    ///
    /// - Pre: `uri` es un tema, álbum o playlist.
    /// - Post: `Resolved` con al menos un tema reproducible (temas y
    ///   episodios; los archivos locales de una playlist se omiten y se
    ///   cuentan en `skipped`). Un tema devuelve solo a sí mismo sin
    ///   consultar a Spotify.
    /// - Errores: no existe → `NotFound`; sin temas → `NothingToPlay`; otro
    ///   tipo de URI → `Usage`; resto → ver `metadata_error`.
    pub async fn resolve_tracks(&self, uri: &SpotifyUri) -> Result<Resolved, AppError> {
        let session = self.session.clone();
        let target = uri.clone();
        let (tracks, expected) = match uri {
            SpotifyUri::Track { .. } => (vec![uri.clone()], 1),
            SpotifyUri::Album { .. } => {
                let tracks = self
                    .runtime
                    .run(async move {
                        let album = Album::get(&session, &target).await?;
                        Ok::<_, CoreError>(album.tracks().cloned().collect::<Vec<_>>())
                    })
                    .await?
                    .map_err(|e| metadata_error(&e, uri))?;
                let expected = tracks.len();
                (tracks, expected)
            }
            SpotifyUri::Playlist { .. } => self
                .runtime
                .run(async move {
                    let playlist = Playlist::get(&session, &target).await?;
                    let tracks = playlist
                        .tracks()
                        .filter(|t| {
                            matches!(t, SpotifyUri::Track { .. } | SpotifyUri::Episode { .. })
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    // `length` es cuántos elementos tiene la playlist según
                    // Spotify; puede ser más de los que devolvió.
                    let expected = usize::try_from(playlist.length).unwrap_or(0);
                    Ok::<_, CoreError>((tracks, expected))
                })
                .await?
                .map_err(|e| metadata_error(&e, uri))?,
            _ => {
                return Err(AppError::Usage(format!(
                    "no se puede reproducir {}",
                    describe(uri)
                )));
            }
        };
        if tracks.is_empty() {
            return Err(AppError::NothingToPlay(describe(uri)));
        }
        let skipped = expected.saturating_sub(tracks.len());
        Ok(Resolved { tracks, skipped })
    }

    /// `true` si la sesión con Spotify se cortó: los temas que se carguen
    /// a partir de ahora van a fallar. No reconecta.
    pub fn session_lost(&self) -> bool {
        self.session.is_invalid()
    }

    /// Canal de eventos del reproductor (cambio de tema, play/pausa, fin...).
    /// Cada llamada crea un canal nuevo que recibe los eventos desde ese
    /// momento. Se cierra (`recv` → `None`) si el hilo del reproductor
    /// termina, p. ej. porque falló la salida de audio.
    pub fn events(&self) -> PlayerEventChannel {
        self.inner.get_player_event_channel()
    }

    /// Carga `track` y empieza a reproducirlo desde el principio. Si estaba
    /// precargado con [`Player::preload`], arranca sin demora. El resultado
    /// llega por [`Player::events`] con un `play_request_id` nuevo
    /// (`PlayRequestIdChanged`).
    pub fn play(&self, track: SpotifyUri) {
        self.inner.load(track, true, 0);
    }

    /// Empieza a bajar `track` mientras suena el actual, para que el paso
    /// al siguiente tema (`play`) no tenga silencio. No cambia lo que suena.
    pub fn preload(&self, track: SpotifyUri) {
        self.inner.preload(track);
    }

    /// Pausa lo que suena (evento `Paused`). Sin efecto si no hay nada
    /// cargado.
    pub fn pause(&self) {
        self.inner.pause();
    }

    /// Reanuda lo pausado (evento `Playing`).
    pub fn resume(&self) {
        self.inner.play();
    }

    /// Vuelve al principio del tema actual (evento `Seeked`), sin cambiar
    /// si está en pausa o sonando. Sin efecto si no hay nada cargado.
    pub fn restart(&self) {
        self.inner.seek(0);
    }

    /// Corta la reproducción y descarga el tema actual.
    pub fn stop(&self) {
        self.inner.stop();
    }

    /// Cambia el volumen de lo que suena y de lo que venga, sin cortar el
    /// audio. Solo afecta a esta app, no al volumen de Windows.
    pub fn set_volume(&self, volume: Volume) {
        self.mixer.set_volume(volume.output());
    }
}

/// Hilo con un runtime tokio de un solo hilo, dedicado a la sesión de audio.
///
/// Invariante: el runtime vive mientras exista el valor; al soltarlo se
/// apaga y se espera al hilo.
struct AudioRuntime {
    handle: runtime::Handle,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl AudioRuntime {
    fn start() -> Result<AudioRuntime, AppError> {
        let (handle_tx, handle_rx) = mpsc::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let thread = thread::Builder::new()
            .name("sesion-audio".into())
            .spawn(move || {
                let rt = match runtime::Builder::new_current_thread().enable_all().build() {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = handle_tx.send(Err(e));
                        return;
                    }
                };
                let _ = handle_tx.send(Ok(rt.handle().clone()));
                // Atiende las tareas de la sesión hasta que nos suelten.
                rt.block_on(async {
                    let _ = shutdown_rx.await;
                });
            })
            .map_err(|e| AppError::Internal(format!("hilo de la sesión de audio: {e}")))?;
        let handle = handle_rx
            .recv()
            .map_err(|_| AppError::Internal("el hilo de la sesión de audio no arrancó".into()))?
            .map_err(|e| AppError::Internal(format!("runtime de la sesión de audio: {e}")))?;
        Ok(AudioRuntime {
            handle,
            shutdown: Some(shutdown_tx),
            thread: Some(thread),
        })
    }

    /// Corre `task` en el hilo de audio y espera su resultado.
    async fn run<F>(&self, task: F) -> Result<F::Output, AppError>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.handle
            .spawn(task)
            .await
            .map_err(|e| AppError::Internal(format!("tarea de la sesión de audio: {e}")))
    }
}

impl Drop for AudioRuntime {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Errores de la sesión de librespot: credenciales rechazadas vs. no se
/// pudo hablar con Spotify.
fn session_error(error: &CoreError) -> AppError {
    match error.kind {
        ErrorKind::Unauthenticated | ErrorKind::PermissionDenied => {
            AppError::SessionRejected(error.to_string())
        }
        _ => AppError::Network(error.to_string()),
    }
}

/// Errores al pedir metadata con la sesión ya abierta.
///
/// - `NotFound`, o una respuesta sin datos (`FailedPrecondition`) → lo
///   pedido no existe.
/// - límite de pedidos → `RateLimited`; credenciales → `SessionRejected`;
///   problemas de conexión → `Network`.
/// - resto → `Spotify` con el texto original.
fn metadata_error(error: &CoreError, uri: &SpotifyUri) -> AppError {
    match error.kind {
        ErrorKind::NotFound | ErrorKind::FailedPrecondition => AppError::NotFound(describe(uri)),
        ErrorKind::ResourceExhausted => {
            AppError::RateLimited(config::DEFAULT_RETRY_AFTER.as_secs())
        }
        ErrorKind::Unauthenticated | ErrorKind::PermissionDenied => {
            AppError::SessionRejected(error.to_string())
        }
        ErrorKind::Unavailable
        | ErrorKind::DeadlineExceeded
        | ErrorKind::Aborted
        | ErrorKind::Cancelled => AppError::Network(error.to_string()),
        _ => AppError::Spotify(error.to_string()),
    }
}

/// `spotify:album:ID` → "el álbum spotify:album:ID", para mensajes.
fn describe(uri: &SpotifyUri) -> String {
    let kind = match uri {
        SpotifyUri::Track { .. } => "el tema",
        SpotifyUri::Album { .. } => "el álbum",
        SpotifyUri::Playlist { .. } => "la playlist",
        _ => "el elemento",
    };
    match uri.to_uri() {
        Ok(text) => format!("{kind} {text}"),
        Err(_) => kind.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errores_de_sesion_se_clasifican() {
        let rejected = CoreError::unauthenticated("bad credentials");
        assert!(matches!(
            session_error(&rejected),
            AppError::SessionRejected(_)
        ));
        let offline = CoreError::from(std::io::Error::from(std::io::ErrorKind::ConnectionRefused));
        assert!(matches!(session_error(&offline), AppError::Network(_)));
    }

    #[test]
    fn errores_de_metadata_se_clasifican() {
        let uri = SpotifyUri::from_uri("spotify:album:0000000000000000000000").unwrap();
        let cases = [
            (
                CoreError::failed_precondition("expected an entry"),
                "NotFound",
            ),
            (CoreError::not_found("x"), "NotFound"),
            (CoreError::deadline_exceeded("timeout"), "Network"),
            (CoreError::resource_exhausted("429"), "RateLimited"),
            (CoreError::internal("boom"), "Spotify"),
        ];
        for (error, expected) in cases {
            let got = metadata_error(&error, &uri);
            let name = match got {
                AppError::NotFound(_) => "NotFound",
                AppError::Network(_) => "Network",
                AppError::RateLimited(_) => "RateLimited",
                AppError::Spotify(_) => "Spotify",
                _ => "otro",
            };
            assert_eq!(name, expected, "{error}");
        }
    }

    #[test]
    fn describe_nombra_el_tipo() {
        let album = SpotifyUri::from_uri("spotify:album:4uLU6hMCjMI75M1A2tKUQC").unwrap();
        assert_eq!(
            describe(&album),
            "el álbum spotify:album:4uLU6hMCjMI75M1A2tKUQC"
        );
    }

    #[test]
    fn runtime_de_audio_corre_tareas_y_se_apaga() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let audio = AudioRuntime::start().unwrap();
        let name = rt
            .block_on(audio.run(async { thread::current().name().map(str::to_string) }))
            .unwrap();
        assert_eq!(name.as_deref(), Some("sesion-audio"));
        drop(audio); // no debe colgarse
    }
}
