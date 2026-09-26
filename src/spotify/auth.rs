//! Login OAuth (PKCE) y cache del token.
//!
//! Único módulo que lee o escribe el cache de token (`Config::token_cache_path`).
//! El cache vive fuera del repo y se escribe de forma atómica: nunca queda un
//! archivo a medio escribir si el proceso se corta.

use std::{
    fs, io,
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use librespot_oauth::{OAuthClient, OAuthClientBuilder, OAuthToken};
use serde::{Deserialize, Serialize};

use crate::{
    config::{self, Config},
    error::AppError,
};

/// Token de Spotify tal como se guarda en el cache.
///
/// Invariante: `refresh_token` no está vacío (si Spotify no manda uno nuevo
/// al renovar, se conserva el anterior).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub access_token: String,
    refresh_token: String,
    /// Vencimiento del access token, en segundos desde UNIX_EPOCH.
    expires_at: u64,
    scopes: Vec<String>,
}

impl Token {
    /// Convierte el token de `librespot-oauth`. Si `previous_refresh` es
    /// `Some` y el token nuevo no trae refresh token, se conserva el anterior.
    fn from_oauth(token: OAuthToken, previous_refresh: Option<&str>) -> Token {
        let remaining = token.expires_at.saturating_duration_since(Instant::now());
        let refresh_token = match (token.refresh_token.is_empty(), previous_refresh) {
            (true, Some(previous)) => previous.to_string(),
            _ => token.refresh_token,
        };
        Token {
            access_token: token.access_token,
            refresh_token,
            expires_at: unix_now() + remaining.as_secs(),
            scopes: token.scopes,
        }
    }

    /// `true` si al token le queda menos de `TOKEN_REFRESH_MARGIN` de vida.
    fn needs_refresh(&self, now: u64) -> bool {
        now + config::TOKEN_REFRESH_MARGIN.as_secs() >= self.expires_at
    }

    /// `true` si el token tiene todos los scopes pedidos.
    fn has_scopes(&self, required: &[&str]) -> bool {
        required
            .iter()
            .all(|scope| self.scopes.iter().any(|s| s == scope))
    }
}

/// Devuelve un token válido, haciendo lo mínimo necesario: usa el cache, lo
/// renueva si está por vencer, o abre el login en el navegador.
///
/// - Pre: ninguna (el cache puede no existir o estar corrupto).
/// - Post: el token devuelto no vence en los próximos
///   `TOKEN_REFRESH_MARGIN`, tiene todos los `SCOPES`, y quedó guardado en el
///   cache.
/// - No debe: imprimir ni loguear el access token ni el refresh token.
pub async fn get_valid_token(config: &Config) -> Result<Token, AppError> {
    let cache_path = config.token_cache_path();
    let client = oauth_client()?;

    if let Some(cached) = read_cache(&cache_path) {
        if cached.has_scopes(config::SCOPES) {
            if !cached.needs_refresh(unix_now()) {
                return Ok(cached);
            }
            match client.refresh_token_async(&cached.refresh_token).await {
                Ok(fresh) => {
                    let token = Token::from_oauth(fresh, Some(&cached.refresh_token));
                    write_cache(&cache_path, &token)?;
                    return Ok(token);
                }
                Err(e) => eprintln!("No se pudo renovar la sesión ({e}). Hay que iniciar sesión."),
            }
        } else {
            println!("La app necesita permisos nuevos. Hay que iniciar sesión de nuevo.");
        }
    }

    println!("Abriendo el navegador para iniciar sesión en Spotify...");
    let fresh = client
        .get_access_token_async()
        .await
        .map_err(|e| AppError::Login(e.to_string()))?;
    let token = Token::from_oauth(fresh, None);
    write_cache(&cache_path, &token)?;
    Ok(token)
}

/// Borra el token guardado.
///
/// - Post: el cache no existe. Devuelve `true` si había una sesión guardada.
pub fn logout(config: &Config) -> Result<bool, AppError> {
    match fs::remove_file(config.token_cache_path()) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn oauth_client() -> Result<OAuthClient, AppError> {
    OAuthClientBuilder::new(
        config::CLIENT_ID,
        config::REDIRECT_URI,
        config::SCOPES.to_vec(),
    )
    .open_in_browser()
    .with_custom_message(config::LOGIN_DONE_HTML)
    .build()
    .map_err(|e| AppError::Login(e.to_string()))
}

/// Lee el cache. Un cache inexistente o ilegible se trata como "sin sesión"
/// (se vuelve a loguear), no como error.
fn read_cache(path: &Path) -> Option<Token> {
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

/// Escribe el cache de forma atómica: primero a un temporal y después
/// `rename`, que reemplaza el archivo anterior de una sola vez.
fn write_cache(path: &Path, token: &Token) -> Result<(), AppError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string(token).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn token(expires_at: u64, scopes: &[&str]) -> Token {
        Token {
            access_token: "acceso".into(),
            refresh_token: "refresco".into(),
            expires_at,
            scopes: scopes.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn oauth_token(refresh: &str) -> OAuthToken {
        OAuthToken {
            access_token: "nuevo".into(),
            refresh_token: refresh.into(),
            expires_at: Instant::now() + Duration::from_secs(3600),
            token_type: "Bearer".into(),
            scopes: vec!["streaming".into()],
        }
    }

    /// Carpeta temporal propia de cada test, para que corran en paralelo.
    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("spotify-terminal-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn needs_refresh_respeta_el_margen() {
        let margin = config::TOKEN_REFRESH_MARGIN.as_secs();
        let now = 1_000_000;
        assert!(!token(now + margin + 1, &[]).needs_refresh(now));
        assert!(token(now + margin, &[]).needs_refresh(now));
        assert!(token(now - 10, &[]).needs_refresh(now));
    }

    #[test]
    fn has_scopes_exige_todos() {
        let t = token(0, &["streaming", "user-read-private"]);
        assert!(t.has_scopes(&["streaming"]));
        assert!(t.has_scopes(&["streaming", "user-read-private"]));
        assert!(!t.has_scopes(&["streaming", "user-library-read"]));
    }

    #[test]
    fn from_oauth_conserva_refresh_token_si_no_viene_uno_nuevo() {
        let t = Token::from_oauth(oauth_token(""), Some("viejo"));
        assert_eq!(t.refresh_token, "viejo");
        let t = Token::from_oauth(oauth_token("nuevo"), Some("viejo"));
        assert_eq!(t.refresh_token, "nuevo");
    }

    #[test]
    fn from_oauth_calcula_vencimiento_futuro() {
        let t = Token::from_oauth(oauth_token("r"), None);
        let now = unix_now();
        assert!(t.expires_at > now + 3500 && t.expires_at <= now + 3600);
    }

    #[test]
    fn cache_ida_y_vuelta_sin_temporal_residual() {
        let path = temp_dir("roundtrip").join("token.json");
        let t = token(123, &["streaming"]);
        write_cache(&path, &t).unwrap();
        assert_eq!(read_cache(&path), Some(t));
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn cache_corrupto_o_ausente_es_sin_sesion() {
        let dir = temp_dir("corrupto");
        let path = dir.join("token.json");
        assert_eq!(read_cache(&path), None);
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "{ no es json").unwrap();
        assert_eq!(read_cache(&path), None);
    }

    #[test]
    fn logout_borra_el_cache_y_es_idempotente() {
        let config = Config {
            data_dir: temp_dir("logout"),
        };
        write_cache(&config.token_cache_path(), &token(1, &[])).unwrap();
        assert!(logout(&config).unwrap());
        assert!(!config.token_cache_path().exists());
        assert!(!logout(&config).unwrap());
    }
}
