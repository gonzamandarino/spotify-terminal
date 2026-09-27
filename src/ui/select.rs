//! Elección de un resultado de búsqueda: lista numerada y una tecla
//! (1..N, Enter = el primero, q / Esc / Ctrl+C = cancelar).

use std::io::{self, Write};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;

use super::RawMode;
use crate::{error::AppError, spotify::web::Hit};

/// Muestra `hits` numerados y espera la elección.
///
/// - Pre: `hits` no vacío y con a lo sumo 9 elementos (una tecla por
///   resultado).
/// - Post: `Some(i)` con `i < hits.len()` si se eligió, `None` si se
///   canceló. La terminal vuelve a modo normal siempre (incluso con error).
///   Las teclas que no corresponden a un resultado se ignoran.
/// - No debe: reproducir ni hacer pedidos a Spotify.
pub async fn choose(hits: &[Hit]) -> Result<Option<usize>, AppError> {
    for (i, hit) in hits.iter().enumerate() {
        println!("  {}. {} — {}", i + 1, hit.name, hit.detail);
    }
    let _raw = RawMode::enable()?;
    let mut out = io::stdout().lock();
    let _ = write!(out, "Elegí [1-{}] (Enter = 1, q = cancelar): ", hits.len());
    let _ = out.flush();
    drop(out);

    let mut keys = EventStream::new();
    loop {
        match keys.next().await {
            Some(Ok(Event::Key(key))) => match map_key(key, hits.len()) {
                Some(Choice::Pick(i)) => {
                    newline(&(i + 1).to_string());
                    return Ok(Some(i));
                }
                Some(Choice::Cancel) => {
                    newline("");
                    return Ok(None);
                }
                None => {}
            },
            Some(Ok(_)) => {}
            Some(Err(e)) => return Err(AppError::Terminal(e)),
            None => {
                return Err(AppError::Terminal(io::Error::other(
                    "se cerró la entrada del teclado",
                )));
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum Choice {
    Pick(usize),
    Cancel,
}

fn map_key(key: KeyEvent, len: usize) -> Option<Choice> {
    // En Windows llegan también los eventos de soltar la tecla.
    if key.kind != KeyEventKind::Press {
        return None;
    }
    match key.code {
        KeyCode::Enter => Some(Choice::Pick(0)),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Choice::Cancel),
        KeyCode::Char('q') | KeyCode::Esc => Some(Choice::Cancel),
        KeyCode::Char(c) => {
            let n = c.to_digit(10)? as usize;
            (1..=len).contains(&n).then(|| Choice::Pick(n - 1))
        }
        _ => None,
    }
}

/// Muestra `echo` y termina la línea (en modo raw hace falta `\r\n`).
fn newline(echo: &str) {
    let mut out = io::stdout().lock();
    let _ = write!(out, "{echo}\r\n");
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn numeros_dentro_del_rango_eligen() {
        assert_eq!(map_key(press(KeyCode::Char('1')), 5), Some(Choice::Pick(0)));
        assert_eq!(map_key(press(KeyCode::Char('5')), 5), Some(Choice::Pick(4)));
        assert_eq!(map_key(press(KeyCode::Enter), 3), Some(Choice::Pick(0)));
    }

    #[test]
    fn teclas_fuera_de_rango_se_ignoran() {
        for c in ['0', '4', '9', 'x', ' '] {
            assert_eq!(map_key(press(KeyCode::Char(c)), 3), None, "{c}");
        }
        let mut release = press(KeyCode::Char('1'));
        release.kind = KeyEventKind::Release;
        assert_eq!(map_key(release, 3), None);
    }

    #[test]
    fn cancelar() {
        assert_eq!(map_key(press(KeyCode::Char('q')), 5), Some(Choice::Cancel));
        assert_eq!(map_key(press(KeyCode::Esc), 5), Some(Choice::Cancel));
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL), 5),
            Some(Choice::Cancel)
        );
    }
}
