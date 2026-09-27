//! Reproductor: sesión de librespot + salida de audio local.

use std::sync::Arc;

use librespot_core::{
    SpotifyUri, authentication::Credentials, config::SessionConfig, session::Session,
};
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
    /// - No debe: empezar a reproducir.
    pub async fn connect(token: &Token) -> Result<Player, AppError> {
        let session = Session::new(SessionConfig::default(), None);
        session
            .connect(Credentials::with_access_token(&token.access_token), false)
            .await
            .map_err(|e| AppError::Session(e.to_string()))?;

        let backend = audio_backend::find(None).ok_or(AppError::NoAudioOutput)?;
        let inner = player::Player::new(
            PlayerConfig::default(),
            session,
            Box::new(NoOpVolume),
            move || backend(None, AudioFormat::default()),
        );
        Ok(Player { inner })
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
