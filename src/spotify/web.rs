//! Llamadas a la Spotify Web API.

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
///   respuesta no exitosa → `WebApi` con el código HTTP.
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
        return Err(AppError::WebApi(format!("{status} al pedir el usuario")));
    }
    response
        .json::<User>()
        .await
        .map_err(|e| AppError::WebApi(format!("respuesta inesperada de /me: {e}")))
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
}
