//! Pantalla mínima de reproducción: muestra el tema y el estado, y lee
//! teclas (espacio = pausa/reanudar, n / p = siguiente / anterior,
//! s = shuffle, a = encolar, q / Esc / Ctrl+C = salir).

use std::{
    future::Future,
    io::{self, Write},
    pin::Pin,
};

use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal,
};
use futures_util::StreamExt;
use librespot_core::SpotifyUri;
use librespot_metadata::audio::UniqueFields;
use librespot_playback::player::PlayerEvent;

use super::{
    RawMode,
    select::{self, Choice},
};
use crate::{
    app::queue::{Clock, Queue, Step, uri_text},
    error::AppError,
    spotify::{
        player::Player,
        web::{Hit, SearchKind, WebClient},
    },
};

/// Reproduce `tracks` y atiende el teclado hasta que no queda nada por
/// sonar o el usuario sale.
///
/// - Pre: ninguna; con `tracks` vacío no hace nada y devuelve `Ok`.
/// - Post: la terminal vuelve a modo normal siempre (incluso con error).
///   Con `shuffle`, arranca por un tema al azar y sigue mezclado (con un
///   solo tema lo avisa y no mezcla). Mientras suena un tema se precarga el
///   siguiente, para que no haya silencio entre temas. Solo cuentan los
///   eventos del tema que se pidió último (`play_request_id`): los repetidos,
///   los de la precarga y los de temas salteados con `n`/`p` se ignoran.
///   Lo encolado con `a` suena después del tema actual, antes del resto de
///   la lista; la búsqueda corre dentro del loop sin frenar los eventos del
///   reproductor, y si falla se avisa y se sigue.
/// - Errores: con un solo tema y nada más por sonar, `TrackUnavailable` si
///   no se puede reproducir; en otro caso el tema no disponible se saltea
///   con aviso, salvo que se haya caído la sesión o fallen
///   `config::MAX_CONSECUTIVE_UNAVAILABLE` seguidos → `Network`. Si el
///   reproductor se cierra solo → `PlayerStopped`.
/// - No debe: bloquear el loop esperando a la Web API ni buscar mientras
///   se escribe (un pedido por Enter).
pub async fn play_queue(
    player: &Player,
    web: &WebClient,
    tracks: &[SpotifyUri],
    shuffle: bool,
) -> Result<(), AppError> {
    let mut rng = rand::rng();
    let mut queue = Queue::new(tracks.to_vec(), shuffle, &mut rng);
    let Some(first) = queue.start() else {
        return Ok(());
    };
    let mut events = player.events();
    let _raw = RawMode::enable()?;
    let mut keys = EventStream::new();

    if shuffle && !queue.can_shuffle() {
        line(NOTHING_TO_SHUFFLE);
    }
    player.play(first);
    let mut state = PlayState::Loading;
    let mut mode = Mode::Normal;
    let mut clock = Clock::default();
    let mut search: Option<SearchFuture<'_>> = None;
    draw(&mode, state, &queue);

    loop {
        let step = tokio::select! {
            event = events.recv() => {
                let Some(event) = event else {
                    return Err(AppError::PlayerStopped);
                };
                on_event(event, &mut queue, &mut state, &mut clock, player)
            }
            (query, result) = poll_search(&mut search) => {
                search = None;
                mode = match result {
                    Ok(hits) if hits.is_empty() => {
                        line(&format!("⚠ {}", AppError::NoResults { kind: SearchKind::Track.plural(), query }));
                        Mode::Normal
                    }
                    Ok(hits) => {
                        for (i, hit) in hits.iter().enumerate() {
                            line(&select::hit_line(i, hit));
                        }
                        Mode::Choosing(hits)
                    }
                    Err(e) => {
                        line(&format!("⚠ {e}"));
                        Mode::Normal
                    }
                };
                Step::Nothing
            }
            key = keys.next() => match key {
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                    let (next_mode, action) = on_key(key, mode, &mut search, web);
                    mode = next_mode;
                    match action {
                        Action::None => Step::Nothing,
                        Action::TogglePause => {
                            match state {
                                PlayState::Paused => player.resume(),
                                _ => player.pause(),
                            }
                            Step::Nothing
                        }
                        Action::Next => queue.next(),
                        Action::Previous => queue.previous(clock.elapsed()),
                        Action::ToggleShuffle => match queue.toggle_shuffle(&mut rng) {
                            Some(on) => {
                                line(if on { "🔀 Shuffle: sí" } else { "➡ Shuffle: no" });
                                queue.repreload()
                            }
                            None => {
                                line(NOTHING_TO_SHUFFLE);
                                Step::Nothing
                            }
                        },
                        Action::Enqueue(hit) => {
                            line(&format!("➕ En cola: {} — {}", hit.name, hit.detail));
                            queue.enqueue(hit.uri);
                            queue.repreload()
                        }
                        Action::Quit => Step::Done,
                    }
                }
                Some(Ok(_)) => Step::Nothing,
                Some(Err(e)) => {
                    player.stop();
                    return Err(AppError::Terminal(e));
                }
                None => {
                    player.stop();
                    return Err(AppError::Terminal(io::Error::other("se cerró la entrada del teclado")));
                }
            },
        };

        match step {
            Step::Nothing => {}
            Step::Preload(next) => player.preload(next),
            Step::Play(next) => {
                player.play(next);
                state = PlayState::Loading;
                clock = Clock::default();
            }
            Step::Restart => {
                player.restart();
                clock.seeked(0);
            }
            Step::Skip { unavailable, next } => {
                line(&format!(
                    "⚠ {} no está disponible, sigo con el próximo.",
                    uri_text(&unavailable)
                ));
                player.play(next);
                state = PlayState::Loading;
                clock = Clock::default();
            }
            Step::Done => {
                player.stop();
                break;
            }
            Step::Fail(e) => {
                player.stop();
                return Err(e);
            }
        }
        draw(&mode, state, &queue);
    }
    line("Fin.");
    Ok(())
}

/// Hace que un panic (nuestro o de un hilo de librespot) deje la terminal
/// en modo normal antes de imprimir su mensaje. Llamar una vez al arrancar.
/// No cubre un `process::exit` de librespot.
pub fn restore_terminal_on_panic() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        default_hook(info);
    }));
}

const NOTHING_TO_SHUFFLE: &str = "⚠ Hay un solo tema: no hay nada que mezclar.";

/// Búsqueda en curso: devuelve el texto buscado y el resultado.
type SearchFuture<'a> = Pin<Box<dyn Future<Output = (String, Result<Vec<Hit>, AppError>)> + 'a>>;

/// Espera la búsqueda en curso; sin búsqueda, no termina nunca (la rama del
/// `select!` queda inactiva).
async fn poll_search(
    search: &mut Option<SearchFuture<'_>>,
) -> (String, Result<Vec<Hit>, AppError>) {
    match search.as_mut() {
        Some(future) => future.await,
        None => std::future::pending().await,
    }
}

/// Actualiza la cola, el estado y el reloj con un evento del reproductor.
fn on_event(
    event: PlayerEvent,
    queue: &mut Queue,
    state: &mut PlayState,
    clock: &mut Clock,
    player: &Player,
) -> Step {
    match event {
        PlayerEvent::PlayRequestIdChanged { play_request_id } => {
            queue.on_request_id(play_request_id);
            Step::Nothing
        }
        PlayerEvent::TrackChanged { audio_item } => {
            let artists = match &audio_item.unique_fields {
                UniqueFields::Track { artists, .. } => artists
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => String::new(),
            };
            line(&format!(
                "♪ {}{} — {}",
                queue.position(),
                audio_item.name,
                artists
            ));
            Step::Nothing
        }
        PlayerEvent::Playing {
            play_request_id,
            position_ms,
            ..
        } => {
            if queue.on_started(play_request_id) {
                *state = PlayState::Playing;
                clock.playing(position_ms);
            }
            Step::Nothing
        }
        PlayerEvent::Paused {
            play_request_id,
            position_ms,
            ..
        } => {
            if queue.on_started(play_request_id) {
                *state = PlayState::Paused;
                clock.paused(position_ms);
            }
            Step::Nothing
        }
        PlayerEvent::Seeked {
            play_request_id,
            position_ms,
            ..
        }
        | PlayerEvent::PositionCorrection {
            play_request_id,
            position_ms,
            ..
        } => {
            if queue.is_current(play_request_id) {
                clock.seeked(position_ms);
            }
            Step::Nothing
        }
        PlayerEvent::TimeToPreloadNextTrack {
            play_request_id, ..
        } => queue.on_preload_time(play_request_id),
        PlayerEvent::EndOfTrack {
            play_request_id, ..
        } => queue.on_end(play_request_id),
        PlayerEvent::Unavailable {
            play_request_id, ..
        } => queue.on_unavailable(play_request_id, player.session_lost()),
        _ => Step::Nothing,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PlayState {
    /// Se pidió un tema y todavía no empezó a sonar.
    Loading,
    Playing,
    Paused,
}

/// Qué está haciendo el teclado.
#[derive(Debug)]
enum Mode {
    /// Controles de reproducción.
    Normal,
    /// Escribiendo qué tema buscar para encolar.
    Typing(String),
    /// Esperando la respuesta de la búsqueda.
    Searching,
    /// Eligiendo entre los resultados.
    Choosing(Vec<Hit>),
}

/// Lo que pide una tecla, además del cambio de modo.
#[derive(Debug, PartialEq)]
enum Action {
    None,
    TogglePause,
    Next,
    Previous,
    ToggleShuffle,
    Enqueue(Hit),
    Quit,
}

/// Interpreta una tecla (solo eventos de presionar) según el modo. Al
/// apretar Enter con texto arranca la búsqueda en `search`; Esc la cancela.
fn on_key<'a>(
    key: KeyEvent,
    mode: Mode,
    search: &mut Option<SearchFuture<'a>>,
    web: &'a WebClient,
) -> (Mode, Action) {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return (mode, Action::Quit);
    }
    match mode {
        Mode::Normal => match key.code {
            KeyCode::Char('a') => (Mode::Typing(String::new()), Action::None),
            _ => (Mode::Normal, map_normal_key(key.code)),
        },
        Mode::Typing(mut text) => match key.code {
            KeyCode::Esc => (Mode::Normal, Action::None),
            KeyCode::Enter => {
                let query = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if query.is_empty() {
                    return (Mode::Normal, Action::None);
                }
                *search = Some(Box::pin(async move {
                    let result = web.search(SearchKind::Track, &query).await;
                    (query, result)
                }));
                (Mode::Searching, Action::None)
            }
            KeyCode::Backspace => {
                text.pop();
                (Mode::Typing(text), Action::None)
            }
            KeyCode::Char(c) => {
                text.push(c);
                (Mode::Typing(text), Action::None)
            }
            _ => (Mode::Typing(text), Action::None),
        },
        Mode::Searching => match key.code {
            KeyCode::Esc => {
                *search = None;
                (Mode::Normal, Action::None)
            }
            _ => (Mode::Searching, Action::None),
        },
        Mode::Choosing(mut hits) => match select::map_key(key, hits.len()) {
            Some(Choice::Pick(i)) => (Mode::Normal, Action::Enqueue(hits.swap_remove(i))),
            Some(Choice::Cancel) => (Mode::Normal, Action::None),
            None => (Mode::Choosing(hits), Action::None),
        },
    }
}

fn map_normal_key(code: KeyCode) -> Action {
    match code {
        KeyCode::Char(' ') => Action::TogglePause,
        KeyCode::Char('n') | KeyCode::Right => Action::Next,
        KeyCode::Char('p') | KeyCode::Left => Action::Previous,
        KeyCode::Char('s') => Action::ToggleShuffle,
        KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
        _ => Action::None,
    }
}

/// Redibuja la línea de estado según el modo.
fn draw(mode: &Mode, state: PlayState, queue: &Queue) {
    let text = match mode {
        Mode::Normal => {
            let icon = match state {
                PlayState::Loading => "… Cargando",
                PlayState::Playing => "▶ Sonando",
                PlayState::Paused => "⏸ En pausa",
            };
            format!(
                "{icon}  shuffle: {}  cola: {}   [espacio n p s a q]",
                if queue.shuffled() { "sí" } else { "no" },
                queue.queued()
            )
        }
        Mode::Typing(text) => format!("Tema para la cola: {text}▏ (Enter busca, Esc cancela)"),
        Mode::Searching => "Buscando... (Esc cancela)".to_string(),
        Mode::Choosing(hits) => select::prompt(hits.len()),
    };
    status(&text);
}

/// Reescribe la línea de estado actual. Si stdout está cerrado, no hace
/// nada (no vale la pena cortar la música por eso).
fn status(text: &str) {
    let mut out = io::stdout().lock();
    let _ = write!(out, "\r\x1b[2K{text}");
    let _ = out.flush();
}

/// Imprime una línea fija (en modo raw hace falta `\r\n` explícito, también
/// dentro de mensajes de varias líneas).
fn line(text: &str) {
    let mut out = io::stdout().lock();
    let _ = write!(out, "\r\x1b[2K{}\r\n", text.replace('\n', "\r\n"));
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> SpotifyUri {
        SpotifyUri::from_uri(&format!("spotify:track:{id:0>22}")).unwrap()
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn teclas_de_reproduccion() {
        for (code, action) in [
            (KeyCode::Char(' '), Action::TogglePause),
            (KeyCode::Char('n'), Action::Next),
            (KeyCode::Right, Action::Next),
            (KeyCode::Char('p'), Action::Previous),
            (KeyCode::Left, Action::Previous),
            (KeyCode::Char('s'), Action::ToggleShuffle),
            (KeyCode::Char('q'), Action::Quit),
            (KeyCode::Esc, Action::Quit),
            (KeyCode::Char('x'), Action::None),
        ] {
            assert_eq!(map_normal_key(code), action, "{code:?}");
        }
    }

    #[test]
    fn escribir_la_busqueda_para_encolar() {
        let web = WebClient::new(crate::spotify::auth::Token::for_tests()).unwrap();
        let mut search = None;
        let (mode, _) = on_key(press(KeyCode::Char('a')), Mode::Normal, &mut search, &web);
        let mut mode = mode;
        for c in "rick!".chars() {
            mode = on_key(press(KeyCode::Char(c)), mode, &mut search, &web).0;
        }
        mode = on_key(press(KeyCode::Backspace), mode, &mut search, &web).0;
        assert!(matches!(&mode, Mode::Typing(t) if t == "rick"));
        // q y espacio se escriben, no salen ni pausan.
        let (mode, action) = on_key(press(KeyCode::Char('q')), mode, &mut search, &web);
        assert_eq!(action, Action::None);
        let (mode, _) = on_key(press(KeyCode::Esc), mode, &mut search, &web);
        assert!(matches!(mode, Mode::Normal));
        assert!(search.is_none());

        let (mode, _) = on_key(press(KeyCode::Char('a')), Mode::Normal, &mut search, &web);
        let (mode, _) = on_key(press(KeyCode::Enter), mode, &mut search, &web);
        assert!(matches!(mode, Mode::Normal), "sin texto no busca");
        assert!(search.is_none());
    }

    #[test]
    fn enter_busca_y_esc_cancela_la_busqueda() {
        let web = WebClient::new(crate::spotify::auth::Token::for_tests()).unwrap();
        let mut search = None;
        let mode = Mode::Typing("rick".into());
        let (mode, _) = on_key(press(KeyCode::Enter), mode, &mut search, &web);
        assert!(matches!(mode, Mode::Searching));
        assert!(search.is_some());
        let (mode, _) = on_key(press(KeyCode::Esc), mode, &mut search, &web);
        assert!(matches!(mode, Mode::Normal));
        assert!(search.is_none());
    }

    #[test]
    fn elegir_un_resultado_lo_encola() {
        let web = WebClient::new(crate::spotify::auth::Token::for_tests()).unwrap();
        let hit = |id: &str| Hit {
            uri: track(id),
            name: id.into(),
            detail: String::new(),
        };
        let mut search = None;
        let mode = Mode::Choosing(vec![hit("1"), hit("2")]);
        let (mode, action) = on_key(press(KeyCode::Char('9')), mode, &mut search, &web);
        assert_eq!(action, Action::None);
        let (mode, action) = on_key(press(KeyCode::Char('2')), mode, &mut search, &web);
        assert!(matches!(mode, Mode::Normal));
        assert_eq!(action, Action::Enqueue(hit("2")));
    }

    #[test]
    fn ctrl_c_sale_en_cualquier_modo() {
        let web = WebClient::new(crate::spotify::auth::Token::for_tests()).unwrap();
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        for mode in [Mode::Normal, Mode::Typing("x".into()), Mode::Searching] {
            let (_, action) = on_key(ctrl_c, mode, &mut None, &web);
            assert_eq!(action, Action::Quit);
        }
    }
}
