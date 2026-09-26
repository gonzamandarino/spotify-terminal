//! Subcomandos de línea. Parseo a mano: son pocos y no justifican `clap`.

use crate::error::AppError;

pub const USAGE: &str = "\
Uso: spotify-terminal <comando>

Comandos:
  login    Inicia sesión en Spotify (o confirma que la sesión guardada sirve)
  logout   Borra la sesión guardada";

#[derive(Debug, PartialEq)]
pub enum Command {
    Login,
    Logout,
    Help,
}

/// Interpreta los argumentos (sin el nombre del programa).
///
/// - Post: sin argumentos o con `help`/`-h`/`--help` → `Command::Help`;
///   un comando desconocido o argumentos de más → `AppError::Usage`.
pub fn parse(args: &[String]) -> Result<Command, AppError> {
    let command = match args.first().map(String::as_str) {
        None | Some("help" | "-h" | "--help") => return Ok(Command::Help),
        Some("login") => Command::Login,
        Some("logout") => Command::Logout,
        Some(other) => {
            return Err(AppError::Usage(format!(
                "comando desconocido: {other}\n\n{USAGE}"
            )));
        }
    };
    if args.len() > 1 {
        return Err(AppError::Usage(format!("demasiados argumentos\n\n{USAGE}")));
    }
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn comandos_validos() {
        assert_eq!(parse(&args(&[])).unwrap(), Command::Help);
        assert_eq!(parse(&args(&["--help"])).unwrap(), Command::Help);
        assert_eq!(parse(&args(&["login"])).unwrap(), Command::Login);
        assert_eq!(parse(&args(&["logout"])).unwrap(), Command::Logout);
    }

    #[test]
    fn comando_desconocido_o_argumentos_de_mas_es_error() {
        assert!(matches!(parse(&args(&["bailar"])), Err(AppError::Usage(_))));
        assert!(matches!(
            parse(&args(&["login", "extra"])),
            Err(AppError::Usage(_))
        ));
    }
}
