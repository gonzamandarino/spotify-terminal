//! Configuración de la app: único lugar para constantes, rutas y umbrales.
//! Ninguna otra parte del código define estos valores por su cuenta.

use std::{path::PathBuf, time::Duration};

use crate::error::AppError;

/// Client ID con el que se hace el login OAuth (el de librespot).
///
/// Con un Client ID propio, `login5` rechaza el token al cargar audio: ver
/// spike T2 del spec 001 y `docs/decisiones.md`.
pub const CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";

/// Redirect URI del login. Tiene que coincidir con la registrada para
/// `CLIENT_ID`; el puerto es donde escucha el callback local.
pub const REDIRECT_URI: &str = "http://127.0.0.1:8898/login";

/// Permisos pedidos a Spotify. Si un spec agrega uno, los usuarios con un
/// token cacheado sin ese scope vuelven a pasar por el login.
pub const SCOPES: &[&str] = &["streaming", "user-read-private"];

/// Se renueva el access token si le queda menos que esto de vida, para que
/// no venza en medio de una operación.
pub const TOKEN_REFRESH_MARGIN: Duration = Duration::from_secs(60);

/// Página que ve el usuario en el navegador al terminar el login.
pub const LOGIN_DONE_HTML: &str =
    "<!doctype html><html><body><h1>Listo. Volvé a la terminal.</h1></body></html>";

/// Carpeta de la app dentro de la de configuración del usuario
/// (`%APPDATA%` en Windows).
const APP_DIR_NAME: &str = "spotify-terminal";

const TOKEN_CACHE_FILE: &str = "token.json";

/// Configuración resuelta para esta máquina.
#[derive(Debug)]
pub struct Config {
    /// Carpeta de datos locales (cache de token). Está fuera del repo.
    pub data_dir: PathBuf,
}

impl Config {
    /// Resuelve las rutas locales de la app.
    ///
    /// - Post: `data_dir` es `<config del usuario>/spotify-terminal`.
    /// - No debe: crear carpetas ni archivos.
    pub fn load() -> Result<Config, AppError> {
        let data_dir = directories::BaseDirs::new()
            .ok_or(AppError::NoConfigDir)?
            .config_dir()
            .join(APP_DIR_NAME);
        Ok(Config { data_dir })
    }

    /// Ruta del archivo de cache de token. Solo `spotify::auth` lo lee o
    /// escribe.
    pub fn token_cache_path(&self) -> PathBuf {
        self.data_dir.join(TOKEN_CACHE_FILE)
    }
}
