//! Motor de la app de escritorio: recibe lo que se escribe en la consola,
//! maneja el reproductor, las búsquedas y la cola, y avisa qué mostrar.
//!
//! Corre en su propio hilo ("motor", runtime tokio de un hilo), separado de
//! la ventana y de la sesión de audio: un redibujo lento no demora al
//! reproductor, y una búsqueda o un login no congelan la ventana. Se habla
//! con él solo por canales ([`Input`] → motor → [`Output`]).

use std::{
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::mpsc as std_mpsc,
    thread::{self, JoinHandle},
    time::Duration,
};

use librespot_core::SpotifyUri;
use librespot_playback::player::{PlayerEvent, PlayerEventChannel};
use rand::{SeedableRng, rngs::StdRng};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use super::{
    backend::{Backend, Playback, SpotifyBackend},
    queue::{Clock, PlayState, Queue, Step, on_player_event, uri_text},
    shell::{self, ShellCommand},
};
use crate::{
    error::AppError,
    spotify::{
        player::Resolved,
        web::{Hit, SearchKind, User},
    },
    ui::{cli::Command, select},
};

/// Lo que la ventana le manda al motor.
#[derive(Debug, PartialEq)]
pub(crate) enum Input {
    /// Una línea escrita en la consola (Enter).
    Line(String),
    /// Atajos de teclado: igual que `pause`, `next` y `prev`.
    TogglePause,
    Next,
    Prev,
    /// Esc: cancela la elección de un resultado o la tarea en curso.
    Cancel,
}

/// Tipo de línea, para elegir el color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LineKind {
    Normal,
    /// Cambio de tema.
    Track,
    Ok,
    Warn,
    Error,
    /// Mensajes secundarios (cancelado, instrucciones).
    Dim,
}

/// Qué suena, para la barra de abajo.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NowPlaying {
    /// Vacío hasta que llega el primer `TrackChanged`.
    pub(crate) title: String,
    pub(crate) artists: String,
    /// "[3/12] ", "[cola] " o vacío.
    pub(crate) position: String,
    pub(crate) duration: Duration,
    /// Tiempo sonando (la ventana lo calcula al dibujar, sin pedírselo al
    /// motor).
    pub(crate) clock: Clock,
    pub(crate) state: PlayState,
    pub(crate) shuffle: bool,
    pub(crate) queued: usize,
}

/// Qué espera la línea de entrada.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Prompt {
    Ready,
    /// Elegir entre N resultados.
    Choose(usize),
    /// Hay una tarea en curso (texto para mostrar); se puede escribir igual.
    Busy(&'static str),
}

/// Lo que el motor le manda a la ventana.
#[derive(Debug, PartialEq)]
pub(crate) enum Output {
    Line(LineKind, String),
    /// `None` = no suena nada.
    NowPlaying(Option<NowPlaying>),
    Prompt(Prompt),
    Clear,
    /// `exit`: la ventana tiene que cerrarse.
    Exit,
}

/// Salida del motor: cada mensaje despierta a la ventana con `wake` para
/// que se redibuje, sin que ella tenga que consultar.
pub(crate) struct Outbox {
    tx: std_mpsc::Sender<Output>,
    wake: Box<dyn Fn() + Send>,
}

impl Outbox {
    fn send(&self, output: Output) {
        // Si la ventana ya se cerró no hay a quién avisar.
        if self.tx.send(output).is_ok() {
            (self.wake)();
        }
    }

    fn line(&self, kind: LineKind, text: impl Into<String>) {
        self.send(Output::Line(kind, text.into()));
    }

    fn error(&self, error: &AppError) {
        self.line(LineKind::Error, format!("Error: {error}"));
    }
}

/// Motor corriendo en su hilo.
pub(crate) struct EngineHandle {
    pub(crate) inputs: UnboundedSender<Input>,
    pub(crate) outputs: std_mpsc::Receiver<Output>,
    pub(crate) thread: JoinHandle<()>,
}

/// Arranca el motor con Spotify de verdad en el hilo "motor".
///
/// - Post: el motor atiende `inputs` hasta `exit` o hasta que se suelta el
///   emisor (la ventana se cerró); en los dos casos corta el audio y suelta
///   el reproductor antes de terminar el hilo. `wake` se llama después de
///   cada `Output`, desde el hilo del motor.
/// - Errores: `Internal` si el hilo no arranca. Si no arranca el runtime,
///   el error llega como `Output::Line` y el hilo termina.
pub(crate) fn spawn(wake: impl Fn() + Send + 'static) -> Result<EngineHandle, AppError> {
    let (in_tx, in_rx) = mpsc::unbounded_channel();
    let (out_tx, out_rx) = std_mpsc::channel();
    let thread = thread::Builder::new()
        .name("motor".into())
        .spawn(move || {
            let out = Outbox {
                tx: out_tx,
                wake: Box::new(wake),
            };
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(e) => {
                    let error = AppError::Internal(format!("runtime del motor: {e}"));
                    out.send(Output::Line(LineKind::Error, error.to_string()));
                    return;
                }
            };
            runtime.block_on(Engine::new(SpotifyBackend, out).run(in_rx));
            // Un login esperando al navegador no tiene que trabar el cierre.
            runtime.shutdown_background();
        })
        .map_err(|e| AppError::Internal(format!("hilo del motor: {e}")))?;
    Ok(EngineHandle {
        inputs: in_tx,
        outputs: out_rx,
        thread,
    })
}

const NOTHING_PLAYING: &str = "⚠ No suena nada. Probá con `play <nombre>`.";
const NOTHING_TO_SHUFFLE: &str = "⚠ Hay un solo tema: no hay nada que mezclar.";

/// Para qué se buscó.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Purpose {
    Play { shuffle: bool },
    Enqueue,
}

/// Resultado de una tarea de fondo.
enum Done<P> {
    Login(Result<(), AppError>),
    Whoami(Result<User, AppError>),
    Search {
        purpose: Purpose,
        kind: SearchKind,
        query: String,
        result: Result<Vec<Hit>, AppError>,
    },
    /// Lista de temas lista para sonar. `new_player` viene si hubo que
    /// conectar (no había reproductor o se había caído la sesión).
    Prepared {
        shuffle: bool,
        new_player: Option<P>,
        result: Result<Resolved, AppError>,
    },
}

type Task<P> = Pin<Box<dyn Future<Output = Done<P>>>>;

/// Lo que está sonando.
struct Playing {
    queue: Queue,
    state: PlayState,
    clock: Clock,
    title: String,
    artists: String,
    duration: Duration,
}

/// Estado del motor.
///
/// Invariantes:
/// - `events` es `Some` si y solo si `player` es `Some`, y es el canal de
///   ese reproductor.
/// - `playing` es `Some` solo con `player` `Some`.
/// - A lo sumo una tarea de fondo (`task`) a la vez; los controles de
///   reproducción no esperan a la tarea.
pub(crate) struct Engine<B: Backend> {
    backend: Rc<B>,
    player: Option<Rc<B::Player>>,
    events: Option<PlayerEventChannel>,
    playing: Option<Playing>,
    choosing: Option<(Vec<Hit>, Purpose)>,
    task: Option<Task<B::Player>>,
    busy: &'static str,
    rng: StdRng,
    out: Outbox,
    shown: Option<NowPlaying>,
    shown_prompt: Prompt,
}

impl<B: Backend + 'static> Engine<B> {
    fn new(backend: B, out: Outbox) -> Engine<B> {
        Engine {
            backend: Rc::new(backend),
            player: None,
            events: None,
            playing: None,
            choosing: None,
            task: None,
            busy: "",
            rng: StdRng::from_rng(&mut rand::rng()),
            out,
            shown: None,
            shown_prompt: Prompt::Ready,
        }
    }

    /// Atiende comandos, eventos del reproductor y tareas hasta `exit` o
    /// hasta que se cierra `inputs`. Al salir corta el audio.
    async fn run(mut self, mut inputs: UnboundedReceiver<Input>) {
        loop {
            tokio::select! {
                input = inputs.recv() => {
                    let Some(input) = input else { break };
                    if self.on_input(input) == Flow::Exit {
                        break;
                    }
                }
                event = next_event(&mut self.events) => self.on_event(event),
                done = finish(&mut self.task) => {
                    self.task = None;
                    self.on_done(done);
                }
            }
            self.publish();
        }
        self.stop_playing();
    }

    fn on_input(&mut self, input: Input) -> Flow {
        match input {
            Input::Line(text) => return self.on_line(&text),
            Input::TogglePause => self.control(Control::Pause),
            Input::Next => self.control(Control::Next),
            Input::Prev => self.control(Control::Prev),
            Input::Cancel => {
                if self.choosing.take().is_some() {
                    self.out.line(LineKind::Dim, "Elección cancelada.");
                } else if self.task.take().is_some() {
                    self.out.line(LineKind::Dim, "Cancelado.");
                }
            }
        }
        Flow::Continue
    }

    fn on_line(&mut self, text: &str) -> Flow {
        if let Some((hits, purpose)) = self.choosing.take() {
            let text = text.trim();
            if text.is_empty() || text.chars().all(|c| c.is_ascii_digit()) {
                let pick = if text.is_empty() {
                    Some(0)
                } else {
                    text.parse::<usize>()
                        .ok()
                        .filter(|n| (1..=hits.len()).contains(n))
                        .map(|n| n - 1)
                };
                match pick.and_then(|i| hits.get(i).cloned()) {
                    Some(hit) => self.picked(hit, purpose),
                    None => {
                        self.out.line(
                            LineKind::Warn,
                            format!("⚠ Elegí un número del 1 al {}.", hits.len()),
                        );
                        self.choosing = Some((hits, purpose));
                    }
                }
                return Flow::Continue;
            }
            // Escribir otro comando cancela la elección.
            self.out.line(LineKind::Dim, "Elección cancelada.");
        }
        match shell::parse_line(text) {
            Ok(command) => self.run_command(command),
            Err(reason) => {
                self.out.line(
                    LineKind::Error,
                    format!("{reason}. Escribí `help` para ver los comandos."),
                );
                Flow::Continue
            }
        }
    }

    fn run_command(&mut self, command: ShellCommand) -> Flow {
        match command {
            ShellCommand::Empty => {}
            ShellCommand::Help | ShellCommand::Cli(Command::Help) => {
                self.out.line(LineKind::Normal, shell::HELP);
            }
            ShellCommand::Pause => self.control(Control::Pause),
            ShellCommand::Next => self.control(Control::Next),
            ShellCommand::Prev => self.control(Control::Prev),
            ShellCommand::Shuffle => self.control(Control::Shuffle),
            ShellCommand::Queue(query) => {
                if self.playing.is_none() {
                    self.out.line(LineKind::Warn, NOTHING_PLAYING);
                } else {
                    self.search(SearchKind::Track, query, Purpose::Enqueue);
                }
            }
            ShellCommand::Stop => {
                if self.playing.is_some() {
                    self.stop_playing();
                    self.out.line(LineKind::Dim, "⏹ Detenido.");
                } else {
                    self.out.line(LineKind::Warn, NOTHING_PLAYING);
                }
            }
            ShellCommand::Clear => self.out.send(Output::Clear),
            ShellCommand::Exit => {
                self.stop_playing();
                self.out.send(Output::Exit);
                return Flow::Exit;
            }
            ShellCommand::Cli(Command::Login) => {
                let backend = self.backend.clone();
                if self.start_task(
                    "esperando la autorización en el navegador",
                    Box::pin(async move { Done::Login(backend.login().await) }),
                ) {
                    self.out.line(
                        LineKind::Dim,
                        "Iniciando sesión… si hace falta, se abre el navegador para autorizar.",
                    );
                }
            }
            ShellCommand::Cli(Command::Logout) => {
                // La sesión de audio es de la cuenta que se va.
                self.stop_playing();
                self.player = None;
                self.events = None;
                match self.backend.logout() {
                    Ok(true) => self.out.line(LineKind::Ok, "Sesión cerrada."),
                    Ok(false) => self
                        .out
                        .line(LineKind::Normal, "No había una sesión guardada."),
                    Err(e) => self.out.error(&e),
                }
            }
            ShellCommand::Cli(Command::Whoami) => {
                let backend = self.backend.clone();
                self.start_task(
                    "pidiendo el usuario",
                    Box::pin(async move { Done::Whoami(backend.current_user().await) }),
                );
            }
            ShellCommand::Cli(Command::Play { target, shuffle }) => self.prepare(target, shuffle),
            ShellCommand::Cli(Command::Search {
                kind,
                query,
                shuffle,
            }) => self.search(kind, query, Purpose::Play { shuffle }),
        }
        Flow::Continue
    }

    /// Lanza `task` si no hay otra en curso. `false` si había otra.
    fn start_task(&mut self, busy: &'static str, task: Task<B::Player>) -> bool {
        if self.task.is_some() {
            self.out.line(
                LineKind::Warn,
                format!(
                    "⚠ Esperá a que termine lo anterior ({}), o Esc para cancelarlo.",
                    self.busy
                ),
            );
            return false;
        }
        self.task = Some(task);
        self.busy = busy;
        true
    }

    fn search(&mut self, kind: SearchKind, query: String, purpose: Purpose) {
        let backend = self.backend.clone();
        self.start_task(
            "buscando",
            Box::pin(async move {
                let result = backend.search(kind, &query).await;
                Done::Search {
                    purpose,
                    kind,
                    query,
                    result,
                }
            }),
        );
    }

    /// Resuelve qué temas suenan para `target` (conectando si hace falta).
    fn prepare(&mut self, target: SpotifyUri, shuffle: bool) {
        let backend = self.backend.clone();
        let existing = self.player.clone().filter(|p| !p.session_lost());
        self.start_task(
            "preparando la reproducción",
            Box::pin(async move {
                let (player, connected) = match existing {
                    Some(player) => (player, false),
                    None => match backend.connect().await {
                        Ok(player) => (Rc::new(player), true),
                        Err(e) => {
                            return Done::Prepared {
                                shuffle,
                                new_player: None,
                                result: Err(e),
                            };
                        }
                    },
                };
                let result = backend.resolve(&player, &target).await;
                // Nadie más tiene el `Rc` de un reproductor recién conectado.
                let new_player = if connected {
                    Rc::into_inner(player)
                } else {
                    None
                };
                Done::Prepared {
                    shuffle,
                    new_player,
                    result,
                }
            }),
        );
    }

    fn on_done(&mut self, done: Done<B::Player>) {
        match done {
            Done::Login(Ok(())) => self
                .out
                .line(LineKind::Ok, "Sesión lista (audio y Web API)."),
            Done::Whoami(Ok(user)) => self.out.line(
                LineKind::Normal,
                format!("{} ({}), plan: {}", user.name(), user.id, user.plan()),
            ),
            Done::Login(Err(e)) | Done::Whoami(Err(e)) => self.out.error(&e),
            Done::Search {
                purpose,
                kind,
                query,
                result,
            } => match result {
                Ok(hits) if hits.is_empty() => self.out.error(&AppError::NoResults {
                    kind: kind.plural(),
                    query,
                }),
                Ok(hits) => {
                    for (i, hit) in hits.iter().enumerate() {
                        self.out.line(LineKind::Normal, select::hit_line(i, hit));
                    }
                    self.out.line(
                        LineKind::Dim,
                        format!(
                            "Elegí 1-{} y Enter (Enter solo = el primero, Esc cancela).",
                            hits.len()
                        ),
                    );
                    self.choosing = Some((hits, purpose));
                }
                Err(e) => self.out.error(&e),
            },
            Done::Prepared {
                shuffle,
                new_player,
                result,
            } => {
                if let Some(player) = new_player {
                    self.stop_playing();
                    self.events = Some(player.events());
                    self.player = Some(Rc::new(player));
                }
                match result {
                    Ok(resolved) => {
                        if resolved.skipped > 0 {
                            self.out.line(
                                LineKind::Warn,
                                format!(
                                    "⚠ Se omiten {} elementos (archivos locales o que Spotify no devolvió).",
                                    resolved.skipped
                                ),
                            );
                        }
                        self.start(resolved.tracks, shuffle);
                    }
                    Err(e) => self.out.error(&e),
                }
            }
        }
    }

    fn picked(&mut self, hit: Hit, purpose: Purpose) {
        match purpose {
            Purpose::Play { shuffle } => self.prepare(hit.uri, shuffle),
            Purpose::Enqueue => {
                let Some(playing) = &mut self.playing else {
                    self.out.line(LineKind::Warn, NOTHING_PLAYING);
                    return;
                };
                playing.queue.enqueue(hit.uri);
                let step = playing.queue.repreload();
                self.out.line(
                    LineKind::Ok,
                    format!("➕ En cola: {} — {}", hit.name, hit.detail),
                );
                self.apply(step);
            }
        }
    }

    /// Reemplaza lo que suena por `tracks`.
    fn start(&mut self, tracks: Vec<SpotifyUri>, shuffle: bool) {
        self.stop_playing();
        let Some(player) = self.player.clone() else {
            return;
        };
        let mut queue = Queue::new(tracks, shuffle, &mut self.rng);
        if shuffle && !queue.can_shuffle() {
            self.out.line(LineKind::Warn, NOTHING_TO_SHUFFLE);
        }
        let Some(first) = queue.start() else {
            return;
        };
        player.play(first);
        self.playing = Some(Playing {
            queue,
            state: PlayState::Loading,
            clock: Clock::default(),
            title: String::new(),
            artists: String::new(),
            duration: Duration::ZERO,
        });
    }

    fn stop_playing(&mut self) {
        if self.playing.take().is_some() {
            if let Some(player) = &self.player {
                player.stop();
            }
        }
    }

    fn control(&mut self, control: Control) {
        let (Some(playing), Some(player)) = (&mut self.playing, &self.player) else {
            self.out.line(LineKind::Warn, NOTHING_PLAYING);
            return;
        };
        let step = match control {
            Control::Pause => {
                match playing.state {
                    PlayState::Paused => player.resume(),
                    _ => player.pause(),
                }
                Step::Nothing
            }
            Control::Next => playing.queue.next(),
            Control::Prev => playing.queue.previous(playing.clock.elapsed()),
            Control::Shuffle => match playing.queue.toggle_shuffle(&mut self.rng) {
                Some(on) => {
                    let step = playing.queue.repreload();
                    self.out.line(
                        LineKind::Ok,
                        if on {
                            "🔀 Shuffle: sí"
                        } else {
                            "➡ Shuffle: no"
                        },
                    );
                    step
                }
                None => {
                    self.out.line(LineKind::Warn, NOTHING_TO_SHUFFLE);
                    Step::Nothing
                }
            },
        };
        self.apply(step);
    }

    fn on_event(&mut self, event: Option<PlayerEvent>) {
        let Some(event) = event else {
            // El hilo del reproductor terminó: hay que conectar de nuevo.
            self.playing = None;
            self.player = None;
            self.events = None;
            self.out.error(&AppError::PlayerStopped);
            return;
        };
        let (Some(playing), Some(player)) = (&mut self.playing, &self.player) else {
            return;
        };
        let (step, track) = on_player_event(
            event,
            &mut playing.queue,
            &mut playing.state,
            &mut playing.clock,
            || player.session_lost(),
        );
        if let Some(track) = track {
            let text = format!(
                "♪ {}{} — {}",
                playing.queue.position(),
                track.name,
                track.artists
            );
            playing.title = track.name;
            playing.artists = track.artists;
            playing.duration = track.duration;
            self.out.line(LineKind::Track, text);
        }
        self.apply(step);
    }

    fn apply(&mut self, step: Step) {
        let (Some(playing), Some(player)) = (&mut self.playing, &self.player) else {
            return;
        };
        match step {
            Step::Nothing => {}
            Step::Preload(next) => player.preload(next),
            Step::Play(next) => {
                player.play(next);
                playing.state = PlayState::Loading;
                playing.clock = Clock::default();
            }
            Step::Restart => {
                player.restart();
                playing.clock.seeked(0);
            }
            Step::Skip { unavailable, next } => {
                player.play(next);
                playing.state = PlayState::Loading;
                playing.clock = Clock::default();
                self.out.line(
                    LineKind::Warn,
                    format!(
                        "⚠ {} no está disponible, sigo con el próximo.",
                        uri_text(&unavailable)
                    ),
                );
            }
            Step::Done => {
                self.stop_playing();
                self.out.line(LineKind::Dim, "Fin de la lista.");
            }
            Step::Fail(e) => {
                self.stop_playing();
                self.out.error(&e);
            }
        }
    }

    /// Manda a la ventana lo que cambió de la barra y del prompt.
    fn publish(&mut self) {
        let now = self.playing.as_ref().map(|p| NowPlaying {
            title: p.title.clone(),
            artists: p.artists.clone(),
            position: p.queue.position().to_string(),
            duration: p.duration,
            clock: p.clock.clone(),
            state: p.state,
            shuffle: p.queue.shuffled(),
            queued: p.queue.queued(),
        });
        if now != self.shown {
            self.shown.clone_from(&now);
            self.out.send(Output::NowPlaying(now));
        }
        let prompt = match (&self.choosing, &self.task) {
            (Some((hits, _)), _) => Prompt::Choose(hits.len()),
            (None, Some(_)) => Prompt::Busy(self.busy),
            (None, None) => Prompt::Ready,
        };
        if prompt != self.shown_prompt {
            self.shown_prompt = prompt.clone();
            self.out.send(Output::Prompt(prompt));
        }
    }
}

#[derive(Debug, PartialEq)]
enum Flow {
    Continue,
    Exit,
}

#[derive(Debug, Clone, Copy)]
enum Control {
    Pause,
    Next,
    Prev,
    Shuffle,
}

/// Próximo evento del reproductor; sin reproductor, no termina nunca.
async fn next_event(events: &mut Option<PlayerEventChannel>) -> Option<PlayerEvent> {
    match events.as_mut() {
        Some(events) => events.recv().await,
        None => std::future::pending().await,
    }
}

/// Espera la tarea en curso; sin tarea, no termina nunca.
async fn finish<P>(task: &mut Option<Task<P>>) -> Done<P> {
    match task.as_mut() {
        Some(task) => task.await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    type Log = Rc<RefCell<Vec<String>>>;

    struct FakePlayer {
        log: Log,
    }

    impl Playback for FakePlayer {
        fn events(&self) -> PlayerEventChannel {
            // Los tests mandan los eventos llamando a `on_event`.
            mpsc::unbounded_channel().1
        }
        fn play(&self, track: SpotifyUri) {
            self.log.borrow_mut().push(format!("play {}", id(&track)));
        }
        fn preload(&self, track: SpotifyUri) {
            self.log
                .borrow_mut()
                .push(format!("preload {}", id(&track)));
        }
        fn pause(&self) {
            self.log.borrow_mut().push("pause".into());
        }
        fn resume(&self) {
            self.log.borrow_mut().push("resume".into());
        }
        fn restart(&self) {
            self.log.borrow_mut().push("restart".into());
        }
        fn stop(&self) {
            self.log.borrow_mut().push("stop".into());
        }
        fn session_lost(&self) -> bool {
            false
        }
    }

    struct FakeBackend {
        log: Log,
        premium: bool,
    }

    impl Backend for FakeBackend {
        type Player = FakePlayer;

        async fn login(&self) -> Result<(), AppError> {
            Ok(())
        }
        fn logout(&self) -> Result<bool, AppError> {
            Ok(true)
        }
        async fn current_user(&self) -> Result<User, AppError> {
            Ok(User {
                id: "yo".into(),
                display_name: None,
                product: Some("premium".into()),
            })
        }
        async fn search(&self, _: SearchKind, query: &str) -> Result<Vec<Hit>, AppError> {
            if query == "nada" {
                return Ok(Vec::new());
            }
            Ok(["h1", "h2"]
                .into_iter()
                .map(|name| Hit {
                    uri: track(name),
                    name: name.into(),
                    detail: String::new(),
                })
                .collect())
        }
        async fn connect(&self) -> Result<FakePlayer, AppError> {
            self.log.borrow_mut().push("connect".into());
            if !self.premium {
                return Err(AppError::NotPremium("free".into()));
            }
            Ok(FakePlayer {
                log: self.log.clone(),
            })
        }
        async fn resolve(&self, _: &FakePlayer, uri: &SpotifyUri) -> Result<Resolved, AppError> {
            // Un tema es él mismo; un álbum "a", tres temas "ax0".."ax2".
            let tracks = match uri {
                SpotifyUri::Track { .. } => vec![uri.clone()],
                _ => (0..3).map(|i| track(&format!("{}x{i}", id(uri)))).collect(),
            };
            Ok(Resolved { tracks, skipped: 0 })
        }
    }

    /// ID sin los ceros de relleno.
    fn id(uri: &SpotifyUri) -> String {
        let text = uri.to_uri().unwrap();
        let id = text.rsplit(':').next().unwrap();
        id.trim_start_matches('0').to_string()
    }

    fn track(name: &str) -> SpotifyUri {
        SpotifyUri::from_uri(&format!("spotify:track:{name:0>22}")).unwrap()
    }

    fn album(name: &str) -> String {
        format!("spotify:album:{name:0>22}")
    }

    struct Harness {
        engine: Engine<FakeBackend>,
        rx: std_mpsc::Receiver<Output>,
        log: Log,
    }

    fn harness(premium: bool) -> Harness {
        let log = Log::default();
        let (tx, rx) = std_mpsc::channel();
        let out = Outbox {
            tx,
            wake: Box::new(|| {}),
        };
        let backend = FakeBackend {
            log: log.clone(),
            premium,
        };
        Harness {
            engine: Engine::new(backend, out),
            rx,
            log,
        }
    }

    impl Harness {
        /// Escribe una línea y espera a que terminen las tareas que lanzó.
        async fn type_line(&mut self, text: &str) -> Flow {
            let flow = self.engine.on_input(Input::Line(text.into()));
            self.settle().await;
            flow
        }

        async fn settle(&mut self) {
            while let Some(task) = self.engine.task.take() {
                let done = task.await;
                self.engine.on_done(done);
            }
            self.engine.publish();
        }

        fn event(&mut self, event: PlayerEvent) {
            self.engine.on_event(Some(event));
            self.engine.publish();
        }

        /// Confirma el último pedido con `id` y lo pone a sonar.
        fn confirm(&mut self, id: u64) {
            self.event(PlayerEvent::PlayRequestIdChanged {
                play_request_id: id,
            });
            self.event(PlayerEvent::Playing {
                play_request_id: id,
                track_id: track("x"),
                position_ms: 0,
            });
        }

        fn outputs(&self) -> Vec<Output> {
            self.rx.try_iter().collect()
        }

        fn take_log(&self) -> Vec<String> {
            self.log.borrow_mut().drain(..).collect()
        }
    }

    fn has_line(outputs: &[Output], kind: LineKind, text: &str) -> bool {
        outputs
            .iter()
            .any(|o| matches!(o, Output::Line(k, t) if *k == kind && t.contains(text)))
    }

    #[tokio::test(flavor = "current_thread")]
    async fn play_de_un_link_conecta_y_reproduce() {
        let mut h = harness(true);
        h.type_line(&format!("play {}", album("a"))).await;
        assert_eq!(h.take_log(), ["connect", "play ax0"]);
        assert!(h.outputs().iter().any(|o| matches!(
            o,
            Output::NowPlaying(Some(now)) if now.state == PlayState::Loading
        )));
        assert_eq!(h.engine.shown_prompt, Prompt::Ready);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn play_nuevo_reemplaza_la_cola_sin_reconectar() {
        let mut h = harness(true);
        h.type_line(&format!("play {}", album("a"))).await;
        h.confirm(1);
        h.take_log();
        h.type_line(&format!("play {}", album("b"))).await;
        assert_eq!(h.take_log(), ["stop", "play bx0"]);
        // Los eventos viejos (id 1) ya no cuentan.
        h.event(PlayerEvent::EndOfTrack {
            play_request_id: 1,
            track_id: track("x"),
        });
        assert!(h.take_log().is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn controles_con_algo_sonando() {
        let mut h = harness(true);
        h.type_line(&format!("play {}", album("a"))).await;
        h.confirm(1);
        h.take_log();
        h.engine.on_input(Input::TogglePause);
        h.type_line("n").await;
        h.type_line("prev").await;
        assert_eq!(h.take_log(), ["pause", "play ax1", "play ax0"]);
        h.type_line("s").await;
        assert!(has_line(&h.outputs(), LineKind::Ok, "Shuffle: sí"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn controles_sin_nada_sonando_avisan() {
        let mut h = harness(true);
        for line in ["pause", "next", "p", "shuffle", "stop", "queue algo"] {
            h.type_line(line).await;
            assert!(
                has_line(&h.outputs(), LineKind::Warn, "No suena nada"),
                "{line}"
            );
        }
        assert!(h.take_log().is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn stop_corta_y_vacia_la_barra() {
        let mut h = harness(true);
        h.type_line(&format!("play {}", album("a"))).await;
        h.outputs();
        h.type_line("stop").await;
        assert_eq!(h.take_log().last().map(String::as_str), Some("stop"));
        assert!(h.outputs().contains(&Output::NowPlaying(None)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn buscar_y_elegir_con_numero_enter_o_esc() {
        let mut h = harness(true);
        h.type_line("play algo").await;
        assert!(h.outputs().contains(&Output::Prompt(Prompt::Choose(2))));
        h.type_line("7").await;
        assert!(has_line(&h.outputs(), LineKind::Warn, "del 1 al 2"));
        h.type_line(" 2 ").await;
        assert_eq!(h.take_log(), ["connect", "play h2"]);

        h.type_line("play list algo").await;
        h.type_line("").await;
        assert_eq!(h.take_log(), ["stop", "play h1"]);

        h.type_line("play algo").await;
        h.engine.on_input(Input::Cancel);
        h.engine.publish();
        assert!(h.outputs().contains(&Output::Prompt(Prompt::Ready)));
        assert!(h.engine.choosing.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn otro_comando_cancela_la_eleccion_y_se_ejecuta() {
        let mut h = harness(true);
        h.type_line("play algo").await;
        h.type_line("whoami").await;
        let outputs = h.outputs();
        assert!(has_line(&outputs, LineKind::Dim, "Elección cancelada"));
        assert!(has_line(&outputs, LineKind::Normal, "plan: premium"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sin_resultados_avisa() {
        let mut h = harness(true);
        h.type_line("play nada").await;
        assert!(has_line(&h.outputs(), LineKind::Error, "no encontré temas"));
        assert!(h.engine.choosing.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn encolar_suena_despues_del_actual() {
        let mut h = harness(true);
        h.type_line(&format!("play {}", album("a"))).await;
        h.confirm(1);
        h.type_line("a algo").await;
        h.type_line("1").await;
        assert!(has_line(&h.outputs(), LineKind::Ok, "En cola: h1"));
        h.take_log();
        h.type_line("next").await;
        assert_eq!(h.take_log(), ["play h1"]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn una_tarea_por_vez() {
        let mut h = harness(true);
        h.engine.on_input(Input::Line("whoami".into()));
        h.engine.on_input(Input::Line("play algo".into()));
        assert!(has_line(
            &h.outputs(),
            LineKind::Warn,
            "Esperá a que termine"
        ));
        h.engine.publish();
        assert!(
            h.outputs()
                .contains(&Output::Prompt(Prompt::Busy("pidiendo el usuario")))
        );
        h.engine.on_input(Input::Cancel);
        assert!(h.engine.task.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sin_premium_no_reproduce() {
        let mut h = harness(false);
        h.type_line(&format!("play {}", album("a"))).await;
        assert_eq!(h.take_log(), ["connect"]);
        assert!(has_line(&h.outputs(), LineKind::Error, "no es Premium"));
        assert!(h.engine.player.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fin_de_la_lista() {
        let mut h = harness(true);
        h.type_line(&format!("play {}", track("t").to_uri().unwrap()))
            .await;
        h.confirm(1);
        h.event(PlayerEvent::EndOfTrack {
            play_request_id: 1,
            track_id: track("t"),
        });
        let outputs = h.outputs();
        assert!(has_line(&outputs, LineKind::Dim, "Fin de la lista"));
        assert_eq!(outputs.last(), Some(&Output::NowPlaying(None)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reproductor_caido_pide_reconectar() {
        let mut h = harness(true);
        h.type_line(&format!("play {}", album("a"))).await;
        h.engine.on_event(None);
        assert!(has_line(&h.outputs(), LineKind::Error, "se cerró"));
        h.take_log();
        h.type_line(&format!("play {}", album("a"))).await;
        assert_eq!(h.take_log(), ["connect", "play ax0"]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn errores_de_uso_con_ayuda_corta() {
        let mut h = harness(true);
        h.type_line("bailar").await;
        let outputs = h.outputs();
        assert!(has_line(
            &outputs,
            LineKind::Error,
            "comando desconocido: bailar"
        ));
        assert!(has_line(&outputs, LineKind::Error, "`help`"));
        h.type_line("help").await;
        assert!(has_line(&h.outputs(), LineKind::Normal, "Comandos:"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn exit_corta_el_audio_y_cierra() {
        let mut h = harness(true);
        h.type_line(&format!("play {}", album("a"))).await;
        h.take_log();
        assert_eq!(h.type_line("exit").await, Flow::Exit);
        assert_eq!(h.take_log(), ["stop"]);
        assert!(h.outputs().contains(&Output::Exit));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cerrar_la_ventana_termina_el_loop() {
        let h = harness(true);
        let (tx, rx) = mpsc::unbounded_channel();
        tx.send(Input::Line("help".into())).unwrap();
        tx.send(Input::Line("clear".into())).unwrap();
        drop(tx);
        h.engine.run(rx).await;
        let outputs: Vec<_> = h.rx.try_iter().collect();
        assert!(has_line(&outputs, LineKind::Normal, "Comandos:"));
        assert!(outputs.contains(&Output::Clear));
    }
}
