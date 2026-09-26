//! Spike T2 del spec 001 — descartable, se borra cuando exista `player.rs`.
//!
//! Pregunta que responde: ¿la sesión de streaming de librespot acepta un
//! access token emitido para NUESTRO Client ID (no el de Spotify)?
//!
//! Uso: cargo run --example spike_librespot -- <track_id | spotify:track:ID>

use std::{error::Error, fs, time::Duration};

use librespot_core::{
    SpotifyUri, authentication::Credentials, config::SessionConfig, session::Session,
    spotify_id::SpotifyId,
};
use librespot_oauth::OAuthClientBuilder;
use librespot_playback::{
    audio_backend,
    config::{AudioFormat, PlayerConfig},
    mixer::NoOpVolume,
    player::Player,
};

const REDIRECT_URI: &str = "http://127.0.0.1:8898/login";
/// Client ID que usan los ejemplos oficiales de librespot (plan B del spec).
const LIBRESPOT_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";
/// Solo para el spike: reusar el access token (dura 1 h) entre intentos.
const TOKEN_MAX_AGE: Duration = Duration::from_secs(50 * 60);

fn cached_token() -> Option<String> {
    let path = spike_token_path()?;
    let age = fs::metadata(&path).ok()?.modified().ok()?.elapsed().ok()?;
    if age > TOKEN_MAX_AGE {
        return None;
    }
    fs::read_to_string(path).ok()
}

fn spike_token_path() -> Option<std::path::PathBuf> {
    let dir = directories::BaseDirs::new()?
        .config_dir()
        .join("spotify-terminal");
    fs::create_dir_all(&dir).ok()?;
    let name = if use_librespot_id() {
        "spike_token_librespot.txt"
    } else {
        "spike_token.txt"
    };
    Some(dir.join(name))
}

/// `SPIKE_CLIENT=librespot` → usa el Client ID de librespot en vez del nuestro.
fn use_librespot_id() -> bool {
    std::env::var("SPIKE_CLIENT").is_ok_and(|v| v == "librespot")
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::new()
        .parse_filters(&std::env::var("RUST_LOG").unwrap_or("librespot=debug".into()))
        .init();
    let client_id = if use_librespot_id() {
        LIBRESPOT_CLIENT_ID.to_string()
    } else {
        std::env::var("SPOTIFY_CLIENT_ID")?
    };
    let arg = std::env::args().nth(1).ok_or("falta el track id")?;
    let track_id = arg.trim_start_matches("spotify:track:");

    let access_token = match cached_token() {
        Some(t) => {
            println!("1/3 Usando token cacheado del spike");
            t
        }
        None => {
            println!("1/3 Login OAuth ({client_id}) (se abre el navegador)...");
            let oauth = OAuthClientBuilder::new(
                &client_id,
                REDIRECT_URI,
                vec!["streaming", "user-read-private"],
            )
            .open_in_browser()
            .build()?;
            let token = oauth.get_access_token_async().await?;
            println!("    OK, scopes: {:?}", token.scopes);
            if let Some(path) = spike_token_path() {
                fs::write(path, &token.access_token)?;
            }
            token.access_token
        }
    };

    println!("2/3 Conectando sesión librespot con ese token...");
    let session = Session::new(SessionConfig::default(), None);
    session
        .connect(Credentials::with_access_token(&access_token), false)
        .await?;
    println!("    OK, usuario: {}", session.username());

    println!("3/3 Reproduciendo {track_id}...");
    let backend = audio_backend::find(None).ok_or("sin backend de audio")?;
    let player = Player::new(
        PlayerConfig::default(),
        session,
        Box::new(NoOpVolume),
        move || backend(None, AudioFormat::default()),
    );
    let mut events = player.get_player_event_channel();
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            println!("    evento: {event:?}");
            if let librespot_playback::player::PlayerEvent::Unavailable { .. } = event {
                eprintln!("Tema no disponible, fin del spike.");
                std::process::exit(2);
            }
        }
    });
    player.load(
        SpotifyUri::Track {
            id: SpotifyId::from_base62(track_id)?,
        },
        true,
        0,
    );
    player.await_end_of_track().await;
    println!("Fin del tema.");
    Ok(())
}
