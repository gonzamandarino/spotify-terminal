//! Errores de la aplicación.
//!
//! Cada variante lleva un mensaje pensado para el usuario final: `main` lo
//! imprime tal cual, sin stack trace. Si agregás una variante, el mensaje
//! tiene que decir qué hacer, no solo qué falló.

use thiserror::Error;

/// Todo lo que puede salir mal en la app, con su mensaje para el usuario.
///
/// `#[derive(Error)]` (de `thiserror`) implementa el trait `std::error::Error`
/// y usa el texto de `#[error(...)]` como implementación de `Display`.
#[derive(Debug, Error)]
pub enum AppError {
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
}
