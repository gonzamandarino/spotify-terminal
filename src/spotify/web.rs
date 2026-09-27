//! Llamadas a la Spotify Web API.

use reqwest::{StatusCode, header::RETRY_AFTER};
use serde::Deserialize;

use crate::{config, error::AppError, spotify::auth::Token};

/// Usuario logueado (`GET /me`).
#[derive(Debug, Deserialize)]
pub struct User {
    pub id: String,
    pub display_name: Option<String>,
    /// Plan de la cuenta (`premium`, `free`, ...). Requiere el scope
    /// `user-read-private`.
    pub product: Option<String>,
}

impl User {
    /// Nombre para mostrar; si el usuario no tiene uno, su id.
    pub fn name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.id)
    }

    pub fn is_premium(&self) -> bool {
        self.product.as_deref() == Some("premium")
    }

    /// Plan de la cuenta para mostrar (`desconocido` si Spotify no lo dice).
    pub fn plan(&self) -> &str {
        self.product.as_deref().unwrap_or("desconocido")
    }
}

/// Devuelve el usuario dueño del token.
///
/// - Pre: `token` vigente de tipo `TokenKind::Web`.
/// - Post: `Ok(User)` con los datos de `/me`; error de red → `Network`,
///   respuesta no exitosa → ver [`status_error`].
/// - No debe: reintentar en loop ni loguear el token.
pub async fn current_user(token: &Token) -> Result<User, AppError> {
    let response = reqwest::Client::new()
        .get(format!("{}/me", config::WEB_API_BASE))
        .bearer_auth(&token.access_token)
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());
        return Err(status_error(status, retry_after, "pedir el usuario"));
    }
    response
        .json::<User>()
        .await
        .map_err(|e| AppError::WebApi(format!("respuesta inesperada de /me: {e}")))
}

/// Traduce una respuesta no exitosa a un error con instrucciones.
///
/// - 401 → `SessionRejected` (token revocado o inválido: hay que reloguearse).
/// - 429 → `RateLimited` con los segundos de `Retry-After` (o
///   `config::DEFAULT_RETRY_AFTER` si no vino).
/// - 5xx → `WebApi` indicando que es un problema de Spotify.
/// - resto → `WebApi` con el código y la operación (`action`).
fn status_error(status: StatusCode, retry_after: Option<u64>, action: &str) -> AppError {
    match status {
        StatusCode::UNAUTHORIZED => AppError::SessionRejected(format!("{status} al {action}")),
        StatusCode::TOO_MANY_REQUESTS => {
            AppError::RateLimited(retry_after.unwrap_or(config::DEFAULT_RETRY_AFTER.as_secs()))
        }
        s if s.is_server_error() => AppError::WebApi(format!(
            "{status} al {action}. Es un problema de Spotify: probá más tarde."
        )),
        _ => AppError::WebApi(format!("{status} al {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_se_deserializa_y_detecta_premium() {
        let json = r#"{"id":"abc","display_name":"Gonza","product":"premium","country":"AR"}"#;
        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.name(), "Gonza");
        assert!(user.is_premium());
    }

    #[test]
    fn user_sin_nombre_ni_plan() {
        let json = r#"{"id":"abc","display_name":null}"#;
        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.name(), "abc");
        assert!(!user.is_premium());
        assert_eq!(user.plan(), "desconocido");
    }

    #[test]
    fn codigos_http_se_traducen() {
        let e = |status, retry| status_error(status, retry, "probar");
        assert!(matches!(
            e(StatusCode::UNAUTHORIZED, None),
            AppError::SessionRejected(_)
        ));
        assert!(matches!(
            e(StatusCode::TOO_MANY_REQUESTS, Some(42)),
            AppError::RateLimited(42)
        ));
        assert!(matches!(
            e(StatusCode::TOO_MANY_REQUESTS, None),
            AppError::RateLimited(s) if s == config::DEFAULT_RETRY_AFTER.as_secs()
        ));
        assert!(matches!(
            e(StatusCode::BAD_GATEWAY, None),
            AppError::WebApi(m) if m.contains("probá más tarde")
        ));
        assert!(matches!(
            e(StatusCode::FORBIDDEN, None),
            AppError::WebApi(_)
        ));
    }
}
