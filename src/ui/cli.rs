//! Subcomandos de línea. Parseo a mano: son pocos y no justifican `clap`.

use librespot_core::SpotifyUri;

use crate::{config, error::AppError, spotify::web::SearchKind};

/// Texto de ayuda (`help` y errores de uso).
pub const USAGE: &str = "\
Uso: spotify-terminal <comando>

Comandos:
  login         Inicia sesión en Spotify (o confirma que la sesión guardada sirve)
  logout        Borra la sesión guardada
  whoami        Muestra el usuario logueado y su plan
  play [-s] <nombre>
                Busca temas por nombre y muestra los 5 mejores para elegir
                (1-5, Enter = el primero, q = cancelar)
  play [-s] list <nombre>
                Lo mismo con playlists (también `play playlist <nombre>`)
  play [-s] <link>
                Reproduce un tema, un álbum o una playlist: URI
                (spotify:track:ID, spotify:album:ID, spotify:playlist:ID),
                link de open.spotify.com o ID de tema.
                -s / --shuffle: arranca mezclado, desde un tema al azar.

Durante la reproducción:
  espacio       pausa / reanudar
  n  o  →       siguiente tema
  p  o  ←       reinicia el tema, o vuelve al anterior si recién empezó
  s             shuffle sí / no
  a             busca un tema y lo agrega a la cola (suena después del actual)
  q             salir";

/// Subcomando pedido, ya validado.
#[derive(Debug, PartialEq)]
pub enum Command {
    Login,
    Logout,
    Whoami,
    /// Tema, álbum o playlist a reproducir; `shuffle` = arrancar mezclado.
    Play {
        target: SpotifyUri,
        shuffle: bool,
    },
    /// Buscar por texto y elegir qué reproducir. Invariante: `query` no está
    /// vacío y sus palabras van separadas por un solo espacio.
    Search {
        kind: SearchKind,
        query: String,
        shuffle: bool,
    },
    Help,
}

/// Palabras que, después de `play`, piden buscar playlists en vez de temas.
const PLAYLIST_WORDS: [&str; 2] = ["list", "playlist"];

/// Opción de `play` para arrancar mezclado (en cualquier posición).
const SHUFFLE_FLAGS: [&str; 2] = ["-s", "--shuffle"];

/// Interpreta los argumentos (sin el nombre del programa).
///
/// - Post: sin argumentos o con `help`/`-h`/`--help` → `Command::Help`.
///   `play` con un solo argumento que es URI, link o ID → `Command::Play`;
///   `play list|playlist <texto>` → búsqueda de playlists; cualquier otro
///   texto después de `play` (una o varias palabras) → búsqueda de temas.
///   `-s`/`--shuffle` en cualquier lugar después de `play` prende `shuffle`
///   y no cuenta como texto.
///   Un comando desconocido, `play` sin texto, un URI/link mal formado o
///   argumentos de más → `AppError::Usage`.
pub fn parse(args: &[String]) -> Result<Command, AppError> {
    let (command, expected_args) = match args.first().map(String::as_str) {
        None | Some("help" | "-h" | "--help") => return Ok(Command::Help),
        Some("login") => (Command::Login, 1),
        Some("logout") => (Command::Logout, 1),
        Some("whoami") => (Command::Whoami, 1),
        Some("play") => return parse_play(&args[1..]),
        Some(other) => return Err(usage_error(&format!("comando desconocido: {other}"))),
    };
    if args.len() > expected_args {
        return Err(usage_error("demasiados argumentos"));
    }
    Ok(command)
}

/// Argumentos de `play` (sin la palabra `play`).
fn parse_play(args: &[String]) -> Result<Command, AppError> {
    let shuffle = args.iter().any(|a| SHUFFLE_FLAGS.contains(&a.as_str()));
    let args: Vec<String> = args
        .iter()
        .filter(|a| !SHUFFLE_FLAGS.contains(&a.as_str()))
        .cloned()
        .collect();
    if let [single] = args.as_slice() {
        // Algo que parece un URI o link se valida como tal: si está mal, es
        // más útil decirlo que buscarlo como texto.
        if looks_like_link(single) {
            let target = parse_playable(single)?;
            return Ok(Command::Play { target, shuffle });
        }
        if let Ok(target) = parse_playable(single) {
            return Ok(Command::Play { target, shuffle });
        }
    }
    let (kind, words) = match args.split_first() {
        Some((first, rest)) if PLAYLIST_WORDS.contains(&first.as_str()) => {
            (SearchKind::Playlist, rest)
        }
        _ => (SearchKind::Track, args.as_slice()),
    };
    let query = words
        .iter()
        .flat_map(|w| w.split_whitespace())
        .collect::<Vec<_>>()
        .join(" ");
    if query.is_empty() {
        return Err(usage_error(match kind {
            SearchKind::Track => "falta qué reproducir",
            SearchKind::Playlist => "falta el nombre de la playlist",
        }));
    }
    Ok(Command::Search {
        kind,
        query,
        shuffle,
    })
}

fn looks_like_link(input: &str) -> bool {
    input.starts_with("spotify:") || input.contains("open.spotify.com/")
}

/// Tipos que acepta `play`, tal como aparecen en URIs y links.
const PLAYABLE_KINDS: [&str; 3] = ["track", "album", "playlist"];

/// Interpreta lo que se pide reproducir. Acepta la URI
/// (`spotify:<tipo>:<ID>`), un link de `open.spotify.com/<tipo>/<ID>` (con o
/// sin `?si=...` o prefijo de idioma) o el ID solo, que se toma como tema.
fn parse_playable(input: &str) -> Result<SpotifyUri, AppError> {
    let (kind, id) = if let Some(rest) = input.strip_prefix("spotify:") {
        rest.split_once(':').unwrap_or_default()
    } else if let Some((_, path)) = input.split_once("open.spotify.com/") {
        // Puede venir con prefijo de idioma: open.spotify.com/intl-es/track/ID
        let mut parts = path
            .split(['?', '/'])
            .skip_while(|p| !PLAYABLE_KINDS.contains(p));
        (
            parts.next().unwrap_or_default(),
            parts.next().unwrap_or_default(),
        )
    } else {
        ("track", input)
    };

    let valid_id =
        id.len() == config::SPOTIFY_ID_LEN && id.chars().all(|c| c.is_ascii_alphanumeric());
    let unrecognized = || usage_error(&format!("no reconozco qué reproducir: {input}"));
    if !(PLAYABLE_KINDS.contains(&kind) && valid_id) {
        return Err(unrecognized());
    }
    SpotifyUri::from_uri(&format!("spotify:{kind}:{id}")).map_err(|_| unrecognized())
}

fn usage_error(message: &str) -> AppError {
    AppError::Usage(format!("{message}\n\n{USAGE}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "4uLU6hMCjMI75M1A2tKUQC";

    fn uri(text: &str) -> SpotifyUri {
        SpotifyUri::from_uri(text).unwrap()
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn comandos_validos() {
        assert_eq!(parse(&args(&[])).unwrap(), Command::Help);
        assert_eq!(parse(&args(&["--help"])).unwrap(), Command::Help);
        assert_eq!(parse(&args(&["login"])).unwrap(), Command::Login);
        assert_eq!(parse(&args(&["logout"])).unwrap(), Command::Logout);
        assert_eq!(parse(&args(&["whoami"])).unwrap(), Command::Whoami);
        assert_eq!(
            parse(&args(&["play", ID])).unwrap(),
            Command::Play {
                target: uri(&format!("spotify:track:{ID}")),
                shuffle: false
            }
        );
    }

    #[test]
    fn comando_desconocido_o_argumentos_de_mas_es_error() {
        for bad in [
            &["bailar"][..],
            &["login", "extra"],
            &["play"],
            &["play", "  "],
            &["play", "-s"],
            &["play", "--shuffle", "list"],
            &["play", "list"],
            &["play", "playlist", " "],
            &["play", "spotify:artist:4uLU6hMCjMI75M1A2tKUQC"],
            &[
                "play",
                "https://open.spotify.com/artist/4uLU6hMCjMI75M1A2tKUQC",
            ],
        ] {
            assert!(
                matches!(parse(&args(bad)), Err(AppError::Usage(_))),
                "{bad:?}"
            );
        }
    }

    fn search(kind: SearchKind, query: &str) -> Command {
        Command::Search {
            kind,
            query: query.into(),
            shuffle: false,
        }
    }

    #[test]
    fn shuffle_en_cualquier_lugar() {
        let playlist = format!("spotify:playlist:{ID}");
        for list in [
            &["play", "-s", &playlist][..],
            &["play", &playlist, "--shuffle"],
        ] {
            assert_eq!(
                parse(&args(list)).unwrap(),
                Command::Play {
                    target: uri(&playlist),
                    shuffle: true
                },
                "{list:?}"
            );
        }
        assert_eq!(
            parse(&args(&["play", "list", "-s", "rock", "nacional"])).unwrap(),
            Command::Search {
                kind: SearchKind::Playlist,
                query: "rock nacional".into(),
                shuffle: true
            }
        );
    }

    #[test]
    fn busqueda_de_temas() {
        assert_eq!(
            parse(&args(&["play", "never", "gonna", "give", "you", "up"])).unwrap(),
            search(SearchKind::Track, "never gonna give you up")
        );
        assert_eq!(
            parse(&args(&["play", "  soda   stereo "])).unwrap(),
            search(SearchKind::Track, "soda stereo")
        );
        // Entre comillas, "list" no pide playlists.
        assert_eq!(
            parse(&args(&["play", "list of demands"])).unwrap(),
            search(SearchKind::Track, "list of demands")
        );
        // Un ID con algo más ya no es un ID: se busca como texto.
        assert_eq!(
            parse(&args(&["play", ID, "x"])).unwrap(),
            search(SearchKind::Track, &format!("{ID} x"))
        );
    }

    #[test]
    fn busqueda_de_playlists() {
        for word in PLAYLIST_WORDS {
            assert_eq!(
                parse(&args(&["play", word, "rock", "nacional"])).unwrap(),
                search(SearchKind::Playlist, "rock nacional")
            );
        }
        assert_eq!(
            parse(&args(&["play", "list", "of", "demands"])).unwrap(),
            search(SearchKind::Playlist, "of demands")
        );
    }

    #[test]
    fn uri_link_o_id_no_busca() {
        for input in [
            ID.to_string(),
            format!("spotify:album:{ID}"),
            format!("https://open.spotify.com/playlist/{ID}?si=x"),
        ] {
            assert!(
                matches!(
                    parse(&args(&["play", &input])).unwrap(),
                    Command::Play { shuffle: false, .. }
                ),
                "{input}"
            );
        }
    }

    #[test]
    fn tema_en_todos_los_formatos() {
        let text = format!("spotify:track:{ID}");
        let expected = uri(&text);
        for input in [
            text,
            ID.to_string(),
            format!("https://open.spotify.com/track/{ID}"),
            format!("https://open.spotify.com/track/{ID}?si=abc123"),
            format!("https://open.spotify.com/intl-es/track/{ID}?si=abc"),
        ] {
            assert_eq!(parse_playable(&input).unwrap(), expected, "{input}");
        }
    }

    #[test]
    fn album_y_playlist() {
        for kind in ["album", "playlist"] {
            let text = format!("spotify:{kind}:{ID}");
            let expected = uri(&text);
            for input in [
                text,
                format!("https://open.spotify.com/{kind}/{ID}?si=x"),
                format!("https://open.spotify.com/intl-es/{kind}/{ID}"),
            ] {
                assert_eq!(parse_playable(&input).unwrap(), expected, "{input}");
            }
        }
    }

    #[test]
    fn entrada_invalida() {
        for input in [
            "",
            "abc",
            "spotify:artist:4uLU6hMCjMI75M1A2tKUQC",
            "spotify:track:",
            "https://open.spotify.com/artist/4uLU6hMCjMI75M1A2tKUQC",
            "4uLU6hMCjMI75M1A2tKUQ!",
        ] {
            assert!(parse_playable(input).is_err(), "{input}");
        }
    }
}
