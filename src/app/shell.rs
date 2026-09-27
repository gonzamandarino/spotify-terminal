//! Comandos que se escriben en la consola de la app de escritorio: los de la
//! CLI (`ui::cli`) más los controles de lo que suena.

use crate::ui::cli::{self, Command};

/// Ayuda de la consola (`help`).
pub(crate) const HELP: &str = "\
Comandos:
  play <nombre>          busca temas y elegís uno (1-5, Enter = el primero)
  play list <nombre>     lo mismo con playlists
  play <link>            tema, álbum o playlist por link, URI o ID
  play -s <…>            igual, pero arranca mezclado
  pause                  pausa / reanudar
  next, n                siguiente tema
  prev, p                anterior o reinicia el tema
  shuffle, s             shuffle sí / no
  queue <tema>, a <tema> busca un tema y lo agrega a la cola
  vol, v                 muestra el volumen
  vol <0-100>            fija el volumen
  vol + / vol -          sube / baja el volumen
  mute, m                silencia / vuelve al volumen de antes
  stop                   corta la reproducción y vacía la cola
  login / logout         inicia / cierra la sesión de Spotify
  setup                  configura o cambia el Client ID de tu app de Spotify
  whoami                 usuario y plan
  clear                  limpia la consola
  version                versión de la app
  exit                   cierra la app

↑ ↓ historial · Tab completa · Esc cancela una búsqueda o elección";

/// Nombres de comando, para completar con Tab.
const NAMES: [&str; 17] = [
    "play", "pause", "next", "prev", "shuffle", "queue", "stop", "vol", "mute", "login", "logout",
    "setup", "whoami", "version", "clear", "help", "exit",
];

/// Una línea de la consola, ya interpretada.
#[derive(Debug, PartialEq)]
pub(crate) enum ShellCommand {
    /// Línea vacía o solo espacios.
    Empty,
    Help,
    Pause,
    Next,
    Prev,
    Shuffle,
    /// Buscar un tema para encolar. Invariante: no vacío, palabras
    /// separadas por un solo espacio.
    Queue(String),
    Stop,
    Volume(VolumeCommand),
    Mute,
    Clear,
    Exit,
    /// `login`, `logout`, `setup`, `version`, `whoami`, `play …` (nunca
    /// `Command::Help`).
    Cli(Command),
}

/// `vol …`.
#[derive(Debug, PartialEq)]
pub(crate) enum VolumeCommand {
    /// `vol` solo: mostrar el actual.
    Show,
    /// `vol N`. Invariante: ≤ 100.
    Set(u8),
    Up,
    Down,
}

/// Interpreta una línea escrita en la consola.
///
/// - Post: el nombre del comando no distingue mayúsculas. Las palabras se
///   separan por espacios; entre comillas (`"…"` o `'…'`) un texto cuenta
///   como una sola palabra, igual que en PowerShell (`play "list of
///   demands"` busca ese tema). `help` → `Help`; `vol` acepta nada, `+`,
///   `-` o un entero de 0 a 100; los comandos de la CLI se validan con
///   `cli::parse_command`.
/// - Errores: el motivo, sin la ayuda (comando desconocido, argumentos de
///   más o de menos, comillas sin cerrar, link mal formado).
pub(crate) fn parse_line(line: &str) -> Result<ShellCommand, String> {
    let mut words = split_words(line)?;
    let Some(first) = words.first_mut() else {
        return Ok(ShellCommand::Empty);
    };
    *first = first.to_lowercase();
    let no_args = |command| {
        if words.len() > 1 {
            Err(format!("{} no lleva nada más", words[0]))
        } else {
            Ok(command)
        }
    };
    match words[0].as_str() {
        "pause" => no_args(ShellCommand::Pause),
        "next" | "n" => no_args(ShellCommand::Next),
        "prev" | "p" => no_args(ShellCommand::Prev),
        "shuffle" | "s" => no_args(ShellCommand::Shuffle),
        "stop" => no_args(ShellCommand::Stop),
        "mute" | "m" => no_args(ShellCommand::Mute),
        "vol" | "v" => parse_volume(&words[1..]).map(ShellCommand::Volume),
        "clear" => no_args(ShellCommand::Clear),
        "exit" => no_args(ShellCommand::Exit),
        "queue" | "a" => {
            let query = words[1..]
                .iter()
                .flat_map(|w| w.split_whitespace())
                .collect::<Vec<_>>()
                .join(" ");
            if query.is_empty() {
                return Err("falta qué tema encolar".into());
            }
            Ok(ShellCommand::Queue(query))
        }
        _ => match cli::parse_command(&words)? {
            Command::Help => Ok(ShellCommand::Help),
            command => Ok(ShellCommand::Cli(command)),
        },
    }
}

/// Argumentos de `vol`.
fn parse_volume(args: &[String]) -> Result<VolumeCommand, String> {
    match args {
        [] => Ok(VolumeCommand::Show),
        [arg] if arg == "+" => Ok(VolumeCommand::Up),
        [arg] if arg == "-" => Ok(VolumeCommand::Down),
        [arg] => match arg.parse::<u8>() {
            Ok(level) if level <= 100 => Ok(VolumeCommand::Set(level)),
            _ => Err(format!(
                "vol {arg}: el volumen va de 0 a 100 (o `vol +` / `vol -`)"
            )),
        },
        _ => Err("vol lleva un solo valor: 0 a 100, + o -".into()),
    }
}

/// Nombres de comando que empiezan con lo escrito, para Tab. Solo completa
/// la primera palabra: si ya hay un espacio, no hay sugerencias.
pub(crate) fn complete(input: &str) -> Vec<&'static str> {
    let prefix = input.trim_start().to_lowercase();
    if prefix.is_empty() || prefix.contains(char::is_whitespace) {
        return Vec::new();
    }
    NAMES
        .iter()
        .copied()
        .filter(|name| name.starts_with(&prefix))
        .collect()
}

/// Separa en palabras respetando comillas dobles o simples.
fn split_words(line: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current: Option<String> = None;
    let mut quote: Option<char> = None;
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => current.get_or_insert_default().push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                current.get_or_insert_default();
            }
            None if c.is_whitespace() => words.extend(current.take()),
            None => current.get_or_insert_default().push(c),
        }
    }
    if quote.is_some() {
        return Err("faltan cerrar comillas".into());
    }
    words.extend(current);
    Ok(words)
}

#[cfg(test)]
mod tests {
    use librespot_core::SpotifyUri;

    use super::*;
    use crate::spotify::web::SearchKind;

    #[test]
    fn controles_y_alias() {
        for (line, expected) in [
            ("pause", ShellCommand::Pause),
            ("next", ShellCommand::Next),
            ("n", ShellCommand::Next),
            ("prev", ShellCommand::Prev),
            ("p", ShellCommand::Prev),
            ("  Shuffle ", ShellCommand::Shuffle),
            ("s", ShellCommand::Shuffle),
            ("stop", ShellCommand::Stop),
            ("clear", ShellCommand::Clear),
            ("exit", ShellCommand::Exit),
            ("help", ShellCommand::Help),
            ("", ShellCommand::Empty),
            ("   ", ShellCommand::Empty),
        ] {
            assert_eq!(parse_line(line), Ok(expected), "{line:?}");
        }
    }

    #[test]
    fn volumen() {
        for (line, expected) in [
            ("vol", VolumeCommand::Show),
            ("V", VolumeCommand::Show),
            ("vol 0", VolumeCommand::Set(0)),
            ("vol 100", VolumeCommand::Set(100)),
            ("v 35", VolumeCommand::Set(35)),
            ("vol +", VolumeCommand::Up),
            ("v -", VolumeCommand::Down),
        ] {
            assert_eq!(
                parse_line(line),
                Ok(ShellCommand::Volume(expected)),
                "{line:?}"
            );
        }
        assert_eq!(parse_line("mute"), Ok(ShellCommand::Mute));
        assert_eq!(parse_line("m"), Ok(ShellCommand::Mute));
        for line in [
            "vol 150", "vol -3", "vol abc", "vol 1 2", "v 7.5", "mute ya",
        ] {
            assert!(parse_line(line).is_err(), "{line:?}");
        }
    }

    #[test]
    fn encolar() {
        assert_eq!(
            parse_line("queue  never gonna"),
            Ok(ShellCommand::Queue("never gonna".into()))
        );
        assert_eq!(
            parse_line("a \"rick  astley\""),
            Ok(ShellCommand::Queue("rick astley".into()))
        );
        assert!(parse_line("queue").is_err());
        assert!(parse_line("a   ").is_err());
    }

    #[test]
    fn comandos_de_la_cli() {
        assert_eq!(parse_line("whoami"), Ok(ShellCommand::Cli(Command::Whoami)));
        assert_eq!(parse_line("LOGIN"), Ok(ShellCommand::Cli(Command::Login)));
        assert_eq!(
            parse_line("play list rock nacional"),
            Ok(ShellCommand::Cli(Command::Search {
                kind: SearchKind::Playlist,
                query: "rock nacional".into(),
                shuffle: false,
            }))
        );
        assert_eq!(
            parse_line("play \"list of demands\""),
            Ok(ShellCommand::Cli(Command::Search {
                kind: SearchKind::Track,
                query: "list of demands".into(),
                shuffle: false,
            }))
        );
        let id = "4uLU6hMCjMI75M1A2tKUQC";
        assert_eq!(
            parse_line(&format!("play -s spotify:album:{id}")),
            Ok(ShellCommand::Cli(Command::Play {
                target: SpotifyUri::from_uri(&format!("spotify:album:{id}")).unwrap(),
                shuffle: true,
            }))
        );
    }

    #[test]
    fn errores_sin_la_ayuda_entera() {
        for line in [
            "bailar",
            "play",
            "next ya",
            "whoami x",
            "play \"abc",
            "play spotify:artist:x",
        ] {
            let error = parse_line(line).unwrap_err();
            assert!(!error.contains("Comandos:"), "{line:?}: {error}");
        }
    }

    #[test]
    fn completar() {
        assert_eq!(complete("pl"), vec!["play"]);
        assert_eq!(complete("P"), vec!["play", "pause", "prev"]);
        assert_eq!(complete("log"), vec!["login", "logout"]);
        assert!(complete("").is_empty());
        assert!(complete("play x").is_empty());
        assert!(complete("zz").is_empty());
    }

    #[test]
    fn comillas() {
        assert_eq!(
            split_words("a 'b c' \"\" d").unwrap(),
            ["a", "b c", "", "d"]
        );
    }
}
