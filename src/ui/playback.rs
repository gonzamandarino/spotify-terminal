//! Pantalla mínima de reproducción: muestra el tema y el estado, y lee
//! teclas (espacio = pausa/reanudar, q / Ctrl+C = salir).

use std::io::{self, Write};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal,
};
use librespot_core::SpotifyUri;
use librespot_metadata::audio::UniqueFields;
use librespot_playback::player::PlayerEvent;
use tokio::sync::mpsc;

use crate::{error::AppError, spotify::player::Player};

enum Key {
    TogglePause,
    Quit,
}

/// Reproduce `track` y atiende el teclado hasta que termina el tema o el
/// usuario sale.
///
/// - Post: la terminal vuelve a modo normal siempre (incluso con error).
/// - Errores: `TrackUnavailable` si Spotify no deja reproducir el tema.
pub async fn play_track(player: &Player, track: SpotifyUri) -> Result<(), AppError> {
    let mut events = player.events();
    let _raw = RawMode::enable()?;
    let mut keys = spawn_key_reader();

    player.play(track);
    let mut paused = false;
    status("Cargando...");

    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(PlayerEvent::TrackChanged { audio_item }) => {
                    let artists = match &audio_item.unique_fields {
                        UniqueFields::Track { artists, .. } => artists
                            .iter()
                            .map(|a| a.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                        _ => String::new(),
                    };
                    line(&format!("♪ {} — {}", audio_item.name, artists));
                }
                Some(PlayerEvent::Playing { .. }) => {
                    paused = false;
                    status("▶ Reproduciendo   [espacio] pausa  [q] salir");
                }
                Some(PlayerEvent::Paused { .. }) => {
                    paused = true;
                    status("⏸ En pausa        [espacio] seguir [q] salir");
                }
                Some(PlayerEvent::Unavailable { track_id, .. }) => {
                    return Err(AppError::TrackUnavailable(track_id.to_string()));
                }
                Some(PlayerEvent::EndOfTrack { .. }) | None => break,
                Some(_) => {}
            },
            key = keys.recv() => match key {
                Some(Key::TogglePause) if paused => player.resume(),
                Some(Key::TogglePause) => player.pause(),
                Some(Key::Quit) | None => {
                    player.stop();
                    break;
                }
            },
        }
    }
    line("Fin.");
    Ok(())
}

/// Lee el teclado en un hilo aparte (la lectura de crossterm bloquea) y
/// manda las teclas que importan por un canal.
fn spawn_key_reader() -> mpsc::UnboundedReceiver<Key> {
    let (tx, rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        while let Ok(event) = event::read() {
            // En Windows llegan eventos de soltar la tecla; solo nos importa
            // cuando se aprieta.
            let Event::Key(key) = event else { continue };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let mapped = match key.code {
                KeyCode::Char(' ') => Key::TogglePause,
                KeyCode::Char('q') | KeyCode::Esc => Key::Quit,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Key::Quit,
                _ => continue,
            };
            if tx.send(mapped).is_err() {
                break; // nadie escucha: se terminó la reproducción
            }
        }
    });
    rx
}

/// Reescribe la línea de estado actual.
fn status(text: &str) {
    print!("\r\x1b[2K{text}");
    let _ = io::stdout().flush();
}

/// Imprime una línea fija (en modo raw hace falta `\r\n` explícito).
fn line(text: &str) {
    print!("\r\x1b[2K{text}\r\n");
    let _ = io::stdout().flush();
}

/// Modo raw de la terminal (teclas sin Enter) mientras vive el valor; al
/// soltarse (`Drop`) vuelve al modo normal, aunque se salga por un error.
struct RawMode;

impl RawMode {
    fn enable() -> Result<RawMode, AppError> {
        terminal::enable_raw_mode()?;
        Ok(RawMode)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}
