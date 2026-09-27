//! Errores de la aplicación.
//!
//! Cada variante lleva un mensaje pensado para el usuario final: `main` lo
//! imprime tal cual, sin stack trace. Si agregás una variante, el mensaje
//! tiene que decir qué hacer, no solo qué falló.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Usage(String),

    #[error(
        "falta SPOTIFY_CLIENT_ID.\n\
         Copiá .env.example a .env y poné el Client ID de tu app del \
         Spotify Developer Dashboard (https://developer.spotify.com/dashboard)."
    )]
    MissingClientId,

    #[error("el archivo .env tiene un formato inválido: {0}")]
    EnvFile(#[from] dotenvy::Error),

    #[error("no se pudo determinar la carpeta de configuración del usuario")]
    NoConfigDir,

    #[error("error leyendo/escribiendo datos locales: {0}")]
    Io(#[from] std::io::Error),

    #[error(
        "no se pudo iniciar sesión con Spotify: {0}\n\
         Probá de nuevo con `spotify-terminal login`."
    )]
    Login(String),

    #[error("no se pudo conectar con Spotify: {0}\nRevisá la conexión a internet.")]
    Network(String),

    #[error("Spotify respondió con un error: {0}")]
    WebApi(String),

    #[error(
        "tu cuenta no es Premium (plan: {0}).\n\
         Reproducir desde clientes externos requiere Spotify Premium."
    )]
    NotPremium(String),

    #[error("no se pudo abrir la sesión de reproducción: {0}")]
    Session(String),

    #[error("no hay una salida de audio disponible en esta PC")]
    NoAudioOutput,

    #[error("el tema no está disponible para reproducir ({0})")]
    TrackUnavailable(String),
}
