//! Login OAuth (PKCE) y caches de token.
//!
//! Hay dos tokens (ver `docs/decisiones.md`):
//! - **Audio** — Client ID de librespot, solo para la sesión de reproducción.
//! - **Web** — Client ID propio, para la Web API (con el de librespot la Web
//!   API responde 429 permanente).
//!
//! Único módulo que lee o escribe los caches de token. Viven fuera del repo y
//! se escriben de forma atómica: nunca queda un archivo a medio escribir.

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use librespot_oauth::{OAuthClient, OAuthClientBuilder, OAuthError, OAuthToken};
use serde::{Deserialize, Serialize};

use crate::{
    config::{self, Config},
    error::AppError,
};

/// Para qué se usa el token. Define Client ID, scopes y archivo de cache.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    Audio,
    Web,
}

impl TokenKind {
    pub const ALL: [TokenKind; 2] = [TokenKind::Audio, TokenKind::Web];

    fn client_id(self, config: &Config) -> &str {
        match self {
            TokenKind::Audio => config::AUDIO_CLIENT_ID,
            TokenKind::Web => &config.web_client_id,
        }
    }

    fn scopes(self) -> &'static [&'static str] {
        match self {
            TokenKind::Audio => config::AUDIO_SCOPES,
            TokenKind::Web => config::WEB_SCOPES,
        }
    }

    fn cache_path(self, config: &Config) -> PathBuf {
        let file = match self {
            TokenKind::Audio => "token-audio.json",
            TokenKind::Web => "token-web.json",
        };
        config.data_dir.join(file)
    }

    /// Nombre para mostrar al usuario.
    pub fn label(self) -> &'static str {
        match self {
            TokenKind::Audio => "audio",
            TokenKind::Web => "Web API",
        }
    }
}

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

/// Devuelve un token válido del tipo pedido, haciendo lo mínimo necesario:
/// usa el cache, lo renueva si está por vencer, o abre el login en el
/// navegador.
///
/// - Pre: ninguna (el cache puede no existir o estar corrupto).
/// - Post: el token no vence en los próximos `TOKEN_REFRESH_MARGIN`, tiene
///   todos los scopes de `kind`, y quedó guardado en su cache.
/// - Errores: sin red al renovar → `Network` (no abre el navegador: el
///   login tampoco andaría); en el login, cancelado → `LoginCancelled`,
///   puerto del callback ocupado → `LoginPortBusy`, sin red → `Network`.
///   Si Spotify rechaza el refresh token, se vuelve a loguear.
/// - No debe: imprimir ni loguear el access token ni el refresh token.
pub async fn get_valid_token(config: &Config, kind: TokenKind) -> Result<Token, AppError> {
    let cache_path = kind.cache_path(config);
    let client = oauth_client(config, kind)?;

    if let Some(cached) = read_cache(&cache_path) {
        if cached.has_scopes(kind.scopes()) {
            if !cached.needs_refresh(unix_now()) {
                return Ok(cached);
            }
            match client.refresh_token_async(&cached.refresh_token).await {
                Ok(fresh) => {
                    let token = Token::from_oauth(fresh, Some(&cached.refresh_token));
                    write_cache(&cache_path, &token)?;
                    return Ok(token);
                }
                // Solo si Spotify rechazó el refresh token tiene sentido
                // volver a loguearse; sin red, el login tampoco andaría.
                Err(e) if refresh_was_rejected(&e) => eprintln!(
                    "La sesión de {} ya no es válida. Hay que iniciar sesión.",
                    kind.label()
                ),
                Err(e) => return Err(AppError::Network(e.to_string())),
            }
        } else {
            println!("Hacen falta permisos nuevos ({}).", kind.label());
        }
    }

    println!(
        "Abriendo el navegador para autorizar el acceso de {}...
         (Si cerraste el navegador sin terminar, cortá con Ctrl+C.)",
        kind.label()
    );
    let fresh = client.get_access_token_async().await.map_err(login_error)?;
    let token = Token::from_oauth(fresh, None);
    write_cache(&cache_path, &token)?;
    Ok(token)
}

/// Borra los tokens guardados (audio y Web API).
///
/// - Post: no queda ningún cache. Devuelve `true` si había alguna sesión.
pub fn logout(config: &Config) -> Result<bool, AppError> {
    let mut removed = false;
    for kind in TokenKind::ALL {
        match fs::remove_file(kind.cache_path(config)) {
            Ok(()) => removed = true,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(removed)
}

/// Prefijo con el que `oauth2` muestra una respuesta de error del servidor
/// (p. ej. `invalid_grant`), a diferencia de un fallo de red. `librespot-oauth`
/// solo expone el error como texto, así que se distingue por acá.
const SERVER_REJECTED_PREFIX: &str = "Server returned error response";

/// `true` si Spotify respondió y rechazó el refresh token (revocado, de otro
/// Client ID...), `false` si el pedido no llegó (sin red, DNS, timeout).
fn refresh_was_rejected(error: &OAuthError) -> bool {
    matches!(error, OAuthError::ExchangeCode { e } if e.contains(SERVER_REJECTED_PREFIX))
}

/// Traduce un fallo del login en el navegador a un error con instrucciones.
fn login_error(error: OAuthError) -> AppError {
    match error {
        // Al cancelar, Spotify redirige con `?error=access_denied`, sin `code`.
        OAuthError::AuthCodeNotFound { .. } => AppError::LoginCancelled,
        OAuthError::AuthCodeListenerBind { addr, .. } => AppError::LoginPortBusy(addr.to_string()),
        OAuthError::ExchangeCode { ref e } if !e.contains(SERVER_REJECTED_PREFIX) => {
            AppError::Network(error.to_string())
        }
        other => AppError::Login(other.to_string()),
    }
}

fn oauth_client(config: &Config, kind: TokenKind) -> Result<OAuthClient, AppError> {
    OAuthClientBuilder::new(
        kind.client_id(config),
        config::REDIRECT_URI,
        kind.scopes().to_vec(),
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
    fn logout_borra_ambos_caches_y_es_idempotente() {
        let config = Config {
            web_client_id: "id".into(),
            data_dir: temp_dir("logout"),
        };
        for kind in TokenKind::ALL {
            write_cache(&kind.cache_path(&config), &token(1, &[])).unwrap();
        }
        assert!(logout(&config).unwrap());
        for kind in TokenKind::ALL {
            assert!(!kind.cache_path(&config).exists());
        }
        assert!(!logout(&config).unwrap());
    }

    fn exchange_error(text: &str) -> OAuthError {
        OAuthError::ExchangeCode { e: text.into() }
    }

    #[test]
    fn refresh_rechazado_vs_fallo_de_red() {
        assert!(refresh_was_rejected(&exchange_error(
            "Server returned error response: invalid_grant: Refresh token revoked"
        )));
        assert!(!refresh_was_rejected(&exchange_error("Request failed")));
        assert!(!refresh_was_rejected(&OAuthError::Recv));
    }

    #[test]
    fn errores_de_login_se_traducen() {
        let cancelled = OAuthError::AuthCodeNotFound {
            uri: "http://localhost/login?error=access_denied".into(),
        };
        assert!(matches!(login_error(cancelled), AppError::LoginCancelled));

        let busy = OAuthError::AuthCodeListenerBind {
            addr: "127.0.0.1:8898".parse().unwrap(),
            e: io::Error::from(io::ErrorKind::AddrInUse),
        };
        assert!(matches!(login_error(busy), AppError::LoginPortBusy(a) if a == "127.0.0.1:8898"));

        assert!(matches!(
            login_error(exchange_error("Request failed")),
            AppError::Network(_)
        ));
        assert!(matches!(
            login_error(exchange_error(
                "Server returned error response: invalid_client"
            )),
            AppError::Login(_)
        ));
    }

    #[test]
    fn cada_tipo_de_token_tiene_su_client_id_y_cache() {
        let config = Config {
            web_client_id: "propio".into(),
            data_dir: PathBuf::from("datos"),
        };
        assert_eq!(TokenKind::Audio.client_id(&config), config::AUDIO_CLIENT_ID);
        assert_eq!(TokenKind::Web.client_id(&config), "propio");
        assert_ne!(
            TokenKind::Audio.cache_path(&config),
            TokenKind::Web.cache_path(&config)
        );
    }
}
