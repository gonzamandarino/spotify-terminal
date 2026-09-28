//! Lo que el motor (`app::engine`) necesita de Spotify, detrás de dos
//! traits chicos para poder testearlo con un reproductor falso.

use librespot_core::SpotifyUri;
use librespot_playback::{config::Bitrate, player::PlayerEventChannel};
use tokio::{runtime::Handle, sync::watch};

use super::volume::Volume;
use crate::{
    config::{self, Config},
    error::AppError,
    setup::{self, Applied, ClientId},
    spotify::{
        auth::{self, Token, TokenKind},
        cover::{self, Cover},
        player::{Player, Resolved},
        tap::AudioTap,
        web::{Hit, LikedTracks, MyPlaylists, SearchKind, User, WebClient},
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
    fn set_volume(&self, volume: Volume);
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
    fn set_volume(&self, volume: Volume) {
        Player::set_volume(self, volume);
    }
    fn session_lost(&self) -> bool {
        Player::session_lost(self)
    }
}

/// Si hay un Client ID configurado (ver `Config::load`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ClientIdStatus {
    Ready,
    /// No hay en ninguna fuente; `saved_invalid`: el guardado no es válido.
    Missing {
        saved_invalid: bool,
    },
}

/// Operaciones contra Spotify que el motor lanza como tareas de fondo.
///
/// Contrato común: cada llamada carga la configuración de nuevo (así un
/// `.env` corregido o un Client ID recién cargado con `setup` sirven sin
/// reiniciar) y pide un token vigente; si hace falta autorizar en el
/// navegador, esa espera no bloquea el hilo del motor. Los errores son los
/// de `auth`, `web` y `player`, sin reintentos.
pub(crate) trait Backend {
    type Player: Playback;

    /// Si hay Client ID. Otro error de configuración cuenta como `Ready`:
    /// se muestra cuando se use.
    fn client_id_status(&self) -> ClientIdStatus;

    /// Guarda `id` y lo deja en uso (ver [`setup::apply`]).
    fn set_client_id(&self, id: &ClientId) -> Result<Applied, AppError>;

    /// Pide los dos tokens (audio y Web API), autorizando si hace falta.
    async fn login(&self) -> Result<(), AppError>;

    /// Borra la sesión guardada (no necesita Client ID). `true` si había
    /// alguna.
    fn logout(&self) -> Result<bool, AppError>;

    async fn current_user(&self) -> Result<User, AppError>;

    async fn search(&self, kind: SearchKind, query: &str) -> Result<Vec<Hit>, AppError>;

    /// Mis playlists, hasta `config::MY_PLAYLISTS_MAX` (ver
    /// [`WebClient::my_playlists`], spec 011).
    async fn my_playlists(&self) -> Result<MyPlaylists, AppError>;

    /// Temas de Tus me gusta, hasta `config::LIKES_MAX` (ver
    /// [`WebClient::liked_tracks`], spec 013). Tras cada página manda
    /// `(recibidos, total)` por `progress`; si nadie escucha, sigue igual.
    async fn liked_tracks(
        &self,
        progress: watch::Sender<(usize, usize)>,
    ) -> Result<LikedTracks, AppError>;

    /// Si `uri` está en Tus me gusta (ver [`WebClient::is_saved`]).
    async fn is_saved(&self, uri: &SpotifyUri) -> Result<bool, AppError>;

    /// Agrega (`save`) o quita `uri` de Tus me gusta (ver
    /// [`WebClient::set_saved`]).
    async fn set_saved(&self, uri: &SpotifyUri, save: bool) -> Result<(), AppError>;

    /// Abre un reproductor nuevo con `volume`. Antes chequea que la cuenta
    /// sea Premium (sin Premium → `NotPremium`, sin abrir la sesión de
    /// audio).
    async fn connect(&self, volume: Volume, bitrate: Bitrate) -> Result<Self::Player, AppError>;

    /// Temas de un tema, álbum o playlist (ver [`Player::resolve_tracks`]).
    async fn resolve(&self, player: &Self::Player, uri: &SpotifyUri) -> Result<Resolved, AppError>;

    /// Tapa de un disco, decodificada (ver [`cover::fetch`], spec 010).
    async fn cover(&self, url: &str) -> Result<Cover, AppError>;
}

/// El `Backend` de verdad: tokens de `auth`, Web API y librespot.
pub(crate) struct SpotifyBackend {
    /// Adónde copiar lo que suena para dibujarlo (app de escritorio con
    /// visualización, spec 010); `None` = no se copia.
    pub(crate) tap: Option<AudioTap>,
}

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

    fn client_id_status(&self) -> ClientIdStatus {
        match Config::load() {
            Err(AppError::MissingClientId { saved_invalid }) => {
                ClientIdStatus::Missing { saved_invalid }
            }
            _ => ClientIdStatus::Ready,
        }
    }

    fn set_client_id(&self, id: &ClientId) -> Result<Applied, AppError> {
        setup::apply(id)
    }

    async fn login(&self) -> Result<(), AppError> {
        for kind in TokenKind::ALL {
            Self::token(kind).await?;
        }
        Ok(())
    }

    fn logout(&self) -> Result<bool, AppError> {
        auth::logout(&config::data_dir()?)
    }

    async fn current_user(&self) -> Result<User, AppError> {
        Self::web().await?.current_user().await
    }

    async fn search(&self, kind: SearchKind, query: &str) -> Result<Vec<Hit>, AppError> {
        Self::web().await?.search(kind, query).await
    }

    async fn my_playlists(&self) -> Result<MyPlaylists, AppError> {
        Self::web()
            .await?
            .my_playlists(config::MY_PLAYLISTS_MAX)
            .await
    }

    async fn liked_tracks(
        &self,
        progress: watch::Sender<(usize, usize)>,
    ) -> Result<LikedTracks, AppError> {
        Self::web()
            .await?
            .liked_tracks(config::LIKES_MAX, |loaded, total| {
                // Sin receptor (tarea cancelada) no hay a quién avisar.
                let _ = progress.send((loaded, total));
            })
            .await
    }

    async fn is_saved(&self, uri: &SpotifyUri) -> Result<bool, AppError> {
        Self::web().await?.is_saved(uri).await
    }

    async fn set_saved(&self, uri: &SpotifyUri, save: bool) -> Result<(), AppError> {
        Self::web().await?.set_saved(uri, save).await
    }

    async fn connect(&self, volume: Volume, bitrate: Bitrate) -> Result<Player, AppError> {
        let user = self.current_user().await?;
        if !user.is_premium() {
            return Err(AppError::NotPremium(user.plan().to_string()));
        }
        Player::connect(
            &Self::token(TokenKind::Audio).await?,
            volume,
            bitrate,
            self.tap.clone(),
        )
        .await
    }

    async fn resolve(&self, player: &Player, uri: &SpotifyUri) -> Result<Resolved, AppError> {
        player.resolve_tracks(uri).await
    }

    async fn cover(&self, url: &str) -> Result<Cover, AppError> {
        cover::fetch(url).await
    }
}
