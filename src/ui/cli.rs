//! Subcomandos de línea. Parseo a mano: son pocos y no justifican `clap`.

use crate::error::AppError;

pub const USAGE: &str = "\
Uso: spotify-terminal <comando>

Comandos:
  login         Inicia sesión en Spotify (o confirma que la sesión guardada sirve)
  logout        Borra la sesión guardada
  whoami        Muestra el usuario logueado y su plan
  play <tema>   Reproduce un tema: URI (spotify:track:ID), link de
                open.spotify.com o ID. Espacio = pausa/reanudar, q = salir";

#[derive(Debug, PartialEq)]
pub enum Command {
    Login,
    Logout,
    Whoami,
    /// URI de tema normalizada (`spotify:track:<ID>`).
    Play(String),
    Help,
}

/// Interpreta los argumentos (sin el nombre del programa).
///
/// - Post: sin argumentos o con `help`/`-h`/`--help` → `Command::Help`;
///   un comando desconocido, un tema inválido o argumentos de más →
///   `AppError::Usage`.
pub fn parse(args: &[String]) -> Result<Command, AppError> {
    let (command, expected_args) = match args.first().map(String::as_str) {
        None | Some("help" | "-h" | "--help") => return Ok(Command::Help),
        Some("login") => (Command::Login, 1),
        Some("logout") => (Command::Logout, 1),
        Some("whoami") => (Command::Whoami, 1),
        Some("play") => {
            let track = args
                .get(1)
                .ok_or_else(|| usage_error("falta el tema a reproducir"))?;
            (Command::Play(parse_track(track)?), 2)
        }
        Some(other) => return Err(usage_error(&format!("comando desconocido: {other}"))),
    };
    if args.len() > expected_args {
        return Err(usage_error("demasiados argumentos"));
    }
    Ok(command)
}

/// Normaliza un tema a `spotify:track:<ID>`. Acepta la URI, un link de
/// `open.spotify.com/track/<ID>` (con o sin `?si=...`) o el ID solo.
fn parse_track(input: &str) -> Result<String, AppError> {
    let id = if let Some(id) = input.strip_prefix("spotify:track:") {
        id
    } else if input.contains("open.spotify.com/") {
        // Puede venir con prefijo de idioma: open.spotify.com/intl-es/track/ID
        input
            .split_once("/track/")
            .and_then(|(_, rest)| rest.split(['?', '/']).next())
            .unwrap_or_default()
    } else {
        input
    };

    // Los IDs de Spotify son 22 caracteres base62.
    if id.len() == 22 && id.chars().all(|c| c.is_ascii_alphanumeric()) {
        Ok(format!("spotify:track:{id}"))
    } else {
        Err(usage_error(&format!("no reconozco el tema: {input}")))
    }
}

fn usage_error(message: &str) -> AppError {
    AppError::Usage(format!("{message}\n\n{USAGE}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "4uLU6hMCjMI75M1A2tKUQC";

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
            Command::Play(format!("spotify:track:{ID}"))
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
        let uri = format!("spotify:track:{ID}");
        for input in [
            uri.clone(),
            ID.to_string(),
            format!("https://open.spotify.com/track/{ID}"),
            format!("https://open.spotify.com/track/{ID}?si=abc123"),
            format!("https://open.spotify.com/intl-es/track/{ID}?si=abc"),
        ] {
            assert_eq!(parse_track(&input).unwrap(), uri, "{input}");
        }
    }

    #[test]
    fn tema_invalido() {
        for input in [
            "",
            "abc",
            "spotify:album:4uLU6hMCjMI75M1A2tKUQC",
            "4uLU6hMCjMI75M1A2tKUQ!",
        ] {
            assert!(parse_track(input).is_err(), "{input}");
        }
    }
}
