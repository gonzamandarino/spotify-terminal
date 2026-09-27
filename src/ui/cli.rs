//! Subcomandos de línea. Parseo a mano: son pocos y no justifican `clap`.

use librespot_core::SpotifyUri;

use crate::{config, error::AppError};

/// Texto de ayuda (`help` y errores de uso).
pub const USAGE: &str = "\
Uso: spotify-terminal <comando>

Comandos:
  login         Inicia sesión en Spotify (o confirma que la sesión guardada sirve)
  logout        Borra la sesión guardada
  whoami        Muestra el usuario logueado y su plan
  play <qué>    Reproduce un tema, un álbum o una playlist: URI
                (spotify:track:ID, spotify:album:ID, spotify:playlist:ID),
                link de open.spotify.com o ID de tema.
                Espacio = pausa/reanudar, q = salir";

/// Subcomando pedido, ya validado.
#[derive(Debug, PartialEq)]
pub enum Command {
    Login,
    Logout,
    Whoami,
    /// Tema, álbum o playlist a reproducir.
    Play(SpotifyUri),
    Help,
}

/// Interpreta los argumentos (sin el nombre del programa).
///
/// - Post: sin argumentos o con `help`/`-h`/`--help` → `Command::Help`;
///   un comando desconocido, algo irreconocible para `play` o argumentos
///   de más → `AppError::Usage`.
pub fn parse(args: &[String]) -> Result<Command, AppError> {
    let (command, expected_args) = match args.first().map(String::as_str) {
        None | Some("help" | "-h" | "--help") => return Ok(Command::Help),
        Some("login") => (Command::Login, 1),
        Some("logout") => (Command::Logout, 1),
        Some("whoami") => (Command::Whoami, 1),
        Some("play") => {
            let track = args
                .get(1)
                .ok_or_else(|| usage_error("falta qué reproducir"))?;
            (Command::Play(parse_playable(track)?), 2)
        }
        Some(other) => return Err(usage_error(&format!("comando desconocido: {other}"))),
    };
    if args.len() > expected_args {
        return Err(usage_error("demasiados argumentos"));
    }
    Ok(command)
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
            Command::Play(uri(&format!("spotify:track:{ID}")))
        );
    }

    #[test]
    fn comando_desconocido_o_argumentos_de_mas_es_error() {
        for bad in [
            &["bailar"][..],
            &["login", "extra"],
            &["play"],
            &["play", ID, "x"],
        ] {
            assert!(
                matches!(parse(&args(bad)), Err(AppError::Usage(_))),
                "{bad:?}"
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
