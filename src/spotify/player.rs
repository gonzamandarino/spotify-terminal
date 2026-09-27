//! Reproductor: sesión de librespot + salida de audio local.

use std::sync::Arc;

use librespot_core::{
    SpotifyUri,
    authentication::Credentials,
    config::SessionConfig,
    error::{Error as CoreError, ErrorKind},
    session::Session,
};
use librespot_metadata::{Album, Metadata, Playlist};
use librespot_playback::{
    audio_backend,
    config::{AudioFormat, PlayerConfig},
    mixer::NoOpVolume,
    player::{self, PlayerEventChannel},
};

use crate::{error::AppError, spotify::auth::Token};

/// Reproductor conectado a Spotify y a la salida de audio por defecto.
///
/// Invariante: la sesión está autenticada mientras exista el `Player`.
pub struct Player {
    /// Misma sesión que usa `inner`; se guarda para pedir metadata
    /// (temas de un álbum o playlist) sin un segundo login.
    session: Session,
    // `Arc` porque librespot devuelve el reproductor compartido entre su
    // hilo de audio y nosotros.
    inner: Arc<player::Player>,
}

impl Player {
    /// Abre la sesión de librespot con el token y prepara la salida de audio.
    ///
    /// - Pre: `token` vigente de tipo `TokenKind::Audio` (con otro
    ///   Client ID la sesión conecta pero no carga audio: spike T2).
    /// - Post: reproductor listo, sin nada cargado.
    /// - Errores: Spotify rechaza el token → `SessionRejected`; no se llega
    ///   al servidor → `Network`; sin placa de audio → `NoAudioOutput`.
    /// - No debe: empezar a reproducir.
    pub async fn connect(token: &Token) -> Result<Player, AppError> {
        let session = Session::new(SessionConfig::default(), None);
        session
            .connect(Credentials::with_access_token(&token.access_token), false)
            .await
            .map_err(session_error)?;

        let backend = audio_backend::find(None).ok_or(AppError::NoAudioOutput)?;
        let inner = player::Player::new(
            PlayerConfig::default(),
            session.clone(),
            Box::new(NoOpVolume),
            move || backend(None, AudioFormat::default()),
        );
        Ok(Player { session, inner })
    }

    /// Temas a reproducir, en orden, para lo que pidió el usuario.
    ///
    /// - Pre: `uri` es un tema, álbum o playlist.
    /// - Post: lista no vacía de URIs reproducibles (temas y episodios; los
    ///   archivos locales de una playlist se omiten). Un tema devuelve solo
    ///   a sí mismo sin consultar a Spotify.
    /// - Errores: no existe → `NotFound`; sin temas → `NothingToPlay`; otro
    ///   tipo de URI → `Usage`.
    pub async fn resolve_tracks(&self, uri: &SpotifyUri) -> Result<Vec<SpotifyUri>, AppError> {
        let not_found = |e| metadata_error(e, uri);
        let tracks: Vec<SpotifyUri> = match uri {
            SpotifyUri::Track { .. } => return Ok(vec![uri.clone()]),
            SpotifyUri::Album { .. } => Album::get(&self.session, uri)
                .await
                .map_err(not_found)?
                .tracks()
                .cloned()
                .collect(),
            SpotifyUri::Playlist { .. } => Playlist::get(&self.session, uri)
                .await
                .map_err(not_found)?
                .tracks()
                .filter(|t| matches!(t, SpotifyUri::Track { .. } | SpotifyUri::Episode { .. }))
                .cloned()
                .collect(),
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
        Ok(tracks)
    }

    /// Canal de eventos del reproductor (cambio de tema, play/pausa, fin...).
    /// Cada llamada crea un canal nuevo que recibe los eventos desde ese
    /// momento.
    pub fn events(&self) -> PlayerEventChannel {
        self.inner.get_player_event_channel()
    }

    /// Carga `track` y empieza a reproducirlo desde el principio.
    pub fn play(&self, track: SpotifyUri) {
        self.inner.load(track, true, 0);
    }

    /// Empieza a bajar `track` mientras suena el actual, para que el paso
    /// al siguiente tema (`play`) no tenga silencio. No cambia lo que suena.
    pub fn preload(&self, track: SpotifyUri) {
        self.inner.preload(track);
    }

    pub fn pause(&self) {
        self.inner.pause();
    }

    pub fn resume(&self) {
        self.inner.play();
    }

    pub fn stop(&self) {
        self.inner.stop();
    }
}

/// Errores de la sesión de librespot: credenciales rechazadas vs. no se
/// pudo hablar con Spotify.
fn session_error(error: CoreError) -> AppError {
    match error.kind {
        ErrorKind::Unauthenticated | ErrorKind::PermissionDenied => {
            AppError::SessionRejected(error.to_string())
        }
        _ => AppError::Network(error.to_string()),
    }
}

/// Errores al pedir metadata con la sesión ya abierta: si no es un problema
/// de conexión ni de credenciales, lo pedido no existe (Spotify contesta
/// `NotFound` o una respuesta sin datos, `FailedPrecondition`).
fn metadata_error(error: CoreError, uri: &SpotifyUri) -> AppError {
    match error.kind {
        ErrorKind::Unavailable
        | ErrorKind::DeadlineExceeded
        | ErrorKind::Aborted
        | ErrorKind::Cancelled => AppError::Network(error.to_string()),
        ErrorKind::Unauthenticated | ErrorKind::PermissionDenied => {
            AppError::SessionRejected(error.to_string())
        }
        _ => AppError::NotFound(describe(uri)),
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
            session_error(rejected),
            AppError::SessionRejected(_)
        ));
        let offline = CoreError::from(std::io::Error::from(std::io::ErrorKind::ConnectionRefused));
        assert!(matches!(session_error(offline), AppError::Network(_)));
    }

    #[test]
    fn errores_de_metadata_se_clasifican() {
        let uri = SpotifyUri::from_uri("spotify:album:0000000000000000000000").unwrap();
        let empty = CoreError::failed_precondition("expected an entry to exist in data");
        assert!(matches!(metadata_error(empty, &uri), AppError::NotFound(_)));
        let timeout = CoreError::deadline_exceeded("timeout");
        assert!(matches!(
            metadata_error(timeout, &uri),
            AppError::Network(_)
        ));
    }

    #[test]
    fn describe_nombra_el_tipo() {
        let album = SpotifyUri::from_uri("spotify:album:4uLU6hMCjMI75M1A2tKUQC").unwrap();
        assert_eq!(
            describe(&album),
            "el álbum spotify:album:4uLU6hMCjMI75M1A2tKUQC"
        );
    }
}
