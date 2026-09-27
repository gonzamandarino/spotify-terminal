//! Errores de la aplicación.
//!
//! Cada variante lleva un mensaje pensado para el usuario final: `main` lo
//! imprime tal cual, sin stack trace, y sale con código distinto de 0. Si
//! agregás una variante, el mensaje tiene que decir qué hacer, no solo qué
//! falló.

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
        "no se autorizó el acceso en el navegador.\n\
         Para usar el cliente hay que aceptarlo: corré `spotify-terminal login`."
    )]
    LoginCancelled,

    #[error(
        "no se pudo esperar el login en {0}: el puerto está ocupado.\n\
         ¿Quedó otra ventana de spotify-terminal esperando el login? Cerrala y \
         probá de nuevo."
    )]
    LoginPortBusy(String),

    #[error(
        "no se pudo iniciar sesión con Spotify: {0}\n\
         Revisá la conexión a internet y que tu app del Dashboard tenga el \
         Redirect URI {redirect}; después corré `spotify-terminal login`.",
        redirect = crate::config::REDIRECT_URI
    )]
    Login(String),

    #[error(
        "no se pudo conectar con Spotify: {0}\nRevisá la conexión a internet y probá de nuevo."
    )]
    Network(String),

    #[error(
        "Spotify rechazó la sesión guardada ({0}).\n\
         Corré `spotify-terminal logout` y después `spotify-terminal login`."
    )]
    SessionRejected(String),

    #[error(
        "Spotify está limitando los pedidos de esta app.\n\
         Esperá {0} s y probá de nuevo."
    )]
    RateLimited(u64),

    #[error("Spotify respondió con un error: {0}")]
    WebApi(String),

    #[error(
        "tu cuenta no es Premium (plan: {0}).\n\
         Reproducir desde clientes externos requiere Spotify Premium."
    )]
    NotPremium(String),

    #[error(
        "no hay una salida de audio disponible en esta PC.\n\
         Conectá parlantes o auriculares y probá de nuevo."
    )]
    NoAudioOutput,

    #[error(
        "no se encontró {0} en Spotify.\n\
         Revisá el link o el ID."
    )]
    NotFound(String),

    #[error("{0} no tiene temas para reproducir.")]
    NothingToPlay(String),

    #[error(
        "el tema {0} no está disponible para reproducir.\n\
         Puede estar bloqueado en tu país o retirado de Spotify; probá con otro."
    )]
    TrackUnavailable(String),
}
