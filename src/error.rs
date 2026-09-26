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

    #[error("no se pudo determinar la carpeta de configuración del usuario")]
    NoConfigDir,

    #[error("error leyendo/escribiendo datos locales: {0}")]
    Io(#[from] std::io::Error),

    #[error(
        "no se pudo iniciar sesión con Spotify: {0}\n\
         Probá de nuevo con `spotify-terminal login`."
    )]
    Login(String),
}
