//! Configuración de la app: único lugar para constantes y valores que
//! vienen del entorno. Ninguna otra parte del código lee variables de
//! entorno ni define rutas o umbrales por su cuenta.

use std::path::PathBuf;

use crate::error::AppError;

/// Variable de entorno (o clave de `.env`) con el Client ID de Spotify.
const CLIENT_ID_VAR: &str = "SPOTIFY_CLIENT_ID";

/// Nombre de la carpeta de la app dentro de la carpeta de configuración del
/// usuario (`%APPDATA%` en Windows).
const APP_DIR_NAME: &str = "spotify-terminal";

/// Configuración ya validada.
///
/// Invariante: `client_id` nunca está vacío ni tiene espacios alrededor.
#[derive(Debug)]
pub struct Config {
    /// Client ID de la app de Spotify (no es secreto en el flujo PKCE).
    pub client_id: String,
    /// Carpeta donde se guardan datos locales (ej. el cache de token).
    /// Está fuera del repo.
    pub data_dir: PathBuf,
}

impl Config {
    /// Carga la configuración desde `.env` (si existe) y las variables de
    /// entorno.
    ///
    /// - Pre: ninguna. Que no exista `.env` es válido (las variables pueden
    ///   venir del entorno).
    /// - Post: devuelve un `Config` que cumple la invariante del struct, o
    ///   `AppError::MissingClientId` si falta o está vacío el Client ID.
    /// - No debe: crear carpetas ni archivos, ni leer el cache de token.
    pub fn load() -> Result<Config, AppError> {
        // `.env` es opcional: si no está, seguimos con el entorno tal cual.
        // Cualquier otro error (archivo mal formado) sí se reporta.
        if let Err(e) = dotenvy::dotenv() {
            if !e.not_found() {
                return Err(AppError::EnvFile(e));
            }
        }

        let client_id = parse_client_id(std::env::var(CLIENT_ID_VAR).ok())?;

        let data_dir = directories::BaseDirs::new()
            .ok_or(AppError::NoConfigDir)?
            .config_dir()
            .join(APP_DIR_NAME);

        Ok(Config {
            client_id,
            data_dir,
        })
    }

    /// Client ID enmascarado para mostrar en pantalla (`****abcd`).
    ///
    /// - Post: nunca devuelve más de los últimos 4 caracteres del Client ID.
    pub fn masked_client_id(&self) -> String {
        mask(&self.client_id)
    }
}

/// Valida el Client ID crudo. Separada de `load` para poder testearla sin
/// tocar variables de entorno reales.
fn parse_client_id(raw: Option<String>) -> Result<String, AppError> {
    match raw {
        Some(value) if !value.trim().is_empty() => Ok(value.trim().to_string()),
        _ => Err(AppError::MissingClientId),
    }
}

fn mask(value: &str) -> String {
    // Contamos `chars()` y no bytes: un `&str` es UTF-8 y cortarlo a mitad
    // de un carácter multibyte haría panic.
    let skip = value.chars().count().saturating_sub(4);
    let visible: String = value.chars().skip(skip).collect();
    format!("****{visible}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_id_ausente_es_error() {
        assert!(matches!(
            parse_client_id(None),
            Err(AppError::MissingClientId)
        ));
    }

    #[test]
    fn client_id_vacio_o_con_espacios_es_error() {
        for raw in ["", "   ", "\t\n"] {
            assert!(matches!(
                parse_client_id(Some(raw.to_string())),
                Err(AppError::MissingClientId)
            ));
        }
    }

    #[test]
    fn client_id_valido_se_recorta() {
        let id = parse_client_id(Some("  abc123  ".to_string())).unwrap();
        assert_eq!(id, "abc123");
    }

    #[test]
    fn mask_muestra_solo_los_ultimos_4() {
        assert_eq!(mask("0123456789abcdef"), "****cdef");
        assert_eq!(mask("ab"), "****ab");
    }
}
