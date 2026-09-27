use std::process::ExitCode;

use librespot_core::SpotifyUri;
use spotify_terminal::{
    app::volume::Volume,
    config::Config,
    error::AppError,
    spotify::{
        auth::{self, TokenKind},
        player::Player,
        web::WebClient,
    },
    ui::{
        cli::{self, Command},
        playback, select,
    },
};

// Un solo hilo para main: acá solo corren el login, la Web API y la UI. La
// sesión de audio tiene su propio hilo (`spotify::player`) y librespot los
// suyos, así que un bloqueo acá no corta la música.
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    playback::restore_terminal_on_panic();
    // Todo error llega hasta acá como `AppError` y se muestra con su mensaje
    // para el usuario; nada de stack traces ni `panic!`.
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), AppError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = Config::load()?;

    match cli::parse(&args)? {
        Command::Help => println!("{}", cli::USAGE),
        Command::Login => {
            for kind in TokenKind::ALL {
                auth::get_valid_token(&config, kind).await?;
                println!("Acceso de {} listo.", kind.label());
            }
            println!("Sesión guardada en {}.", config.data_dir.display());
        }
        Command::Logout => {
            if auth::logout(&config)? {
                println!("Sesión cerrada.");
            } else {
                println!("No había una sesión guardada.");
            }
        }
        Command::Whoami => {
            let web = WebClient::new(auth::get_valid_token(&config, TokenKind::Web).await?)?;
            let user = web.current_user().await?;
            println!("{} ({}), plan: {}", user.name(), user.id, user.plan());
        }
        Command::Play { target, shuffle } => {
            let web = WebClient::new(auth::get_valid_token(&config, TokenKind::Web).await?)?;
            play(&config, &web, &target, shuffle).await?;
        }
        Command::Search {
            kind,
            query,
            shuffle,
        } => {
            let web = WebClient::new(auth::get_valid_token(&config, TokenKind::Web).await?)?;
            let hits = web.search(kind, &query).await?;
            if hits.is_empty() {
                return Err(AppError::NoResults {
                    kind: kind.plural(),
                    query,
                });
            }
            // Premium y la sesión de audio recién después de elegir:
            // cancelar no cuesta nada.
            if let Some(i) = select::choose(&hits).await? {
                play(&config, &web, &hits[i].uri, shuffle).await?;
            }
        }
    }
    Ok(())
}

/// Reproduce un tema, álbum o playlist con los controles de `playback`
/// (`shuffle` = arrancar mezclado). `web` se usa también para encolar.
async fn play(
    config: &Config,
    web: &WebClient,
    target: &SpotifyUri,
    shuffle: bool,
) -> Result<(), AppError> {
    // Chequeo barato antes de abrir la sesión de audio: sin Premium
    // librespot no reproduce y el error sería menos claro.
    let user = web.current_user().await?;
    if !user.is_premium() {
        return Err(AppError::NotPremium(user.plan().to_string()));
    }
    let audio_token = auth::get_valid_token(config, TokenKind::Audio).await?;
    let mut volume = Volume::load(&config.data_dir);
    let initial = volume;
    let player = Player::connect(
        &audio_token,
        volume,
        spotify_terminal::config::AUDIO_BITRATE,
    )
    .await?;
    let resolved = player.resolve_tracks(target).await?;
    if resolved.skipped > 0 {
        println!(
            "⚠ Se omiten {} elementos (archivos locales o que Spotify no devolvió).",
            resolved.skipped
        );
    }
    let result = playback::play_queue(&player, web, &resolved.tracks, shuffle, &mut volume).await;
    // Se guarda aunque la reproducción haya terminado con error: el volumen
    // elegido sigue valiendo. Si no se puede guardar, solo se avisa.
    if volume != initial {
        if let Err(e) = volume.save(&config.data_dir) {
            eprintln!("⚠ No se pudo guardar el volumen: {e}");
        }
    }
    result
}
