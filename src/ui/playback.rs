//! Pantalla mínima de reproducción: muestra el tema y el estado, y lee
//! teclas (espacio = pausa/reanudar, n / p = siguiente / anterior,
//! s = shuffle, a = encolar, q / Esc / Ctrl+C = salir).

use std::{
    collections::VecDeque,
    future::Future,
    io::{self, Write},
    pin::Pin,
    time::{Duration, Instant},
};

use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal,
};
use futures_util::StreamExt;
use librespot_core::SpotifyUri;
use librespot_metadata::audio::UniqueFields;
use librespot_playback::player::PlayerEvent;
use rand::{Rng, seq::SliceRandom};

use super::{
    RawMode,
    select::{self, Choice},
};
use crate::{
    config,
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
    let mut queue = Queue::new(tracks, shuffle, &mut rng);
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

/// Tiempo que lleva sonando el tema actual, a partir de las posiciones que
/// informa el reproductor (sin pedirle nada ni hacer polling).
#[derive(Debug, Default)]
struct Clock {
    base: Duration,
    /// Desde cuándo suena sin pausa; `None` si está en pausa o cargando.
    since: Option<Instant>,
}

impl Clock {
    fn playing(&mut self, position_ms: u32) {
        self.base = Duration::from_millis(position_ms.into());
        self.since = Some(Instant::now());
    }

    fn paused(&mut self, position_ms: u32) {
        self.base = Duration::from_millis(position_ms.into());
        self.since = None;
    }

    fn seeked(&mut self, position_ms: u32) {
        self.base = Duration::from_millis(position_ms.into());
        if self.since.is_some() {
            self.since = Some(Instant::now());
        }
    }

    fn elapsed(&self) -> Duration {
        self.base + self.since.map_or(Duration::ZERO, |s| s.elapsed())
    }
}

/// Qué tiene que hacer el loop con el reproductor.
#[derive(Debug)]
enum Step {
    Nothing,
    Preload(SpotifyUri),
    Play(SpotifyUri),
    /// Volver al principio del tema actual.
    Restart,
    /// El tema actual no está disponible: avisar y pasar a `next`.
    Skip {
        unavailable: SpotifyUri,
        next: SpotifyUri,
    },
    Done,
    Fail(AppError),
}

/// Un tema que ya sonó o suena, con su etiqueta de posición ("[3/12] ",
/// "[cola] " o vacía).
#[derive(Debug)]
struct Entry {
    uri: SpotifyUri,
    label: String,
}

/// Estado de la cola, separado de la I/O para poder testearlo.
///
/// Qué suena después del tema actual, en orden: lo encolado (`up_next`),
/// los temas a los que se volvió con "anterior" y quedaron adelante
/// (`timeline` después de `cursor`), y el resto de la lista en `order`.
///
/// Invariantes:
/// - `order` es una permutación de `0..tracks.len()`; con shuffle apagado,
///   es la identidad. Las posiciones hasta `context_pos` ya entraron a
///   `timeline`.
/// - `up_next` nunca se mezcla.
/// - Después de `start`, `timeline` no está vacío y `cursor` apunta al tema
///   actual.
/// - `request_id` es el del último `play` pedido, o `None` entre que se pide
///   y librespot lo confirma (`PlayRequestIdChanged`); mientras es `None` se
///   ignoran los eventos, que son del tema anterior.
struct Queue<'a> {
    tracks: &'a [SpotifyUri],
    order: Vec<usize>,
    /// Posición en `order` del último tema de la lista que empezó a sonar.
    context_pos: Option<usize>,
    up_next: VecDeque<SpotifyUri>,
    /// Lo que sonó y suena, en el orden en que se escuchó.
    timeline: Vec<Entry>,
    cursor: usize,
    shuffled: bool,
    request_id: Option<u64>,
    /// El tema actual llegó a sonar (`Playing`/`Paused`). Un `Unavailable`
    /// después de eso es de la precarga del siguiente, no del actual.
    started: bool,
    /// Ya llegó `TimeToPreloadNextTrack` del tema actual: si cambia lo que
    /// sigue (shuffle, encolar), hay que precargar de nuevo.
    preload_due: bool,
    consecutive_unavailable: u32,
}

impl<'a> Queue<'a> {
    /// Con `shuffle` (y más de un tema), el orden entero sale mezclado: el
    /// primer tema también es al azar.
    fn new(tracks: &'a [SpotifyUri], shuffle: bool, rng: &mut impl Rng) -> Queue<'a> {
        let mut order: Vec<usize> = (0..tracks.len()).collect();
        let shuffled = shuffle && tracks.len() > 1;
        if shuffled {
            order.shuffle(rng);
        }
        Queue {
            tracks,
            order,
            context_pos: None,
            up_next: VecDeque::new(),
            timeline: Vec::new(),
            cursor: 0,
            shuffled,
            request_id: None,
            started: false,
            preload_due: false,
            consecutive_unavailable: 0,
        }
    }

    /// Primer tema a pedir, o `None` si la lista está vacía.
    fn start(&mut self) -> Option<SpotifyUri> {
        self.advance()
    }

    fn can_shuffle(&self) -> bool {
        self.tracks.len() > 1
    }

    fn shuffled(&self) -> bool {
        self.shuffled
    }

    /// Temas encolados que todavía no sonaron.
    fn queued(&self) -> usize {
        self.up_next.len()
    }

    /// Etiqueta del tema actual: "[3/12] ", "[cola] " o vacía.
    fn position(&self) -> &str {
        self.timeline
            .get(self.cursor)
            .map_or("", |e| e.label.as_str())
    }

    fn is_current(&self, id: u64) -> bool {
        self.request_id == Some(id)
    }

    fn on_request_id(&mut self, id: u64) {
        self.request_id = Some(id);
        self.started = false;
        self.preload_due = false;
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

    fn on_preload_time(&mut self, id: u64) -> Step {
        if !self.is_current(id) {
            return Step::Nothing;
        }
        self.preload_due = true;
        self.repreload()
    }

    /// Precarga lo que sigue si ya era momento (después de cambiar el orden
    /// o encolar); si no, nada.
    fn repreload(&self) -> Step {
        match self.peek_next() {
            Some(next) if self.preload_due => Step::Preload(next),
            _ => Step::Nothing,
        }
    }

    fn on_end(&mut self, id: u64) -> Step {
        if !self.is_current(id) {
            return Step::Nothing;
        }
        self.next()
    }

    /// Pasa al tema siguiente (tecla `n` o fin del tema).
    fn next(&mut self) -> Step {
        match self.advance() {
            Some(next) => Step::Play(next),
            None => Step::Done,
        }
    }

    /// Tecla `p`: con más de `config::PREVIOUS_RESTART_THRESHOLD` sonando, o
    /// en el primer tema, reinicia; si no, vuelve al tema que sonó antes.
    fn previous(&mut self, elapsed: Duration) -> Step {
        if elapsed > config::PREVIOUS_RESTART_THRESHOLD || self.cursor == 0 {
            return Step::Restart;
        }
        self.cursor -= 1;
        self.reset_request();
        Step::Play(self.timeline[self.cursor].uri.clone())
    }

    /// Prende o apaga el shuffle. Prenderlo mezcla lo que falta de la lista
    /// (el tema actual sigue); apagarlo retoma el orden original a partir
    /// del último tema de la lista que sonó. `None` si hay un solo tema.
    fn toggle_shuffle(&mut self, rng: &mut impl Rng) -> Option<bool> {
        if !self.can_shuffle() {
            return None;
        }
        if self.shuffled {
            let resume = self.context_pos.map(|p| self.order[p]);
            self.order = (0..self.tracks.len()).collect();
            self.context_pos = resume;
        } else {
            let from = self.next_context_pos();
            self.order[from..].shuffle(rng);
        }
        self.shuffled = !self.shuffled;
        Some(self.shuffled)
    }

    /// Encola `uri`: suena después del actual y de lo ya encolado.
    fn enqueue(&mut self, uri: SpotifyUri) {
        self.up_next.push_back(uri);
    }

    fn on_unavailable(&mut self, id: u64, session_lost: bool) -> Step {
        if !self.is_current(id) || self.started {
            return Step::Nothing;
        }
        let unavailable = self.timeline[self.cursor].uri.clone();
        if session_lost {
            return Step::Fail(AppError::Network(
                "se perdió la conexión con Spotify".into(),
            ));
        }
        if self.timeline.len() == 1 && self.peek_next().is_none() {
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

    fn next_context_pos(&self) -> usize {
        self.context_pos.map_or(0, |p| p + 1)
    }

    /// Lo que sonaría con `next`, sin moverse.
    fn peek_next(&self) -> Option<SpotifyUri> {
        if let Some(uri) = self.up_next.front() {
            return Some(uri.clone());
        }
        if let Some(entry) = self.timeline.get(self.cursor + 1) {
            return Some(entry.uri.clone());
        }
        let i = *self.order.get(self.next_context_pos())?;
        Some(self.tracks[i].clone())
    }

    /// Pasa al tema siguiente; los eventos se ignoran hasta que librespot
    /// confirme el pedido nuevo.
    fn advance(&mut self) -> Option<SpotifyUri> {
        let at = if self.timeline.is_empty() {
            0
        } else {
            self.cursor + 1
        };
        if let Some(uri) = self.up_next.pop_front() {
            let entry = Entry {
                uri,
                label: "[cola] ".into(),
            };
            self.timeline.insert(at, entry);
        } else if at >= self.timeline.len() {
            let pos = self.next_context_pos();
            let i = *self.order.get(pos)?;
            self.context_pos = Some(pos);
            let label = if self.tracks.len() > 1 {
                format!("[{}/{}] ", pos + 1, self.tracks.len())
            } else {
                String::new()
            };
            self.timeline.push(Entry {
                uri: self.tracks[i].clone(),
                label,
            });
        }
        self.cursor = at;
        self.reset_request();
        Some(self.timeline[at].uri.clone())
    }

    fn reset_request(&mut self) {
        self.request_id = None;
        self.started = false;
        self.preload_due = false;
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

/// Imprime una línea fija (en modo raw hace falta `\r\n` explícito, también
/// dentro de mensajes de varias líneas).
fn line(text: &str) {
    let mut out = io::stdout().lock();
    let _ = write!(out, "\r\x1b[2K{}\r\n", text.replace('\n', "\r\n"));
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;

    fn tracks(n: usize) -> Vec<SpotifyUri> {
        (0..n).map(|i| track(&i.to_string())).collect()
    }

    fn track(id: &str) -> SpotifyUri {
        SpotifyUri::from_uri(&format!("spotify:track:{id:0>22}")).unwrap()
    }

    fn rng() -> StdRng {
        StdRng::seed_from_u64(7)
    }

    fn is_play(step: &Step, expected: &SpotifyUri) -> bool {
        matches!(step, Step::Play(uri) if uri == expected)
    }

    fn played(step: Step) -> SpotifyUri {
        match step {
            Step::Play(uri) => uri,
            other => panic!("se esperaba Play, vino {other:?}"),
        }
    }

    /// Una cola en orden, con el primer tema sonando (request id 1).
    fn started(list: &[SpotifyUri]) -> Queue<'_> {
        let mut q = Queue::new(list, false, &mut rng());
        q.start();
        q.on_request_id(1);
        q.on_started(1);
        q
    }

    const SHORT: Duration = Duration::from_secs(1);
    const LONG: Duration = Duration::from_secs(10);

    #[test]
    fn fin_de_tema_repetido_no_saltea_de_mas() {
        let list = tracks(3);
        let mut q = started(&list);
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
        let mut q = Queue::new(&list, false, &mut rng());
        q.start();
        assert!(matches!(q.on_end(0), Step::Nothing));
        assert!(!q.on_started(0));
        assert!(matches!(q.on_preload_time(0), Step::Nothing));
    }

    #[test]
    fn precarga_fallida_no_corta_el_tema_actual() {
        let list = tracks(3);
        let mut q = started(&list);
        assert!(matches!(q.on_preload_time(1), Step::Preload(uri) if uri == list[1]));
        // La precarga falla: librespot manda Unavailable con el id actual.
        assert!(matches!(q.on_unavailable(1, false), Step::Nothing));
        assert!(is_play(&q.on_end(1), &list[1]));
    }

    #[test]
    fn tema_no_disponible_se_saltea_en_lista() {
        let list = tracks(3);
        let mut q = Queue::new(&list, false, &mut rng());
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
        let mut q = Queue::new(&list, false, &mut rng());
        q.start();
        q.on_request_id(1);
        assert!(matches!(
            q.on_unavailable(1, false),
            Step::Fail(AppError::TrackUnavailable(_))
        ));
    }

    #[test]
    fn tema_unico_no_disponible_con_cola_sigue_con_lo_encolado() {
        let list = tracks(1);
        let mut q = Queue::new(&list, false, &mut rng());
        q.start();
        q.on_request_id(1);
        q.enqueue(track("99"));
        assert!(
            matches!(q.on_unavailable(1, false), Step::Skip { next, .. } if next == track("99"))
        );
    }

    #[test]
    fn sesion_caida_corta_la_lista() {
        let list = tracks(5);
        let mut q = Queue::new(&list, false, &mut rng());
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
        let mut q = Queue::new(&list, false, &mut rng());
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
        let mut q = Queue::new(&list, false, &mut rng());
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
        let mut q = started(&list);
        assert!(matches!(q.on_preload_time(1), Step::Nothing));
        assert!(matches!(q.on_end(1), Step::Done));
    }

    #[test]
    fn siguiente_y_en_el_ultimo_termina() {
        let list = tracks(2);
        let mut q = started(&list);
        assert!(is_play(&q.next(), &list[1]));
        assert!(matches!(q.next(), Step::Done));
    }

    #[test]
    fn siguiente_varias_veces_rapido_ignora_eventos_viejos() {
        let list = tracks(5);
        let mut q = started(&list);
        assert!(is_play(&q.next(), &list[1]));
        assert!(is_play(&q.next(), &list[2]));
        // Eventos del tema 0 y del 1 (nunca confirmados) llegan tarde.
        assert!(matches!(q.on_end(1), Step::Nothing));
        assert!(!q.on_started(1));
        q.on_request_id(3);
        assert!(q.on_started(3));
        assert_eq!(q.position(), "[3/5] ");
        assert!(is_play(&q.on_end(3), &list[3]));
    }

    #[test]
    fn anterior_segun_el_tiempo_sonando() {
        let list = tracks(3);
        let mut q = started(&list);
        // En el primer tema siempre reinicia.
        assert!(matches!(q.previous(SHORT), Step::Restart));
        q.next();
        assert!(matches!(q.previous(LONG), Step::Restart));
        assert!(is_play(&q.previous(SHORT), &list[0]));
        // Después de volver, "siguiente" retoma donde estaba.
        assert!(is_play(&q.next(), &list[1]));
        assert!(is_play(&q.next(), &list[2]));
    }

    #[test]
    fn anterior_con_shuffle_vuelve_a_lo_que_sono() {
        let list = tracks(10);
        let mut q = started(&list);
        q.toggle_shuffle(&mut rng());
        let a = played(q.next());
        let b = played(q.next());
        assert!(is_play(&q.previous(SHORT), &a));
        assert!(is_play(&q.previous(SHORT), &list[0]));
        assert!(is_play(&q.next(), &a));
        assert!(is_play(&q.next(), &b));
    }

    /// Lo que suena desde el actual hasta el final, pasando con `next`.
    fn rest(q: &mut Queue) -> Vec<SpotifyUri> {
        let mut out = Vec::new();
        while let Step::Play(uri) = q.next() {
            out.push(uri);
        }
        out
    }

    #[test]
    fn shuffle_mezcla_lo_que_falta_sin_repetir_ni_saltear() {
        let list = tracks(20);
        let mut q = started(&list);
        q.next();
        q.next(); // suena list[2]
        assert_eq!(q.toggle_shuffle(&mut rng()), Some(true));
        assert_eq!(q.position(), "[3/20] ");
        let mut after = rest(&mut q);
        assert_ne!(after, list[3..], "con 17 temas, no debería quedar en orden");
        after.sort_by_key(|u| u.to_uri().unwrap());
        assert_eq!(after, list[3..]);
    }

    #[test]
    fn apagar_shuffle_retoma_el_orden_original() {
        let list = tracks(20);
        let mut q = started(&list);
        q.toggle_shuffle(&mut rng());
        let current = played(q.next());
        assert_eq!(q.toggle_shuffle(&mut rng()), Some(false));
        let i = list.iter().position(|u| *u == current).unwrap();
        let expected = list.get(i + 1).cloned();
        match q.next() {
            Step::Play(uri) => assert_eq!(Some(uri), expected),
            Step::Done => assert_eq!(expected, None),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn arrancar_mezclado_usa_toda_la_lista() {
        let list = tracks(20);
        let mut seen = Vec::new();
        let mut q = Queue::new(&list, true, &mut rng());
        seen.push(q.start().unwrap());
        seen.extend(rest(&mut q));
        assert!(q.shuffled());
        assert_ne!(seen, list);
        seen.sort_by_key(|u| u.to_uri().unwrap());
        assert_eq!(seen, list);
    }

    #[test]
    fn un_solo_tema_no_se_mezcla() {
        let list = tracks(1);
        let mut q = Queue::new(&list, true, &mut rng());
        assert!(!q.shuffled());
        assert!(!q.can_shuffle());
        assert_eq!(q.toggle_shuffle(&mut rng()), None);
    }

    #[test]
    fn lo_encolado_suena_despues_del_actual_en_orden() {
        let list = tracks(3);
        let mut q = started(&list);
        q.enqueue(track("91"));
        q.enqueue(track("92"));
        assert_eq!(q.queued(), 2);
        assert!(is_play(&q.next(), &track("91")));
        assert_eq!(q.position(), "[cola] ");
        assert!(is_play(&q.next(), &track("92")));
        assert!(is_play(&q.next(), &list[1]));
        assert_eq!(q.queued(), 0);
    }

    #[test]
    fn el_shuffle_no_mezcla_lo_encolado() {
        let list = tracks(10);
        let mut q = started(&list);
        q.enqueue(track("91"));
        q.enqueue(track("92"));
        q.toggle_shuffle(&mut rng());
        assert!(is_play(&q.next(), &track("91")));
        assert!(is_play(&q.next(), &track("92")));
    }

    #[test]
    fn tema_unico_con_encolados_no_termina() {
        let list = tracks(1);
        let mut q = started(&list);
        q.enqueue(track("91"));
        assert!(is_play(&q.on_end(1), &track("91")));
        q.on_request_id(2);
        assert!(matches!(q.on_end(2), Step::Done));
    }

    #[test]
    fn encolar_despues_de_volver_suena_despues_del_actual() {
        let list = tracks(3);
        let mut q = started(&list);
        q.next();
        q.previous(SHORT); // vuelve a list[0]; list[1] queda adelante
        q.enqueue(track("91"));
        assert!(is_play(&q.next(), &track("91")));
        assert!(is_play(&q.next(), &list[1]));
    }

    #[test]
    fn encolar_o_mezclar_despues_del_aviso_de_precarga_precarga_de_nuevo() {
        let list = tracks(3);
        let mut q = started(&list);
        assert!(matches!(q.repreload(), Step::Nothing));
        assert!(matches!(q.on_preload_time(1), Step::Preload(u) if u == list[1]));
        q.enqueue(track("91"));
        assert!(matches!(q.repreload(), Step::Preload(u) if u == track("91")));
        // Con el tema nuevo, hasta su propio aviso no se precarga nada.
        q.next();
        q.on_request_id(2);
        assert!(matches!(q.repreload(), Step::Nothing));
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

    #[test]
    fn reloj_del_tema() {
        let mut clock = Clock::default();
        assert_eq!(clock.elapsed(), Duration::ZERO);
        clock.paused(5_000);
        assert_eq!(clock.elapsed(), Duration::from_secs(5));
        clock.seeked(0);
        assert_eq!(clock.elapsed(), Duration::ZERO);
        clock.playing(2_000);
        assert!(clock.elapsed() >= Duration::from_secs(2));
    }
}
