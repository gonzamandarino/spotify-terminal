//! Pantalla mínima de reproducción: muestra el tema y el estado, y lee
//! teclas (espacio = pausa/reanudar, q / Esc / Ctrl+C = salir).

use std::io::{self, Write};

use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal,
};
use futures_util::StreamExt;
use librespot_core::SpotifyUri;
use librespot_metadata::audio::UniqueFields;
use librespot_playback::player::PlayerEvent;

use super::RawMode;
use crate::{config, error::AppError, spotify::player::Player};

/// Reproduce `tracks` en orden y atiende el teclado hasta que termina el
/// último tema o el usuario sale.
///
/// - Pre: ninguna; con `tracks` vacío no hace nada y devuelve `Ok`.
/// - Post: la terminal vuelve a modo normal siempre (incluso con error).
///   Mientras suena un tema se precarga el siguiente, para que no haya
///   silencio entre temas. Solo cuentan los eventos del tema que se pidió
///   último (`play_request_id`): los repetidos o de la precarga se ignoran.
/// - Errores: con un solo tema, `TrackUnavailable` si no se puede
///   reproducir; en una lista, el tema no disponible se saltea con aviso,
///   salvo que se haya caído la sesión o fallen
///   `config::MAX_CONSECUTIVE_UNAVAILABLE` seguidos → `Network`. Si el
///   reproductor se cierra solo → `PlayerStopped`.
pub async fn play_queue(player: &Player, tracks: &[SpotifyUri]) -> Result<(), AppError> {
    let mut queue = Queue::new(tracks);
    let Some(first) = queue.start() else {
        return Ok(());
    };
    let mut events = player.events();
    let _raw = RawMode::enable()?;
    let mut keys = EventStream::new();

    player.play(first);
    let mut paused = false;
    status("Cargando...");

    loop {
        tokio::select! {
            event = events.recv() => {
                let Some(event) = event else {
                    return Err(AppError::PlayerStopped);
                };
                let step = match event {
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
                        line(&format!("♪ {}{} — {}", queue.position(), audio_item.name, artists));
                        Step::Nothing
                    }
                    PlayerEvent::Playing { play_request_id, .. } => {
                        if queue.on_started(play_request_id) {
                            paused = false;
                            status("▶ Reproduciendo   [espacio] pausa  [q] salir");
                        }
                        Step::Nothing
                    }
                    PlayerEvent::Paused { play_request_id, .. } => {
                        if queue.on_started(play_request_id) {
                            paused = true;
                            status("⏸ En pausa        [espacio] seguir [q] salir");
                        }
                        Step::Nothing
                    }
                    PlayerEvent::TimeToPreloadNextTrack { play_request_id, .. } => {
                        queue.on_preload_time(play_request_id)
                    }
                    PlayerEvent::EndOfTrack { play_request_id, .. } => {
                        queue.on_end(play_request_id)
                    }
                    PlayerEvent::Unavailable { play_request_id, .. } => {
                        queue.on_unavailable(play_request_id, player.session_lost())
                    }
                    _ => Step::Nothing,
                };
                match step {
                    Step::Nothing => {}
                    Step::Preload(next) => player.preload(next),
                    Step::Play(next) => player.play(next),
                    Step::Skip { unavailable, next } => {
                        line(&format!("⚠ {} no está disponible, sigo con el próximo.", uri_text(&unavailable)));
                        player.play(next);
                    }
                    Step::Done => break,
                    Step::Fail(e) => {
                        player.stop();
                        return Err(e);
                    }
                }
            }
            key = keys.next() => match key {
                Some(Ok(Event::Key(key))) => match map_key(key) {
                    Some(Key::TogglePause) if paused => player.resume(),
                    Some(Key::TogglePause) => player.pause(),
                    Some(Key::Quit) => {
                        player.stop();
                        break;
                    }
                    None => {}
                },
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    player.stop();
                    return Err(AppError::Terminal(e));
                }
                None => {
                    player.stop();
                    return Err(AppError::Terminal(io::Error::other("se cerró la entrada del teclado")));
                }
            },
        }
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

enum Key {
    TogglePause,
    Quit,
}

fn map_key(key: KeyEvent) -> Option<Key> {
    // En Windows llegan también los eventos de soltar la tecla.
    if key.kind != KeyEventKind::Press {
        return None;
    }
    match key.code {
        KeyCode::Char(' ') => Some(Key::TogglePause),
        KeyCode::Char('q') | KeyCode::Esc => Some(Key::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Key::Quit),
        _ => None,
    }
}

/// Qué tiene que hacer el loop con el reproductor después de un evento.
#[derive(Debug)]
enum Step {
    Nothing,
    Preload(SpotifyUri),
    Play(SpotifyUri),
    /// El tema actual no está disponible: avisar y pasar a `next`.
    Skip {
        unavailable: SpotifyUri,
        next: SpotifyUri,
    },
    Done,
    Fail(AppError),
}

/// Estado de la cola, separado de la I/O para poder testearlo.
///
/// Invariante: `request_id` es el del último `play` pedido, o `None` entre
/// que se pide y librespot lo confirma (`PlayRequestIdChanged`); mientras es
/// `None` se ignoran los eventos, que son del tema anterior.
struct Queue<'a> {
    tracks: &'a [SpotifyUri],
    current: usize,
    request_id: Option<u64>,
    /// El tema actual llegó a sonar (`Playing`/`Paused`). Un `Unavailable`
    /// después de eso es de la precarga del siguiente, no del actual.
    started: bool,
    consecutive_unavailable: u32,
}

impl<'a> Queue<'a> {
    fn new(tracks: &'a [SpotifyUri]) -> Queue<'a> {
        Queue {
            tracks,
            current: 0,
            request_id: None,
            started: false,
            consecutive_unavailable: 0,
        }
    }

    /// Primer tema a pedir, o `None` si la lista está vacía.
    fn start(&mut self) -> Option<SpotifyUri> {
        self.tracks.first().cloned()
    }

    /// "[3/12] " en listas, vacío con un solo tema.
    fn position(&self) -> String {
        if self.tracks.len() > 1 {
            format!("[{}/{}] ", self.current + 1, self.tracks.len())
        } else {
            String::new()
        }
    }

    fn is_current(&self, id: u64) -> bool {
        self.request_id == Some(id)
    }

    fn on_request_id(&mut self, id: u64) {
        self.request_id = Some(id);
        self.started = false;
    }

    /// Devuelve `true` si el evento es del tema actual.
    fn on_started(&mut self, id: u64) -> bool {
        if !self.is_current(id) {
            return false;
        }
        self.started = true;
        self.consecutive_unavailable = 0;
        true
    }

    fn on_preload_time(&self, id: u64) -> Step {
        match self.tracks.get(self.current + 1) {
            Some(next) if self.is_current(id) => Step::Preload(next.clone()),
            _ => Step::Nothing,
        }
    }

    fn on_end(&mut self, id: u64) -> Step {
        if !self.is_current(id) {
            return Step::Nothing;
        }
        match self.advance() {
            Some(next) => Step::Play(next),
            None => Step::Done,
        }
    }

    fn on_unavailable(&mut self, id: u64, session_lost: bool) -> Step {
        if !self.is_current(id) || self.started {
            return Step::Nothing;
        }
        let unavailable = self.tracks[self.current].clone();
        if session_lost {
            return Step::Fail(AppError::Network(
                "se perdió la conexión con Spotify".into(),
            ));
        }
        if self.tracks.len() == 1 {
            return Step::Fail(AppError::TrackUnavailable(uri_text(&unavailable)));
        }
        self.consecutive_unavailable += 1;
        if self.consecutive_unavailable >= config::MAX_CONSECUTIVE_UNAVAILABLE {
            return Step::Fail(AppError::Network(format!(
                "fallaron {} temas seguidos",
                self.consecutive_unavailable
            )));
        }
        match self.advance() {
            Some(next) => Step::Skip { unavailable, next },
            None => Step::Done,
        }
    }

    /// Pasa al tema siguiente; los eventos se ignoran hasta que librespot
    /// confirme el pedido nuevo.
    fn advance(&mut self) -> Option<SpotifyUri> {
        self.current += 1;
        self.request_id = None;
        self.started = false;
        self.tracks.get(self.current).cloned()
    }
}

fn uri_text(uri: &SpotifyUri) -> String {
    uri.to_uri().unwrap_or_else(|_| "(sin URI)".to_string())
}

/// Reescribe la línea de estado actual. Si stdout está cerrado, no hace
/// nada (no vale la pena cortar la música por eso).
fn status(text: &str) {
    let mut out = io::stdout().lock();
    let _ = write!(out, "\r\x1b[2K{text}");
    let _ = out.flush();
}

/// Imprime una línea fija (en modo raw hace falta `\r\n` explícito).
fn line(text: &str) {
    let mut out = io::stdout().lock();
    let _ = write!(out, "\r\x1b[2K{text}\r\n");
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracks(n: usize) -> Vec<SpotifyUri> {
        (0..n)
            .map(|i| SpotifyUri::from_uri(&format!("spotify:track:{i:0>22}")).unwrap())
            .collect()
    }

    fn is_play(step: &Step, expected: &SpotifyUri) -> bool {
        matches!(step, Step::Play(uri) if uri == expected)
    }

    #[test]
    fn fin_de_tema_repetido_no_saltea_de_mas() {
        let list = tracks(3);
        let mut q = Queue::new(&list);
        q.start();
        q.on_request_id(1);
        q.on_started(1);
        assert!(is_play(&q.on_end(1), &list[1]));
        // librespot repite EndOfTrack si falla la decodificación.
        assert!(matches!(q.on_end(1), Step::Nothing));
        q.on_request_id(2);
        assert!(matches!(q.on_end(1), Step::Nothing));
        assert!(is_play(&q.on_end(2), &list[2]));
    }

    #[test]
    fn eventos_antes_de_confirmar_el_pedido_se_ignoran() {
        let list = tracks(2);
        let mut q = Queue::new(&list);
        q.start();
        assert!(matches!(q.on_end(0), Step::Nothing));
        assert!(!q.on_started(0));
        assert!(matches!(q.on_preload_time(0), Step::Nothing));
    }

    #[test]
    fn precarga_fallida_no_corta_el_tema_actual() {
        let list = tracks(3);
        let mut q = Queue::new(&list);
        q.start();
        q.on_request_id(1);
        q.on_started(1);
        assert!(matches!(q.on_preload_time(1), Step::Preload(uri) if uri == list[1]));
        // La precarga falla: librespot manda Unavailable con el id actual.
        assert!(matches!(q.on_unavailable(1, false), Step::Nothing));
        assert!(is_play(&q.on_end(1), &list[1]));
    }

    #[test]
    fn tema_no_disponible_se_saltea_en_lista() {
        let list = tracks(3);
        let mut q = Queue::new(&list);
        q.start();
        q.on_request_id(1);
        let step = q.on_unavailable(1, false);
        assert!(
            matches!(&step, Step::Skip { unavailable, next } if *unavailable == list[0] && *next == list[1])
        );
    }

    #[test]
    fn tema_unico_no_disponible_es_error() {
        let list = tracks(1);
        let mut q = Queue::new(&list);
        q.start();
        q.on_request_id(1);
        assert!(matches!(
            q.on_unavailable(1, false),
            Step::Fail(AppError::TrackUnavailable(_))
        ));
    }

    #[test]
    fn sesion_caida_corta_la_lista() {
        let list = tracks(5);
        let mut q = Queue::new(&list);
        q.start();
        q.on_request_id(1);
        assert!(matches!(
            q.on_unavailable(1, true),
            Step::Fail(AppError::Network(_))
        ));
    }

    #[test]
    fn muchos_no_disponibles_seguidos_cortan_la_lista() {
        let list = tracks(10);
        let mut q = Queue::new(&list);
        q.start();
        let mut id = 0;
        for _ in 1..config::MAX_CONSECUTIVE_UNAVAILABLE {
            id += 1;
            q.on_request_id(id);
            assert!(matches!(q.on_unavailable(id, false), Step::Skip { .. }));
        }
        id += 1;
        q.on_request_id(id);
        assert!(matches!(
            q.on_unavailable(id, false),
            Step::Fail(AppError::Network(_))
        ));
    }

    #[test]
    fn un_tema_que_suena_reinicia_el_contador() {
        let list = tracks(10);
        let mut q = Queue::new(&list);
        q.start();
        let mut id = 0;
        for _ in 0..3 {
            for _ in 1..config::MAX_CONSECUTIVE_UNAVAILABLE {
                id += 1;
                q.on_request_id(id);
                assert!(matches!(q.on_unavailable(id, false), Step::Skip { .. }));
            }
            id += 1;
            q.on_request_id(id);
            q.on_started(id);
            q.on_end(id);
        }
    }

    #[test]
    fn ultimo_tema_termina_la_cola() {
        let list = tracks(1);
        let mut q = Queue::new(&list);
        q.start();
        q.on_request_id(7);
        q.on_started(7);
        assert!(matches!(q.on_preload_time(7), Step::Nothing));
        assert!(matches!(q.on_end(7), Step::Done));
    }

    #[test]
    fn teclas() {
        let key = |code, modifiers| KeyEvent::new(code, modifiers);
        assert!(matches!(
            map_key(key(KeyCode::Char(' '), KeyModifiers::NONE)),
            Some(Key::TogglePause)
        ));
        assert!(matches!(
            map_key(key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Key::Quit)
        ));
        assert!(map_key(key(KeyCode::Char('c'), KeyModifiers::NONE)).is_none());
        let mut release = key(KeyCode::Char('q'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert!(map_key(release).is_none());
    }
}
