//! Configuración de la app: único lugar para constantes, rutas y umbrales.
//! Ninguna otra parte del código lee variables de entorno ni define estos
//! valores por su cuenta.

use std::{path::PathBuf, time::Duration};

use librespot_playback::config::Bitrate;

use crate::{app::engine::GlobalAction, desktop::hotkeys, error::AppError};

/// Client ID para la sesión de audio (el de librespot). Con uno propio,
/// `login5` rechaza el token al cargar audio (spike T2, spec 001).
pub const AUDIO_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";

/// Scopes del token de audio: solo streaming.
pub const AUDIO_SCOPES: &[&str] = &["streaming"];

/// Scopes del token de la Web API (con el Client ID propio). Si un spec
/// agrega uno, los tokens cacheados sin ese scope vuelven a pasar por login.
pub const WEB_SCOPES: &[&str] = &["user-read-private"];

/// Redirect URI del login, la misma para ambos Client IDs. El puerto es
/// donde escucha el callback local.
pub const REDIRECT_URI: &str = "http://127.0.0.1:8898/login";

/// Base de la Spotify Web API.
pub const WEB_API_BASE: &str = "https://api.spotify.com/v1";

/// Espera sugerida al usuario ante un 429 si Spotify no manda `Retry-After`.
pub const DEFAULT_RETRY_AFTER: Duration = Duration::from_secs(30);

/// Se renueva el access token si le queda menos que esto de vida, para que
/// no venza en medio de una operación.
pub const TOKEN_REFRESH_MARGIN: Duration = Duration::from_secs(60);

/// Página que ve el usuario en el navegador al terminar el login.
pub const LOGIN_DONE_HTML: &str =
    "<!doctype html><html><body><h1>Listo. Volvé a la terminal.</h1></body></html>";

/// Tiempo máximo de un pedido a la Web API. Sin esto, una red colgada deja
/// al cliente esperando para siempre.
pub const HTTP_TIMEOUT: Duration = Duration::from_secs(15);

/// Plan de cuenta que permite reproducir (campo `product` de `GET /me`).
pub const PREMIUM_PLAN: &str = "premium";

/// Largo de un ID de Spotify (base62).
pub const SPOTIFY_ID_LEN: usize = 22;

/// Calidad del audio. 160 kbps es el default de librespot: menos red y RAM
/// que 320, y sin diferencia audible con auriculares comunes.
pub const AUDIO_BITRATE: Bitrate = Bitrate::Bitrate160;

/// Temas seguidos que pueden fallar al cargar antes de cortar la lista:
/// más que eso casi seguro es la conexión, no los temas.
pub const MAX_CONSECUTIVE_UNAVAILABLE: u32 = 3;

/// Resultados que muestra una búsqueda (`play <texto>`, `play list <texto>`).
pub const SEARCH_LIMIT: usize = 5;

/// Máximo de resultados que acepta `GET /search` en modo desarrollo (desde
/// feb 2026; con más responde 400).
pub const SEARCH_API_MAX_LIMIT: usize = 10;

// Se chequea al compilar: la API no acepta más de `SEARCH_API_MAX_LIMIT` y
// `ui::select` elige con una sola tecla (1-9).
const _: () =
    assert!(SEARCH_LIMIT >= 1 && SEARCH_LIMIT <= 9 && SEARCH_LIMIT <= SEARCH_API_MAX_LIMIT);

/// "Anterior" con más que esto de tema sonando lo reinicia en vez de volver
/// al tema anterior (como la app oficial).
pub const PREVIOUS_RESTART_THRESHOLD: Duration = Duration::from_secs(3);

/// Archivos de cache de token dentro de `Config::data_dir`.
pub const AUDIO_TOKEN_FILE: &str = "token-audio.json";
pub const WEB_TOKEN_FILE: &str = "token-web.json";

/// Variable de entorno (o clave de `.env`) con el Client ID propio.
const WEB_CLIENT_ID_VAR: &str = "SPOTIFY_CLIENT_ID";

/// Carpeta de la app dentro de la de configuración del usuario
/// (`%APPDATA%` en Windows).
const APP_DIR_NAME: &str = "spotify-terminal";

// --- Volumen (spec 005) ---

/// Volumen con el que arranca la app si no hay uno guardado, en %.
pub const VOLUME_DEFAULT: u8 = 100;

/// Cuánto suben / bajan `vol +` / `vol -` y sus atajos, en %.
pub const VOLUME_STEP: u8 = 5;

/// Archivo (en la carpeta de datos) con el último volumen, en %.
pub const VOLUME_FILE: &str = "volumen.txt";

/// Espera después del último cambio de volumen antes de guardarlo: con una
/// tecla apretada, un cambio tras otro no escribe a disco cada vez.
pub const VOLUME_SAVE_DELAY: Duration = Duration::from_secs(2);

const _: () = assert!(VOLUME_DEFAULT <= 100 && VOLUME_STEP >= 1 && VOLUME_STEP <= 100);

// --- Atajos globales (spec 006) ---

/// Modificadores de todos los atajos globales. `Ctrl+Shift` se descartó:
/// tapa la selección de texto de las demás apps.
pub(crate) const GLOBAL_SHORTCUT_MODIFIERS: hotkeys::Modifiers = hotkeys::Modifiers {
    ctrl: true,
    alt: true,
    shift: false,
};

/// Atajos globales de la app de escritorio: tecla (con
/// `GLOBAL_SHORTCUT_MODIFIERS`) → acción. `Ctrl+Alt+Espacio` no se usa: ya
/// la registra otra app en la PC de desarrollo.
pub(crate) const GLOBAL_SHORTCUTS: &[(hotkeys::Key, GlobalAction)] = &[
    (hotkeys::Key::Char('P'), GlobalAction::TogglePause),
    (hotkeys::Key::Right, GlobalAction::Next),
    (hotkeys::Key::Left, GlobalAction::Prev),
    (hotkeys::Key::Enter, GlobalAction::Stop),
    (hotkeys::Key::Up, GlobalAction::VolumeUp),
    (hotkeys::Key::Down, GlobalAction::VolumeDown),
];

// --- App de escritorio (spec 004) ---

/// Título de la ventana (barra de tareas y barra de título propia).
pub const WINDOW_TITLE: &str = "spotify-terminal";

/// Tamaño inicial y mínimo de la ventana, en puntos lógicos.
pub const WINDOW_SIZE: [f32; 2] = [920.0, 580.0];
pub const WINDOW_MIN_SIZE: [f32; 2] = [520.0, 320.0];

/// Líneas que guarda la consola; las más viejas se descartan.
pub const SCROLLBACK_LINES: usize = 2_000;

/// Comandos que recuerda el historial (↑/↓) durante la sesión.
pub const HISTORY_LEN: usize = 200;

/// Cada cuánto se redibuja la barra de progreso mientras suena algo. En
/// pausa o sin nada sonando no se redibuja solo.
pub const PROGRESS_REPAINT: Duration = Duration::from_secs(1);

/// Archivo (en la carpeta de datos) donde se anota un panic de la app de
/// escritorio, que no tiene consola donde mostrarlo.
pub const PANIC_LOG_FILE: &str = "desktop-panic.log";

/// Colores de la app de escritorio (RGB), tema oscuro tipo Spotify.
pub mod theme {
    pub const BACKGROUND: [u8; 3] = [0x12, 0x12, 0x12];
    pub const PANEL: [u8; 3] = [0x18, 0x18, 0x18];
    pub const BORDER: [u8; 3] = [0x2A, 0x2A, 0x2A];
    pub const ACCENT: [u8; 3] = [0x1D, 0xB9, 0x54];
    pub const TEXT: [u8; 3] = [0xE0, 0xE0, 0xE0];
    pub const TEXT_STRONG: [u8; 3] = [0xFF, 0xFF, 0xFF];
    pub const SECONDARY: [u8; 3] = [0x8A, 0x8A, 0x8A];
    pub const WARNING: [u8; 3] = [0xF5, 0xC4, 0x51];
    pub const ERROR: [u8; 3] = [0xF1, 0x5E, 0x6C];
    /// Fondo del botón de cerrar al pasar el mouse.
    pub const CLOSE_HOVER: [u8; 3] = [0xC4, 0x2B, 0x1C];
    /// Fuente de la consola de Windows (Consolas), normal y negrita, en
    /// `%WINDIR%\Fonts`.
    pub const CONSOLE_FONT: &str = "consola.ttf";
    pub const CONSOLE_FONT_BOLD: &str = "consolab.ttf";
    /// Tamaño de letra de la consola, en puntos.
    pub const FONT_SIZE: f32 = 14.0;
}

/// Configuración resuelta para esta máquina.
///
/// Invariante: `web_client_id` nunca está vacío ni tiene espacios alrededor.
#[derive(Debug)]
pub struct Config {
    /// Client ID de tu app del Spotify Developer Dashboard (Web API).
    pub web_client_id: String,
    /// Carpeta de datos locales (caches de token). Está fuera del repo.
    pub data_dir: PathBuf,
}

impl Config {
    /// Carga `.env` (si existe) y resuelve rutas locales.
    ///
    /// - Post: `Config` que cumple la invariante, o `MissingClientId` si falta
    ///   o está vacío `SPOTIFY_CLIENT_ID`.
    /// - No debe: crear carpetas ni archivos.
    pub fn load() -> Result<Config, AppError> {
        // `.env` es opcional (la variable puede venir del entorno); un `.env`
        // mal formado sí es error.
        if let Err(e) = dotenvy::dotenv() {
            if !e.not_found() {
                return Err(AppError::EnvFile(e));
            }
        }
        let web_client_id = parse_client_id(std::env::var(WEB_CLIENT_ID_VAR).ok())?;
        let data_dir = data_dir()?;
        Ok(Config {
            web_client_id,
            data_dir,
        })
    }
}

/// Carpeta de datos locales (`%APPDATA%\spotify-terminal`), sin necesitar
/// el resto de la configuración.
///
/// - Post: la ruta; no la crea.
/// - Errores: `NoConfigDir` si el sistema no informa la carpeta del usuario.
pub fn data_dir() -> Result<PathBuf, AppError> {
    Ok(directories::BaseDirs::new()
        .ok_or(AppError::NoConfigDir)?
        .config_dir()
        .join(APP_DIR_NAME))
}

/// Valida el Client ID crudo. Separada de `load` para testearla sin tocar
/// variables de entorno reales.
fn parse_client_id(raw: Option<String>) -> Result<String, AppError> {
    match raw {
        Some(value) if !value.trim().is_empty() => Ok(value.trim().to_string()),
        _ => Err(AppError::MissingClientId),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_id_ausente_o_vacio_es_error() {
        for raw in [None, Some(""), Some("   "), Some("\t\n")] {
            assert!(matches!(
                parse_client_id(raw.map(str::to_string)),
                Err(AppError::MissingClientId)
            ));
        }
    }

    #[test]
    fn client_id_valido_se_recorta() {
        assert_eq!(
            parse_client_id(Some("  abc123  ".into())).unwrap(),
            "abc123"
        );
    }
}
