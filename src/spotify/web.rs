//! Llamadas a la Spotify Web API.

use librespot_core::SpotifyUri;
use reqwest::{StatusCode, header::RETRY_AFTER};
use serde::{Deserialize, de::DeserializeOwned};

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

    /// `true` solo si Spotify informa el plan Premium (`config::PREMIUM_PLAN`).
    /// Un plan desconocido cuenta como no Premium.
    pub fn is_premium(&self) -> bool {
        self.product.as_deref() == Some(config::PREMIUM_PLAN)
    }

    /// Plan de la cuenta para mostrar (`desconocido` si Spotify no lo dice).
    pub fn plan(&self) -> &str {
        self.product.as_deref().unwrap_or("desconocido")
    }
}

/// Cliente de la Web API: un solo cliente HTTP (conexión y TLS
/// reutilizables) con timeout, atado a un token.
pub struct WebClient {
    http: reqwest::Client,
    token: Token,
}

impl WebClient {
    /// - Pre: `token` vigente de tipo `TokenKind::Web`.
    /// - Post: cliente listo; ningún pedido tarda más de `config::HTTP_TIMEOUT`.
    /// - No debe: hacer pedidos.
    pub fn new(token: Token) -> Result<WebClient, AppError> {
        let http = reqwest::Client::builder()
            .timeout(config::HTTP_TIMEOUT)
            .build()
            .map_err(|e| AppError::Internal(format!("cliente HTTP: {e}")))?;
        Ok(WebClient { http, token })
    }

    /// Devuelve el usuario dueño del token.
    ///
    /// - Post: `Ok(User)` con los datos de `/me`; sin red o timeout →
    ///   `Network`; respuesta no exitosa → ver [`status_error`].
    /// - No debe: reintentar en loop ni loguear el token.
    pub async fn current_user(&self) -> Result<User, AppError> {
        self.get_json("/me", &[], "pedir el usuario").await
    }

    /// Busca temas o playlists por texto (`GET /search`), en un solo pedido.
    ///
    /// - Pre: `query` no vacío.
    /// - Post: hasta `config::SEARCH_LIMIT` resultados en el orden de
    ///   Spotify (mejor coincidencia primero), solo reproducibles desde la
    ///   cuenta (`market=from_token`). Los elementos que Spotify devuelve
    ///   como `null` (p. ej. playlists editoriales, que las apps en modo
    ///   desarrollo no ven) se descartan; por eso las playlists se piden con
    ///   `config::SEARCH_API_MAX_LIMIT`. Sin resultados → `Ok(vec![])`.
    ///   Errores como en [`WebClient::current_user`].
    /// - No debe: reintentar, paginar ni guardar el texto buscado.
    pub async fn search(&self, kind: SearchKind, query: &str) -> Result<Vec<Hit>, AppError> {
        let limit = match kind {
            SearchKind::Track => config::SEARCH_LIMIT,
            SearchKind::Playlist => config::SEARCH_API_MAX_LIMIT,
        }
        .to_string();
        let params = [
            ("q", query),
            ("type", kind.api_type()),
            ("limit", &limit),
            ("market", "from_token"),
        ];
        let response: SearchResponse = self.get_json("/search", &params, "buscar").await?;
        Ok(response.hits())
    }

    /// `GET` a `path` de la Web API y deserializa la respuesta. `action`
    /// describe la operación en los mensajes de error.
    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
        action: &str,
    ) -> Result<T, AppError> {
        let response = self
            .http
            .get(format!("{}{path}", config::WEB_API_BASE))
            .query(query)
            .bearer_auth(self.token.access_token())
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
            return Err(status_error(status, retry_after, action));
        }
        response
            .json::<T>()
            .await
            .map_err(|e| AppError::Spotify(format!("respuesta inesperada de {path}: {e}")))
    }
}

/// Qué se busca.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchKind {
    Track,
    Playlist,
}

impl SearchKind {
    /// Valor del parámetro `type` de `/search`.
    fn api_type(self) -> &'static str {
        match self {
            SearchKind::Track => "track",
            SearchKind::Playlist => "playlist",
        }
    }

    /// Nombre en plural para mensajes ("no encontré temas...").
    pub fn plural(self) -> &'static str {
        match self {
            SearchKind::Track => "temas",
            SearchKind::Playlist => "playlists",
        }
    }
}

/// Un resultado de búsqueda, listo para mostrar y reproducir.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub uri: SpotifyUri,
    /// Nombre del tema o de la playlist.
    pub name: String,
    /// Tema: "artistas · álbum · m:ss". Playlist: "de dueño · N temas".
    pub detail: String,
}

#[derive(Deserialize)]
struct SearchResponse {
    tracks: Option<Page<TrackItem>>,
    playlists: Option<Page<PlaylistItem>>,
}

#[derive(Deserialize)]
struct Page<T> {
    /// La API puede mandar `null` en lugar de un elemento.
    items: Vec<Option<T>>,
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

#[derive(Deserialize)]
struct TrackItem {
    uri: String,
    name: String,
    artists: Vec<Named>,
    album: Named,
    duration_ms: u64,
}

#[derive(Deserialize)]
struct PlaylistItem {
    uri: String,
    name: String,
    owner: Owner,
    /// Desde feb 2026 la cantidad de temas viene en `items`; antes, en `tracks`.
    #[serde(alias = "tracks")]
    items: Option<Total>,
}

#[derive(Deserialize)]
struct Owner {
    id: String,
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct Total {
    total: u64,
}

impl SearchResponse {
    /// Resultados válidos, en orden, cortados en `config::SEARCH_LIMIT`.
    /// Un elemento con URI inválida se descarta como si fuera `null`.
    fn hits(self) -> Vec<Hit> {
        let tracks = self.tracks.into_iter().flat_map(|p| p.items).flatten();
        let playlists = self.playlists.into_iter().flat_map(|p| p.items).flatten();
        tracks
            .map(|t| {
                let artists: Vec<_> = t.artists.into_iter().map(|a| a.name).collect();
                let detail = format!(
                    "{} · {} · {}",
                    artists.join(", "),
                    t.album.name,
                    format_duration(t.duration_ms)
                );
                (t.uri, t.name, detail)
            })
            .chain(playlists.map(|p| {
                let owner = p.owner.display_name.unwrap_or(p.owner.id);
                let detail = match p.items {
                    Some(Total { total }) => format!("de {owner} · {total} temas"),
                    None => format!("de {owner}"),
                };
                (p.uri, p.name, detail)
            }))
            .filter_map(|(uri, name, detail)| {
                let uri = SpotifyUri::from_uri(&uri).ok()?;
                Some(Hit {
                    uri,
                    name: name.trim().to_string(),
                    detail,
                })
            })
            .take(config::SEARCH_LIMIT)
            .collect()
    }
}

/// `m:ss` (los minutos no se cortan en horas: 75:02).
fn format_duration(ms: u64) -> String {
    let secs = ms / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Traduce una respuesta no exitosa a un error con instrucciones.
///
/// - 401 → `SessionRejected` (token revocado o inválido: hay que reloguearse).
/// - 429 → `RateLimited` con los segundos de `Retry-After` (o
///   `config::DEFAULT_RETRY_AFTER` si no vino).
/// - 5xx → `Spotify` indicando que es un problema de Spotify.
/// - resto → `Spotify` con el código y la operación (`action`).
fn status_error(status: StatusCode, retry_after: Option<u64>, action: &str) -> AppError {
    match status {
        StatusCode::UNAUTHORIZED => AppError::SessionRejected(format!("{status} al {action}")),
        StatusCode::TOO_MANY_REQUESTS => {
            AppError::RateLimited(retry_after.unwrap_or(config::DEFAULT_RETRY_AFTER.as_secs()))
        }
        s if s.is_server_error() => AppError::Spotify(format!(
            "{status} al {action}. Es un problema de Spotify: probá más tarde."
        )),
        _ => AppError::Spotify(format!("{status} al {action}")),
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
            AppError::Spotify(m) if m.contains("probá más tarde")
        ));
        assert!(matches!(
            e(StatusCode::FORBIDDEN, None),
            AppError::Spotify(_)
        ));
    }

    const TRACKS_JSON: &str = r#"{"tracks":{"items":[
        {"uri":"spotify:track:4PTG3Z6ehGkBFwjybzWkR8","name":"Never Gonna Give You Up",
         "artists":[{"name":"Rick Astley"}],"album":{"name":"Whenever You Need Somebody"},
         "duration_ms":213573,"is_playable":true},
        null,
        {"uri":"spotify:track:3pepZAOvUCBt3qWi9Ax6Aq","name":"Never, Never Gonna Give Ya Up",
         "artists":[{"name":"Barry White"},{"name":"Otro"}],"album":{"name":"Hits"},
         "duration_ms":290666}
    ],"total":2}}"#;

    #[test]
    fn busqueda_de_temas() {
        let response: SearchResponse = serde_json::from_str(TRACKS_JSON).unwrap();
        let hits = response.hits();
        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits[0],
            Hit {
                uri: SpotifyUri::from_uri("spotify:track:4PTG3Z6ehGkBFwjybzWkR8").unwrap(),
                name: "Never Gonna Give You Up".into(),
                detail: "Rick Astley · Whenever You Need Somebody · 3:33".into(),
            }
        );
        assert_eq!(hits[1].detail, "Barry White, Otro · Hits · 4:50");
    }

    #[test]
    fn busqueda_de_playlists_descarta_null_y_corta_en_el_limite() {
        let item = |i: usize| {
            format!(
                r#"{{"uri":"spotify:playlist:{i:0>22}","name":" P{i} ",
                    "owner":{{"id":"u{i}","display_name":null}},"items":{{"total":{i}}}}}"#
            )
        };
        let mut items = vec!["null".to_string(), "null".to_string()];
        items.extend((0..config::SEARCH_API_MAX_LIMIT).map(item));
        let json = format!(r#"{{"playlists":{{"items":[{}]}}}}"#, items.join(","));
        let hits = serde_json::from_str::<SearchResponse>(&json)
            .unwrap()
            .hits();
        assert_eq!(hits.len(), config::SEARCH_LIMIT);
        assert_eq!(hits[0].name, "P0");
        assert_eq!(hits[1].detail, "de u1 · 1 temas");
    }

    #[test]
    fn playlist_con_formato_viejo_o_sin_total() {
        let json = r#"{"playlists":{"items":[
            {"uri":"spotify:playlist:627lheesMF3W2repUfJb5M","name":"A",
             "owner":{"id":"x","display_name":"Alberto"},"tracks":{"total":50}},
            {"uri":"spotify:playlist:1yBGduvI0IvHYwc1JNpdJC","name":"B",
             "owner":{"id":"y"}},
            {"uri":"no-es-uri","name":"C","owner":{"id":"z"}}
        ]}}"#;
        let hits = serde_json::from_str::<SearchResponse>(json).unwrap().hits();
        let details: Vec<_> = hits.iter().map(|h| h.detail.as_str()).collect();
        assert_eq!(details, ["de Alberto · 50 temas", "de y"]);
    }

    #[test]
    fn busqueda_sin_resultados() {
        let json = r#"{"tracks":{"items":[],"total":0}}"#;
        assert!(
            serde_json::from_str::<SearchResponse>(json)
                .unwrap()
                .hits()
                .is_empty()
        );
    }

    #[test]
    fn duracion() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(59_999), "0:59");
        assert_eq!(format_duration(4_502_000), "75:02");
    }
}
