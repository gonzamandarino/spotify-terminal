//! Configuración de la app: único lugar para constantes, rutas y umbrales.
//! Ninguna otra parte del código lee variables de entorno ni define estos
//! valores por su cuenta.

use std::{path::PathBuf, time::Duration};

use librespot_playback::config::Bitrate;

use crate::{
    app::engine::GlobalAction,
    desktop::{
        combo::{Combo, Key},
        settings::{Palette, WindowAction},
    },
    error::AppError,
    setup::{self, SavedClientId},
};

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

// --- Distribución (spec 008) ---

/// Versión de la app (la de `Cargo.toml`); el tag del Release tiene que
/// coincidir.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Archivo (en la carpeta de datos) con el Client ID que cargó `setup`.
pub const CLIENT_ID_FILE: &str = "client-id.txt";

/// Largo de un Client ID de Spotify (hexadecimal).
pub const CLIENT_ID_LEN: usize = 32;

/// Donde se crea la app propia.
pub const DASHBOARD_URL: &str = "https://developer.spotify.com/dashboard";

/// Endpoint de canje de tokens, usado para chequear el Client ID antes de
/// abrir el navegador (ver `spotify::auth`).
pub const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";

/// Código de error de `TOKEN_URL` para un Client ID que no existe (T1 de
/// spec 008: `{"error":"invalid_client","error_description":"Failed to get client"}`).
pub const INVALID_CLIENT_ERROR: &str = "invalid_client";

/// Texto del cuerpo de un 403 de la Web API cuando la cuenta no está en
/// *User Management* de la app del Client ID. Pendiente de confirmar en
/// T1c de spec 008 (es el que reporta la comunidad).
pub const USER_NOT_REGISTERED_MESSAGE: &str = "the user may not be registered";

/// Pasos para crear la app en el Developer Dashboard. Único lugar donde
/// están: si Spotify cambia el Dashboard, se corrige acá (y en
/// `dist/LEEME.txt`, que un test compara).
pub fn setup_guide() -> String {
    format!(
        "Para usar spotify-terminal necesitás un Client ID propio de Spotify \
         (gratis, se hace una sola vez):\n\
         \n\
         1. Entrá a {DASHBOARD_URL} con tu cuenta de Spotify (Premium) y \
         aceptá los términos si te los pide.\n\
         2. Tocá \"Create app\". Nombre y descripción: cualquiera (por ejemplo \
         \"spotify-terminal\").\n\
         3. En \"Redirect URIs\" pegá exactamente {REDIRECT_URI} y tocá \"Add\".\n\
         4. En \"Which API/SDKs are you planning to use?\" marcá \"Web API\".\n\
         5. Aceptá los términos y tocá \"Save\".\n\
         6. En la app recién creada, copiá el \"Client ID\" (32 letras y \
         números) y pegalo acá."
    )
}

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

/// `Ctrl+Alt+tecla`: los atajos globales usan esos modificadores.
/// `Ctrl+Shift` se descartó: tapa la selección de texto de las demás apps.
const fn ctrl_alt(key: Key) -> Combo {
    Combo::new(true, true, false, key)
}

/// Atajos globales de la app de escritorio: combinación → acción.
/// `Ctrl+Alt+Espacio` no se usa: ya la registra otra app en la PC de
/// desarrollo.
pub(crate) const GLOBAL_SHORTCUTS: &[(Combo, GlobalAction)] = &[
    (ctrl_alt(Key::Char('P')), GlobalAction::TogglePause),
    (ctrl_alt(Key::Right), GlobalAction::Next),
    (ctrl_alt(Key::Left), GlobalAction::Prev),
    (ctrl_alt(Key::Enter), GlobalAction::Stop),
    (ctrl_alt(Key::Up), GlobalAction::VolumeUp),
    (ctrl_alt(Key::Down), GlobalAction::VolumeDown),
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

// --- Ajustes (spec 007) ---
//
// Los valores de arriba (colores, fuente, atajos, volumen, consola) son los
// **defaults** de los ajustes que el usuario cambia desde la barra de menús
// de la app de escritorio. Acá van además los rangos y catálogos.

/// Archivo (en la carpeta de datos) con los ajustes que difieren del default.
pub const SETTINGS_FILE: &str = "ajustes.json";

/// Espera después del último cambio de un ajuste antes de guardarlo: un
/// cambio continuo (arrastrar el selector de color, Ctrl+rueda) no escribe
/// a disco en cada paso.
pub const SETTINGS_SAVE_DELAY: Duration = Duration::from_secs(1);

/// Tamaño de letra permitido y cuánto cambia con Agrandar / Achicar, en pt.
pub const FONT_SIZE_MIN: f32 = 8.0;
pub const FONT_SIZE_MAX: f32 = 32.0;
pub const FONT_SIZE_STEP: f32 = 1.0;

/// Paso de volumen permitido, en %.
pub const VOLUME_STEP_MIN: u8 = 1;
pub const VOLUME_STEP_MAX: u8 = 25;

/// Umbral de "anterior reinicia el tema" permitido, en segundos (0 =
/// siempre vuelve al tema anterior).
pub const PREVIOUS_THRESHOLD_MAX_SECS: u64 = 30;

/// Líneas de scrollback y comandos del historial permitidos.
pub const SCROLLBACK_MIN: usize = 100;
pub const SCROLLBACK_MAX: usize = 20_000;
pub const HISTORY_MIN: usize = 10;
pub const HISTORY_MAX: usize = 2_000;

/// Símbolo del prompt por defecto y su largo máximo, en caracteres.
pub const PROMPT_DEFAULT: &str = "♫ ›";
pub const PROMPT_MAX_CHARS: usize = 8;

/// Calidades de audio que se ofrecen, en kbps.
pub const BITRATES_KBPS: &[u16] = &[96, 160, 320];

/// Letra subrayada de cada menú de la barra, en orden: Tema, Fuente,
/// Atajos, Reproducción, Consola, Ventana, Ajustes. `Alt`+letra abre el
/// menú, así que `Alt`+esas letras no se aceptan como atajo de ventana.
pub(crate) const MENU_ACCESS_KEYS: [char; 7] = ['T', 'F', 'A', 'R', 'C', 'V', 'J'];

/// Atajo fijo que vuelve todo a fábrica: no se puede reasignar ni quitar,
/// por si un tema deja el menú ilegible.
pub(crate) const RESTORE_ALL_SHORTCUT: Combo = Combo::new(true, false, true, Key::F(12));

/// Combinaciones con Ctrl que la línea de entrada usa para editar
/// (seleccionar todo, copiar, pegar, cortar, deshacer, rehacer): no se
/// aceptan como atajo de ventana. Sin Ctrl ni Alt tampoco (salvo F1-F12),
/// porque taparían lo que se escribe.
pub(crate) const EDITING_SHORTCUTS: &[Combo] = &[
    Combo::new(true, false, false, Key::Char('A')),
    Combo::new(true, false, false, Key::Char('C')),
    Combo::new(true, false, false, Key::Char('V')),
    Combo::new(true, false, false, Key::Char('X')),
    Combo::new(true, false, false, Key::Char('Z')),
    Combo::new(true, false, false, Key::Char('Y')),
];

const fn ctrl(key: Key) -> Combo {
    Combo::new(true, false, false, key)
}

/// Atajos de la ventana por defecto (spec 004 + zoom de spec 007). Las
/// acciones que no están acá arrancan sin atajo.
pub(crate) const WINDOW_SHORTCUTS: &[(Combo, WindowAction)] = &[
    (ctrl(Key::Space), WindowAction::TogglePause),
    (ctrl(Key::Right), WindowAction::Next),
    (ctrl(Key::Left), WindowAction::Prev),
    (ctrl(Key::Up), WindowAction::VolumeUp),
    (ctrl(Key::Down), WindowAction::VolumeDown),
    (ctrl(Key::Plus), WindowAction::FontBigger),
    (ctrl(Key::Minus), WindowAction::FontSmaller),
    (ctrl(Key::Char('0')), WindowAction::FontReset),
];

/// Fuente monoespaciada que se puede elegir: archivos en `%WINDIR%\Fonts`.
/// Solo se ofrecen las que están instaladas.
pub(crate) struct FontEntry {
    pub(crate) name: &'static str,
    pub(crate) regular: &'static str,
    /// Sin negrita propia, la negrita usa la normal.
    pub(crate) bold: Option<&'static str>,
}

pub(crate) const FONT_CATALOG: &[FontEntry] = &[
    FontEntry {
        name: "Consolas",
        regular: theme::CONSOLE_FONT,
        bold: Some(theme::CONSOLE_FONT_BOLD),
    },
    FontEntry {
        name: "Cascadia Mono",
        regular: "CascadiaMono.ttf",
        bold: None,
    },
    FontEntry {
        name: "Courier New",
        regular: "cour.ttf",
        bold: Some("courbd.ttf"),
    },
    FontEntry {
        name: "Lucida Console",
        regular: "lucon.ttf",
        bold: None,
    },
];

/// La monoespaciada que trae egui (siempre está). También es la que queda
/// si la elegida no está instalada.
pub const BUILTIN_FONT: &str = "egui";

/// Fuente por defecto: la de la consola de Windows.
pub const DEFAULT_FONT: &str = "Consolas";

/// Temas predefinidos. El primero es el default.
pub(crate) const THEME_PRESETS: &[(&str, Palette)] = &[
    ("Spotify oscuro", SPOTIFY_DARK),
    (
        "Claro",
        Palette {
            background: [0xF5, 0xF5, 0xF5],
            panel: [0xFF, 0xFF, 0xFF],
            border: [0xD0, 0xD0, 0xD0],
            accent: [0x15, 0x88, 0x3E],
            text: [0x20, 0x20, 0x20],
            text_strong: [0x00, 0x00, 0x00],
            secondary: [0x6A, 0x6A, 0x6A],
            warning: [0x9A, 0x60, 0x00],
            error: [0xC6, 0x28, 0x28],
            close_hover: theme::CLOSE_HOVER,
        },
    ),
    (
        "Alto contraste",
        Palette {
            background: [0x00, 0x00, 0x00],
            panel: [0x00, 0x00, 0x00],
            border: [0xFF, 0xFF, 0xFF],
            accent: [0x00, 0xFF, 0x7F],
            text: [0xFF, 0xFF, 0xFF],
            text_strong: [0xFF, 0xFF, 0xFF],
            secondary: [0xC0, 0xC0, 0xC0],
            warning: [0xFF, 0xFF, 0x00],
            error: [0xFF, 0x55, 0x55],
            close_hover: theme::CLOSE_HOVER,
        },
    ),
];

/// Tema de spec 004 (los colores de `theme`).
pub(crate) const SPOTIFY_DARK: Palette = Palette {
    background: theme::BACKGROUND,
    panel: theme::PANEL,
    border: theme::BORDER,
    accent: theme::ACCENT,
    text: theme::TEXT,
    text_strong: theme::TEXT_STRONG,
    secondary: theme::SECONDARY,
    warning: theme::WARNING,
    error: theme::ERROR,
    close_hover: theme::CLOSE_HOVER,
};

const _: () = assert!(
    FONT_SIZE_MIN <= theme::FONT_SIZE
        && theme::FONT_SIZE <= FONT_SIZE_MAX
        && VOLUME_STEP_MIN <= VOLUME_STEP
        && VOLUME_STEP <= VOLUME_STEP_MAX
        && PREVIOUS_RESTART_THRESHOLD.as_secs() <= PREVIOUS_THRESHOLD_MAX_SECS
        && SCROLLBACK_MIN <= SCROLLBACK_LINES
        && SCROLLBACK_LINES <= SCROLLBACK_MAX
        && HISTORY_MIN <= HISTORY_LEN
        && HISTORY_LEN <= HISTORY_MAX
);

/// De dónde salió el Client ID en uso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientIdSource {
    /// `SPOTIFY_CLIENT_ID`, de la variable de entorno o de `.env`.
    Env,
    /// `CLIENT_ID_FILE`, guardado por `setup`.
    SavedFile,
}

/// Configuración resuelta para esta máquina.
///
/// Invariante: `web_client_id` nunca está vacío ni tiene espacios alrededor.
#[derive(Debug, Clone)]
pub struct Config {
    /// Client ID de tu app del Spotify Developer Dashboard (Web API).
    pub web_client_id: String,
    pub client_id_source: ClientIdSource,
    /// Carpeta de datos locales (caches de token). Está fuera del repo.
    pub data_dir: PathBuf,
}

impl Config {
    /// Resuelve el Client ID y las rutas locales. Prioridad: variable de
    /// entorno `SPOTIFY_CLIENT_ID`, después la misma clave en `.env` (de la
    /// carpeta actual), después `CLIENT_ID_FILE` en la carpeta de datos.
    ///
    /// - Post: `Config` que cumple la invariante, o
    ///   `MissingClientId { saved_invalid }` si no hay ninguno
    ///   (`saved_invalid`: el archivo existe pero no es válido).
    /// - No debe: crear carpetas ni archivos.
    pub fn load() -> Result<Config, AppError> {
        let env = env_client_id()?;
        let data_dir = data_dir()?;
        let (web_client_id, client_id_source) =
            resolve_client_id(env, setup::read_saved(&data_dir))?;
        Ok(Config {
            web_client_id,
            client_id_source,
            data_dir,
        })
    }
}

/// Valor crudo de `SPOTIFY_CLIENT_ID`, cargando antes `.env` si existe.
/// `.env` no pisa una variable de entorno ya definida, así que esta gana.
///
/// - Errores: `EnvFile` si `.env` existe y está mal formado.
pub fn env_client_id() -> Result<Option<String>, AppError> {
    if let Err(e) = dotenvy::dotenv() {
        if !e.not_found() {
            return Err(AppError::EnvFile(e));
        }
    }
    Ok(std::env::var(WEB_CLIENT_ID_VAR).ok())
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

/// Elige el Client ID entre `SPOTIFY_CLIENT_ID` (`env`) y el guardado.
/// Separada de `Config::load` para testearla sin tocar el entorno real.
///
/// - Post: `env` recortado si no es vacío (sin exigir formato, como antes
///   de spec 008); si no, el guardado si es válido; si no,
///   `MissingClientId`.
pub fn resolve_client_id(
    env: Option<String>,
    saved: SavedClientId,
) -> Result<(String, ClientIdSource), AppError> {
    if let Some(value) = env.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        return Ok((value.to_string(), ClientIdSource::Env));
    }
    match saved {
        SavedClientId::Valid(id) => Ok((id.as_str().to_string(), ClientIdSource::SavedFile)),
        SavedClientId::Missing => Err(AppError::MissingClientId {
            saved_invalid: false,
        }),
        SavedClientId::Invalid => Err(AppError::MissingClientId {
            saved_invalid: true,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::validate;

    const SAVED: &str = "0123456789abcdef0123456789abcdef";

    fn saved() -> SavedClientId {
        SavedClientId::Valid(validate(SAVED).unwrap())
    }

    fn missing(
        saved_invalid: bool,
    ) -> impl Fn(&Result<(String, ClientIdSource), AppError>) -> bool {
        move |r| matches!(r, Err(AppError::MissingClientId { saved_invalid: s }) if *s == saved_invalid)
    }

    #[test]
    fn env_gana_sobre_el_archivo_y_se_recorta() {
        for file in [saved(), SavedClientId::Missing, SavedClientId::Invalid] {
            let (id, source) = resolve_client_id(Some("  abc123  ".into()), file).unwrap();
            assert_eq!((id.as_str(), source), ("abc123", ClientIdSource::Env));
        }
    }

    #[test]
    fn sin_env_se_usa_el_archivo() {
        for env in [None, Some(""), Some("   "), Some("\t\n")] {
            let (id, source) = resolve_client_id(env.map(str::to_string), saved()).unwrap();
            assert_eq!((id.as_str(), source), (SAVED, ClientIdSource::SavedFile));
        }
    }

    #[test]
    fn sin_ninguno_es_missing_client_id() {
        assert!(missing(false)(&resolve_client_id(
            None,
            SavedClientId::Missing
        )));
        assert!(missing(true)(&resolve_client_id(
            Some(" ".into()),
            SavedClientId::Invalid
        )));
    }

    #[test]
    fn la_guia_tiene_link_y_redirect_uri() {
        let guide = setup_guide();
        assert!(guide.contains(DASHBOARD_URL) && guide.contains(REDIRECT_URI));
    }

    /// Si cambia una constante que el LEEME del zip repite, este test marca
    /// el LEEME desactualizado (spec 008, AC-13).
    #[test]
    fn leeme_coincide_con_la_config() {
        let leeme = include_str!("../dist/LEEME.txt");
        for text in [
            DASHBOARD_URL,
            REDIRECT_URI,
            &format!("%APPDATA%\\{APP_DIR_NAME}"),
            "setup",
            "Más información",
        ] {
            assert!(leeme.contains(text), "falta en dist/LEEME.txt: {text}");
        }
    }
}
