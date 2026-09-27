//! Lo que el motor (`app::engine`) necesita de Spotify, detrás de dos
//! traits chicos para poder testearlo con un reproductor falso.

use librespot_core::SpotifyUri;
use librespot_playback::player::PlayerEventChannel;
use tokio::runtime::Handle;

use crate::{
    config::Config,
    error::AppError,
    spotify::{
        auth::{self, Token, TokenKind},
        player::{Player, Resolved},
        web::{Hit, SearchKind, User, WebClient},
    },
};

/// Control del reproductor, como lo usa el motor. Mismo contrato que los
/// métodos homónimos de [`Player`].
pub(crate) trait Playback {
    fn events(&self) -> PlayerEventChannel;
    fn play(&self, track: SpotifyUri);
    fn preload(&self, track: SpotifyUri);
    fn pause(&self);
    fn resume(&self);
    fn restart(&self);
    fn stop(&self);
    fn session_lost(&self) -> bool;
}

impl Playback for Player {
    fn events(&self) -> PlayerEventChannel {
        Player::events(self)
    }
    fn play(&self, track: SpotifyUri) {
        Player::play(self, track);
    }
    fn preload(&self, track: SpotifyUri) {
        Player::preload(self, track);
    }
    fn pause(&self) {
        Player::pause(self);
    }
    fn resume(&self) {
        Player::resume(self);
    }
    fn restart(&self) {
        Player::restart(self);
    }
    fn stop(&self) {
        Player::stop(self);
    }
    fn session_lost(&self) -> bool {
        Player::session_lost(self)
    }
}

/// Operaciones contra Spotify que el motor lanza como tareas de fondo.
///
/// Contrato común: cada llamada carga la configuración de nuevo (así un
/// `.env` corregido sirve sin reiniciar) y pide un token vigente; si hace
/// falta autorizar en el navegador, esa espera no bloquea el hilo del motor.
/// Los errores son los de `auth`, `web` y `player`, sin reintentos.
pub(crate) trait Backend {
    type Player: Playback;

    /// Pide los dos tokens (audio y Web API), autorizando si hace falta.
    async fn login(&self) -> Result<(), AppError>;

    /// Borra la sesión guardada. `true` si había alguna.
    fn logout(&self) -> Result<bool, AppError>;

    async fn current_user(&self) -> Result<User, AppError>;

    async fn search(&self, kind: SearchKind, query: &str) -> Result<Vec<Hit>, AppError>;

    /// Abre un reproductor nuevo. Antes chequea que la cuenta sea Premium
    /// (sin Premium → `NotPremium`, sin abrir la sesión de audio).
    async fn connect(&self) -> Result<Self::Player, AppError>;

    /// Temas de un tema, álbum o playlist (ver [`Player::resolve_tracks`]).
    async fn resolve(&self, player: &Self::Player, uri: &SpotifyUri) -> Result<Resolved, AppError>;
}

/// El `Backend` de verdad: tokens de `auth`, Web API y librespot.
pub(crate) struct SpotifyBackend;

impl SpotifyBackend {
    /// Token vigente de `kind`. `auth::get_valid_token` puede quedarse
    /// esperando el callback del navegador con una llamada bloqueante
    /// (librespot-oauth), así que corre en un hilo de bloqueo y no en el
    /// del motor: mientras tanto la cola sigue avanzando.
    async fn token(kind: TokenKind) -> Result<Token, AppError> {
        let config = Config::load()?;
        let handle = Handle::current();
        tokio::task::spawn_blocking(move || handle.block_on(auth::get_valid_token(&config, kind)))
            .await
            .map_err(|e| AppError::Internal(format!("tarea de login: {e}")))?
    }

    async fn web() -> Result<WebClient, AppError> {
        WebClient::new(Self::token(TokenKind::Web).await?)
    }
}

impl Backend for SpotifyBackend {
    type Player = Player;

    async fn login(&self) -> Result<(), AppError> {
        for kind in TokenKind::ALL {
            Self::token(kind).await?;
        }
        Ok(())
    }

    fn logout(&self) -> Result<bool, AppError> {
        auth::logout(&Config::load()?)
    }

    async fn current_user(&self) -> Result<User, AppError> {
        Self::web().await?.current_user().await
    }

    async fn search(&self, kind: SearchKind, query: &str) -> Result<Vec<Hit>, AppError> {
        Self::web().await?.search(kind, query).await
    }

    async fn connect(&self) -> Result<Player, AppError> {
        let user = self.current_user().await?;
        if !user.is_premium() {
            return Err(AppError::NotPremium(user.plan().to_string()));
        }
        Player::connect(&Self::token(TokenKind::Audio).await?).await
    }

    async fn resolve(&self, player: &Player, uri: &SpotifyUri) -> Result<Resolved, AppError> {
        player.resolve_tracks(uri).await
    }
}
