//! Errores de la aplicación.
//!
//! Cada variante lleva un mensaje pensado para el usuario final: `main` lo
//! imprime tal cual, sin stack trace, y sale con código distinto de 0. Si
//! agregás una variante, el mensaje tiene que decir qué hacer, no solo qué
//! falló.
//!
//! Los errores de las librerías se traducen a estas variantes en el módulo
//! que los recibe (`auth`, `web`, `player`, `playback`), no en `main`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    /// Argumentos inválidos; el texto ya incluye la ayuda de uso.
    #[error("{0}")]
    Usage(String),

    /// No hay Client ID en ninguna fuente (ver `Config::load`).
    /// `saved_invalid`: el guardado existe pero no es válido. La CLI y la
    /// app de escritorio lo atajan y muestran la guía; este mensaje queda
    /// para donde no se puede preguntar.
    #[error(
        "{}no hay un Client ID de Spotify configurado.\n\
         Corré `spotify-terminal setup` (o `setup` en la app de escritorio) y \
         seguí los pasos.",
        if *saved_invalid { "el Client ID guardado no es válido: " } else { "" }
    )]
    MissingClientId { saved_invalid: bool },

    /// Spotify no reconoce el Client ID configurado (`invalid_client`).
    #[error(
        "Spotify no reconoce el Client ID configurado.\n\
         Revisá que sea el de tu app del Developer Dashboard y cargalo de nuevo \
         con `setup`."
    )]
    InvalidClientId,

    /// La Web API respondió 403 porque la cuenta no está habilitada en la
    /// app del Dashboard de este Client ID.
    #[error(
        "Spotify no habilita tu cuenta en la app del Developer Dashboard de este \
         Client ID.\n\
         Si la app es de otra persona, tiene que agregarte en \"User Management\" \
         (hasta 5 cuentas). Si no, creá tu propia app y cargá su Client ID con \
         `setup`."
    )]
    UserNotAllowed,

    #[error("el archivo .env tiene un formato inválido: {0}")]
    EnvFile(#[from] dotenvy::Error),

    #[error("no se pudo determinar la carpeta de configuración del usuario")]
    NoConfigDir,

    /// Lectura/escritura de archivos locales (caches de token).
    #[error("error leyendo/escribiendo datos locales: {0}")]
    Io(#[from] std::io::Error),

    /// No se pudo usar la terminal (modo raw, lectura de teclas).
    #[error(
        "no se pudo usar la terminal: {0}\n\
         Corré el comando desde una terminal interactiva (no redirigido)."
    )]
    Terminal(std::io::Error),

    /// El usuario rechazó la autorización en el navegador.
    #[error(
        "no se autorizó el acceso en el navegador.\n\
         Para usar el cliente hay que aceptarlo: corré `spotify-terminal login`."
    )]
    LoginCancelled,

    /// El puerto del callback de login (en el texto) está en uso.
    #[error(
        "no se pudo esperar el login en {0}: el puerto está ocupado.\n\
         ¿Quedó otra ventana de spotify-terminal esperando el login? Cerrala y \
         probá de nuevo."
    )]
    LoginPortBusy(String),

    /// Spotify rechazó el login por otro motivo (Client ID, Redirect URI...).
    #[error(
        "no se pudo iniciar sesión con Spotify: {0}\n\
         Revisá la conexión a internet y que tu app del Dashboard tenga el \
         Redirect URI {redirect}; después corré `spotify-terminal login`.",
        redirect = crate::config::REDIRECT_URI
    )]
    Login(String),

    /// No se llegó a Spotify (sin red, DNS, timeout, conexión perdida).
    #[error(
        "no se pudo conectar con Spotify: {0}\nRevisá la conexión a internet y probá de nuevo."
    )]
    Network(String),

    /// Spotify respondió que las credenciales guardadas no sirven.
    #[error(
        "Spotify rechazó la sesión guardada ({0}).\n\
         Corré `spotify-terminal logout` y después `spotify-terminal login`."
    )]
    SessionRejected(String),

    /// Spotify limita los pedidos; segundos a esperar.
    #[error(
        "Spotify está limitando los pedidos de esta app.\n\
         Esperá {0} s y probá de nuevo."
    )]
    RateLimited(u64),

    /// Spotify respondió con un error que no es de credenciales ni de límite.
    #[error("Spotify respondió con un error: {0}\nProbá de nuevo en un rato.")]
    Spotify(String),

    /// La cuenta no es Premium (plan en el texto).
    #[error(
        "tu cuenta no es Premium (plan: {0}).\n\
         Reproducir desde clientes externos requiere Spotify Premium."
    )]
    NotPremium(String),

    /// No hay dispositivo de salida de audio.
    #[error(
        "no hay una salida de audio disponible en esta PC.\n\
         Conectá parlantes o auriculares y probá de nuevo."
    )]
    NoAudioOutput,

    /// El hilo de reproducción de librespot terminó sin que se lo pidiéramos.
    #[error(
        "el reproductor de audio se cerró inesperadamente.\n\
         Revisá que los parlantes o auriculares sigan conectados y probá de nuevo."
    )]
    PlayerStopped,

    /// Lo pedido (descripción en el texto) no existe.
    #[error(
        "no se encontró {0} en Spotify.\n\
         Revisá el link o el ID."
    )]
    NotFound(String),

    /// Una búsqueda no devolvió nada (`kind` en plural: "temas", "playlists").
    #[error(
        "no encontré {kind} para «{query}».
Probá con otras palabras."
    )]
    NoResults { kind: &'static str, query: String },

    /// Álbum o playlist sin temas reproducibles.
    #[error("{0} no tiene temas para reproducir.")]
    NothingToPlay(String),

    /// El único tema pedido no se puede reproducir.
    #[error(
        "el tema {0} no está disponible para reproducir.\n\
         Puede estar bloqueado en tu país o retirado de Spotify; probá con otro."
    )]
    TrackUnavailable(String),

    /// Falla interna que no depende del usuario (p. ej. un hilo que no arrancó).
    #[error("error interno: {0}\nProbá de nuevo; si se repite, es un bug.")]
    Internal(String),
}
