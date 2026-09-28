//! Llamadas a la Spotify Web API.

use librespot_core::SpotifyUri;
use reqwest::{
    Method, Response, StatusCode,
    header::{CONTENT_LENGTH, RETRY_AFTER},
};
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

    /// Mis playlists (`GET /me/playlists`): las creadas y las seguidas, en
    /// el orden de Spotify (spec 011).
    ///
    /// - Pre: `max` ≥ 1.
    /// - Post: pide páginas de `config::MY_PLAYLISTS_PAGE` hasta que no haya
    ///   más o se junten `max`; `hits` tiene a lo sumo `max`, sin los
    ///   elementos `null` ni los de URI inválida. `total` es el que informa
    ///   Spotify (puede ser mayor que `hits.len()`). Sin playlists → `hits`
    ///   vacío. Errores como en [`WebClient::current_user`]; un error en
    ///   cualquier página corta todo.
    /// - No debe: pedir más páginas que las que hacen falta para `max`, ni
    ///   reintentar.
    pub async fn my_playlists(&self, max: usize) -> Result<MyPlaylists, AppError> {
        let limit = config::MY_PLAYLISTS_PAGE.to_string();
        let mut hits = Vec::new();
        let mut offset = 0;
        loop {
            let offset_text = offset.to_string();
            let page: PlaylistPage = self
                .get_json(
                    "/me/playlists",
                    &[("limit", &limit), ("offset", &offset_text)],
                    "pedir tus playlists",
                )
                .await?;
            let received = page.items.len();
            let total = page.total;
            hits.extend(page.items.into_iter().flatten().filter_map(playlist_hit));
            offset += received;
            if page.next.is_none() || received == 0 || hits.len() >= max {
                hits.truncate(max);
                return Ok(MyPlaylists { hits, total });
            }
        }
    }

    /// Temas de Tus me gusta (`GET /me/tracks`), del más reciente al más
    /// viejo, como la app oficial (spec 013).
    ///
    /// - Pre: `max` ≥ 1.
    /// - Post: pide páginas de `config::LIKES_PAGE` hasta que no haya más o
    ///   se junten `max` temas; `tracks` tiene a lo sumo `max`. Los
    ///   elementos `null`, los archivos locales y los de URI inválida no
    ///   entran y se cuentan en `omitted`. `total` es el que informa
    ///   Spotify. Después de cada página llama a `progress(recibidos,
    ///   total)`. Errores como en [`WebClient::current_user`]; un error en
    ///   cualquier página corta todo (nunca devuelve una lista a medias).
    /// - No debe: pedir más páginas que las que hacen falta para `max`, ni
    ///   reintentar.
    pub async fn liked_tracks(
        &self,
        max: usize,
        mut progress: impl FnMut(usize, usize),
    ) -> Result<LikedTracks, AppError> {
        let limit = config::LIKES_PAGE.to_string();
        let mut liked = LikedTracks::default();
        let mut offset = 0;
        loop {
            let offset_text = offset.to_string();
            let page: SavedTrackPage = self
                .get_json(
                    "/me/tracks",
                    &[("limit", &limit), ("offset", &offset_text)],
                    "pedir tus me gusta",
                )
                .await?;
            let received = page.items.len();
            offset += received;
            liked.total = page.total;
            liked.add(page.items);
            progress(offset, page.total);
            if page.next.is_none() || received == 0 || liked.tracks.len() >= max {
                liked.tracks.truncate(max);
                return Ok(liked);
            }
        }
    }

    /// Si `uri` está en la biblioteca (`GET /me/library/contains`; para un
    /// tema, "Tus me gusta").
    ///
    /// - Post: `Ok(true)` / `Ok(false)` según Spotify; una respuesta sin
    ///   valor → `Spotify`. Errores como en [`WebClient::current_user`].
    /// - No debe: modificar la biblioteca.
    pub async fn is_saved(&self, uri: &SpotifyUri) -> Result<bool, AppError> {
        let uri = uri_param(uri)?;
        let saved: Vec<bool> = self
            .get_json(
                "/me/library/contains",
                &[("uris", &uri)],
                "consultar Tus me gusta",
            )
            .await?;
        saved
            .first()
            .copied()
            .ok_or_else(|| AppError::Spotify("respuesta vacía de /me/library/contains".into()))
    }

    /// Guarda (`save`) o quita `uri` de la biblioteca (`PUT` / `DELETE
    /// /me/library`). Guardar algo ya guardado, o quitar algo que no está,
    /// no es error.
    ///
    /// - Post: `Ok(())` si Spotify lo aceptó. Errores como en
    ///   [`WebClient::current_user`].
    /// - No debe: tocar otra URI que `uri`.
    pub async fn set_saved(&self, uri: &SpotifyUri, save: bool) -> Result<(), AppError> {
        let uri = uri_param(uri)?;
        let (method, action) = if save {
            (Method::PUT, "guardar en Tus me gusta")
        } else {
            (Method::DELETE, "quitar de Tus me gusta")
        };
        self.send(method, "/me/library", &[("uris", &uri)], action)
            .await
            .map(drop)
    }

    /// `GET` a `path` de la Web API y deserializa la respuesta. `action`
    /// describe la operación en los mensajes de error.
    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
        action: &str,
    ) -> Result<T, AppError> {
        self.send(Method::GET, path, query, action)
            .await?
            .json::<T>()
            .await
            .map_err(|e| AppError::Spotify(format!("respuesta inesperada de {path}: {e}")))
    }

    /// Pedido `method` a `path`; una respuesta no exitosa se traduce con
    /// [`status_error`].
    async fn send(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        action: &str,
    ) -> Result<Response, AppError> {
        let response = request(&self.http, method, path, query)
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
            // El cuerpo solo importa para distinguir un 403; si no se
            // puede leer, se sigue con el código.
            let body = response.text().await.unwrap_or_default();
            return Err(status_error(status, retry_after, &body, action));
        }
        Ok(response)
    }
}

/// Arma un pedido a la Web API. Uno que no es `GET` va con cuerpo vacío y
/// `Content-Length: 0`: sin eso, `PUT /me/library` responde 411.
fn request(
    http: &reqwest::Client,
    method: Method,
    path: &str,
    query: &[(&str, &str)],
) -> reqwest::RequestBuilder {
    let with_body = method != Method::GET;
    let builder = http
        .request(method, format!("{}{path}", config::WEB_API_BASE))
        .query(query);
    if with_body {
        builder.header(CONTENT_LENGTH, "0").body(Vec::new())
    } else {
        builder
    }
}

/// `spotify:track:…` para el parámetro `uris`.
fn uri_param(uri: &SpotifyUri) -> Result<String, AppError> {
    uri.to_uri()
        .map_err(|e| AppError::Internal(format!("URI sin texto: {e}")))
}

/// Resultado de [`WebClient::my_playlists`].
#[derive(Debug, Clone, PartialEq)]
pub struct MyPlaylists {
    pub hits: Vec<Hit>,
    /// Cuántas informa Spotify en total.
    pub total: usize,
}

/// Resultado de [`WebClient::liked_tracks`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LikedTracks {
    /// Temas reproducibles, del más reciente al más viejo.
    pub tracks: Vec<SpotifyUri>,
    /// Elementos recibidos que no entran (archivos locales, `null`, URI
    /// inválida).
    pub omitted: usize,
    /// Cuántos informa Spotify en total.
    pub total: usize,
}

impl LikedTracks {
    /// Suma los elementos de una página de `/me/tracks`.
    fn add(&mut self, items: Vec<Option<SavedTrack>>) {
        for item in items {
            let uri = item
                .and_then(|i| i.track)
                .filter(|t| !t.is_local)
                .and_then(|t| SpotifyUri::from_uri(&t.uri).ok())
                .filter(|u| matches!(u, SpotifyUri::Track { .. }));
            match uri {
                Some(uri) => self.tracks.push(uri),
                None => self.omitted += 1,
            }
        }
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

/// Una página de `GET /me/playlists`.
#[derive(Deserialize)]
struct PlaylistPage {
    items: Vec<Option<PlaylistItem>>,
    next: Option<String>,
    #[serde(default)]
    total: usize,
}

/// Una página de `GET /me/tracks`.
#[derive(Deserialize)]
struct SavedTrackPage {
    items: Vec<Option<SavedTrack>>,
    next: Option<String>,
    #[serde(default)]
    total: usize,
}

#[derive(Deserialize)]
struct SavedTrack {
    /// Un tema retirado puede venir como `null`.
    track: Option<SavedTrackItem>,
}

#[derive(Deserialize)]
struct SavedTrackItem {
    uri: String,
    #[serde(default)]
    is_local: bool,
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
            .filter_map(|(uri, name, detail)| hit(&uri, &name, detail))
            .chain(playlists.filter_map(playlist_hit))
            .take(config::SEARCH_LIMIT)
            .collect()
    }
}

/// Una playlist como `Hit` ("de dueño · N temas"); URI inválida → `None`.
fn playlist_hit(p: PlaylistItem) -> Option<Hit> {
    let owner = p.owner.display_name.unwrap_or(p.owner.id);
    let detail = match p.items {
        Some(Total { total }) => format!("de {owner} · {total} temas"),
        None => format!("de {owner}"),
    };
    hit(&p.uri, &p.name, detail)
}

/// `Hit` con el nombre sin espacios de más; URI inválida → `None`.
fn hit(uri: &str, name: &str, detail: String) -> Option<Hit> {
    Some(Hit {
        uri: SpotifyUri::from_uri(uri).ok()?,
        name: name.trim().to_string(),
        detail,
    })
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
/// - 403 con `config::USER_NOT_REGISTERED_MESSAGE` en `body` →
///   `UserNotAllowed` (la cuenta no está en *User Management* de la app
///   del Client ID).
/// - 5xx → `Spotify` indicando que es un problema de Spotify.
/// - resto → `Spotify` con el código y la operación (`action`).
fn status_error(
    status: StatusCode,
    retry_after: Option<u64>,
    body: &str,
    action: &str,
) -> AppError {
    match status {
        StatusCode::UNAUTHORIZED => AppError::SessionRejected(format!("{status} al {action}")),
        StatusCode::FORBIDDEN if body.contains(config::USER_NOT_REGISTERED_MESSAGE) => {
            AppError::UserNotAllowed
        }
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
        let e = |status, retry| status_error(status, retry, "", "probar");
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

    #[test]
    fn forbidden_de_usuario_no_habilitado() {
        let body = r#"{"error":{"status":403,"message":"Check settings on developer.spotify.com/dashboard, the user may not be registered."}}"#;
        assert!(matches!(
            status_error(StatusCode::FORBIDDEN, None, body, "probar"),
            AppError::UserNotAllowed
        ));
        // El mismo texto con otro código no es este caso.
        assert!(matches!(
            status_error(StatusCode::BAD_REQUEST, None, body, "probar"),
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
    fn pagina_de_mis_playlists() {
        let json = r#"{"items":[
            {"uri":"spotify:playlist:627lheesMF3W2repUfJb5M","name":"Mía",
             "owner":{"id":"yo","display_name":"Gonza"},"items":{"total":12}},
            null,
            {"uri":"spotify:playlist:1yBGduvI0IvHYwc1JNpdJC","name":"Seguida",
             "owner":{"id":"otro"},"tracks":{"total":3}},
            {"uri":"no-es-uri","name":"Rota","owner":{"id":"z"}}
        ],"next":"https://api.spotify.com/v1/me/playlists?offset=50","total":120}"#;
        let page: PlaylistPage = serde_json::from_str(json).unwrap();
        assert!(page.next.is_some());
        assert_eq!(page.total, 120);
        let lines: Vec<_> = page
            .items
            .into_iter()
            .flatten()
            .filter_map(playlist_hit)
            .map(|h| format!("{} {}", h.name, h.detail))
            .collect();
        assert_eq!(
            lines,
            ["Mía de Gonza · 12 temas", "Seguida de otro · 3 temas"]
        );
    }

    #[test]
    fn ultima_pagina_de_mis_playlists() {
        let page: PlaylistPage =
            serde_json::from_str(r#"{"items":[],"next":null,"total":0}"#).unwrap();
        assert!(page.next.is_none() && page.items.is_empty());
    }

    #[test]
    fn pagina_de_mis_me_gusta() {
        let json = r#"{"items":[
            {"added_at":"2026-09-01T00:00:00Z","track":{"uri":"spotify:track:4PTG3Z6ehGkBFwjybzWkR8","name":"A"}},
            null,
            {"added_at":"x","track":null},
            {"added_at":"x","track":{"uri":"spotify:local:Artista:Album:Tema:180","is_local":true}},
            {"added_at":"x","track":{"uri":"no-es-uri"}},
            {"added_at":"x","track":{"uri":"spotify:track:3pepZAOvUCBt3qWi9Ax6Aq","is_local":false}}
        ],"next":"https://api.spotify.com/v1/me/tracks?offset=50","total":130}"#;
        let page: SavedTrackPage = serde_json::from_str(json).unwrap();
        assert!(page.next.is_some());
        assert_eq!(page.total, 130);
        let mut liked = LikedTracks::default();
        liked.add(page.items);
        let ids: Vec<_> = liked.tracks.iter().map(|u| u.to_uri().unwrap()).collect();
        assert_eq!(
            ids,
            [
                "spotify:track:4PTG3Z6ehGkBFwjybzWkR8",
                "spotify:track:3pepZAOvUCBt3qWi9Ax6Aq"
            ]
        );
        assert_eq!(liked.omitted, 4);
    }

    #[test]
    fn ultima_pagina_de_mis_me_gusta() {
        let page: SavedTrackPage =
            serde_json::from_str(r#"{"items":[],"next":null,"total":0}"#).unwrap();
        assert!(page.next.is_none() && page.items.is_empty());
    }

    #[test]
    fn put_y_delete_van_con_content_length_cero() {
        let http = reqwest::Client::new();
        for method in [Method::PUT, Method::DELETE] {
            let req = request(&http, method.clone(), "/me/library", &[("uris", "x")])
                .build()
                .unwrap();
            assert_eq!(req.headers()[CONTENT_LENGTH], "0", "{method}");
            assert_eq!(req.body().and_then(|b| b.as_bytes()), Some(&[][..]));
        }
        let get = request(&http, Method::GET, "/me", &[]).build().unwrap();
        assert!(get.headers().get(CONTENT_LENGTH).is_none());
    }

    #[test]
    fn uri_para_el_parametro_uris() {
        let uri = SpotifyUri::from_uri("spotify:track:4PTG3Z6ehGkBFwjybzWkR8").unwrap();
        assert_eq!(
            uri_param(&uri).unwrap(),
            "spotify:track:4PTG3Z6ehGkBFwjybzWkR8"
        );
    }

    #[test]
    fn duracion() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(59_999), "0:59");
        assert_eq!(format_duration(4_502_000), "75:02");
    }
}
