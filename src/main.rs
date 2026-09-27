mod config;
mod error;
mod spotify;
mod ui;

use std::process::ExitCode;

use config::Config;
use error::AppError;
use spotify::{
    auth::{self, TokenKind},
    player::Player,
    web::WebClient,
};
use ui::{
    cli::{self, Command},
    playback,
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
        Command::Play(target) => {
            let web = WebClient::new(auth::get_valid_token(&config, TokenKind::Web).await?)?;
            // Chequeo barato antes de abrir la sesión de audio: sin Premium
            // librespot no reproduce y el error sería menos claro.
            let user = web.current_user().await?;
            if !user.is_premium() {
                return Err(AppError::NotPremium(user.plan().to_string()));
            }
            let audio_token = auth::get_valid_token(&config, TokenKind::Audio).await?;
            let player = Player::connect(&audio_token).await?;
            let resolved = player.resolve_tracks(&target).await?;
            if resolved.skipped > 0 {
                println!(
                    "⚠ Se omiten {} elementos (archivos locales o que Spotify no devolvió).",
                    resolved.skipped
                );
            }
            playback::play_queue(&player, &resolved.tracks).await?;
        }
    }
    Ok(())
}
