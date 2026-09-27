use std::{io, process::ExitCode};

use librespot_core::SpotifyUri;
use spotify_terminal::{
    app::volume::Volume,
    config::{self, ClientIdSource, Config},
    error::AppError,
    setup::{self, Applied},
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

    // `help`, `version`, `setup` y `logout` no necesitan Client ID; el resto
    // lo pide con la guía si falta (spec 008).
    match cli::parse(&args)? {
        Command::Help => println!("{}", cli::USAGE),
        Command::Version => println!("spotify-terminal {}", config::VERSION),
        Command::Setup => setup_command()?,
        Command::Logout => {
            if auth::logout(&config::data_dir()?)? {
                println!("Sesión cerrada.");
            } else {
                println!("No había una sesión guardada.");
            }
        }
        Command::Login => {
            let config = config_or_setup()?;
            for kind in TokenKind::ALL {
                auth::get_valid_token(&config, kind).await?;
                println!("Acceso de {} listo.", kind.label());
            }
            println!("Sesión guardada en {}.", config.data_dir.display());
        }
        Command::Whoami => {
            let config = config_or_setup()?;
            let web = WebClient::new(auth::get_valid_token(&config, TokenKind::Web).await?)?;
            let user = web.current_user().await?;
            println!("{} ({}), plan: {}", user.name(), user.id, user.plan());
        }
        Command::Play { target, shuffle } => {
            let config = config_or_setup()?;
            let web = WebClient::new(auth::get_valid_token(&config, TokenKind::Web).await?)?;
            play(&config, &web, &target, shuffle).await?;
        }
        Command::Search {
            kind,
            query,
            shuffle,
        } => {
            let config = config_or_setup()?;
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

/// Configuración para un comando que usa la Web API. Si no hay Client ID,
/// muestra la guía, lo pide por la terminal, lo guarda y sigue (spec 008).
///
/// - Errores: los de `Config::load`; cancelar la guía → `MissingClientId`.
fn config_or_setup() -> Result<Config, AppError> {
    match Config::load() {
        Err(AppError::MissingClientId { saved_invalid }) => {
            if ask_client_id(saved_invalid)?.is_none() {
                return Err(AppError::MissingClientId { saved_invalid });
            }
            println!();
            Config::load()
        }
        other => other,
    }
}

/// `setup`: muestra el Client ID actual, la guía y pide uno nuevo.
fn setup_command() -> Result<(), AppError> {
    if let Ok(current) = Config::load() {
        let from = match current.client_id_source {
            ClientIdSource::Env => "variable de entorno o .env",
            ClientIdSource::SavedFile => "guardado",
        };
        println!("Client ID actual: {} ({from}).\n", current.web_client_id);
    }
    match ask_client_id(false)? {
        None => println!("Cancelado: el Client ID no cambió."),
        Some(applied) => {
            println!("Client ID guardado.");
            if let Some(notice) = applied.notice("spotify-terminal login") {
                println!("{notice}");
            }
        }
    }
    Ok(())
}

/// Guía + pedido interactivo del Client ID. `None` si el usuario canceló.
fn ask_client_id(saved_invalid: bool) -> Result<Option<Applied>, AppError> {
    if saved_invalid {
        println!(
            "⚠ El Client ID guardado ({}) no es válido: hay que cargarlo de nuevo.\n",
            config::CLIENT_ID_FILE
        );
    }
    println!("{}\n", config::setup_guide());
    let id =
        setup::prompt(&mut io::stdin().lock(), &mut io::stdout()).map_err(AppError::Terminal)?;
    id.map(|id| setup::apply(&id)).transpose()
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
