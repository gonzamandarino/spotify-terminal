//! Motor de la app de escritorio: recibe lo que se escribe en la consola,
//! maneja el reproductor, las búsquedas y la cola, y avisa qué mostrar.
//!
//! Corre en su propio hilo ("motor", runtime tokio de un hilo), separado de
//! la ventana y de la sesión de audio: un redibujo lento no demora al
//! reproductor, y una búsqueda o un login no congelan la ventana. Se habla
//! con él solo por canales ([`Input`] → motor → [`Output`]).

use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    rc::Rc,
    sync::mpsc as std_mpsc,
    thread::{self, JoinHandle},
    time::Duration,
};

use librespot_core::SpotifyUri;
use librespot_playback::{
    config::Bitrate,
    player::{PlayerEvent, PlayerEventChannel},
};
use rand::{SeedableRng, rngs::StdRng};
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    time::Instant,
};

use super::{
    backend::{Backend, ClientIdStatus, Playback, SpotifyBackend},
    queue::{Clock, PlayState, Queue, Step, TrackInfo, on_player_event, uri_text},
    shell::{self, ShellCommand, VolumeCommand},
    volume::Volume,
};
use crate::{
    config,
    error::AppError,
    setup,
    spotify::{
        cover::Cover,
        player::Resolved,
        tap::AudioTap,
        web::{Hit, MyPlaylists, SearchKind, User},
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
    /// Botón de shuffle (spec 012): igual que `shuffle`.
    Shuffle,
    /// Botón ♥ (spec 012): `unlike` si el tema está en Tus me gusta; si
    /// no (o no se sabe), `like`.
    ToggleLike,
    /// Atajos de volumen: igual que `vol +` y `vol -`, sin escribir en la
    /// consola.
    VolumeUp,
    VolumeDown,
    /// Esc: cancela la elección de un resultado o la tarea en curso.
    Cancel,
    /// Un atajo global (spec 006), con la ventana enfocada o no. No
    /// escribe en la consola; sin nada sonando, los controles no hacen
    /// nada.
    Global(GlobalAction),
    /// Atajos vigentes para `help` ("Ctrl+→  siguiente"): los de la
    /// ventana y los globales activos (spec 007: configurables).
    Shortcuts {
        window: Vec<String>,
        global: Vec<String>,
    },
    /// Ajustes de reproducción nuevos (spec 007). Rigen desde el próximo
    /// uso; la calidad, desde el próximo `play`.
    Playback(PlaybackSettings),
    /// Si la ventana quiere las tapas de los discos (visualización Vinilo,
    /// spec 010). Con `true`, manda la del tema que suena y la de cada
    /// tema nuevo; con `false`, deja de bajarlas.
    Covers(bool),
}

/// Lo que puede hacer un atajo global.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum GlobalAction {
    /// Como `pause`.
    TogglePause,
    /// Como `next`.
    Next,
    /// Como `prev`.
    Prev,
    /// Como `stop`.
    Stop,
    /// Como `vol +` / `vol -`.
    VolumeUp,
    VolumeDown,
}

impl GlobalAction {
    pub(crate) const ALL: [GlobalAction; 6] = [
        GlobalAction::TogglePause,
        GlobalAction::Next,
        GlobalAction::Prev,
        GlobalAction::Stop,
        GlobalAction::VolumeUp,
        GlobalAction::VolumeDown,
    ];

    /// Para `help` y los avisos: "pausa / reanudar", "siguiente"...
    pub(crate) fn describe(self) -> &'static str {
        match self {
            GlobalAction::TogglePause => "pausa / reanudar",
            GlobalAction::Next => "siguiente",
            GlobalAction::Prev => "anterior",
            GlobalAction::Stop => "stop",
            GlobalAction::VolumeUp => "subir volumen",
            GlobalAction::VolumeDown => "bajar volumen",
        }
    }
}

/// Ajustes de reproducción que el usuario cambia desde la app de
/// escritorio (spec 007). La CLI usa siempre `default()`.
///
/// Invariante: cada valor está dentro de su rango de `config`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlaybackSettings {
    /// Cuánto suben / bajan `vol +` / `vol -` y sus atajos, en %.
    pub(crate) volume_step: u8,
    /// Con más que esto sonando, "anterior" reinicia el tema.
    pub(crate) previous_threshold: Duration,
    /// Calidad del audio desde el próximo `play`.
    pub(crate) bitrate: Bitrate,
}

impl Default for PlaybackSettings {
    fn default() -> PlaybackSettings {
        PlaybackSettings {
            volume_step: config::VOLUME_STEP,
            previous_threshold: config::PREVIOUS_RESTART_THRESHOLD,
            bitrate: config::AUDIO_BITRATE,
        }
    }
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
    /// URL de la tapa del disco (spec 010); llega con `Output::Cover` si
    /// la ventana las pidió.
    pub(crate) cover: Option<String>,
    /// Si el tema está en Tus me gusta (spec 011); `None` = no se sabe
    /// (consultando, falló, o es un episodio).
    pub(crate) liked: Option<bool>,
}

/// Qué espera la línea de entrada.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Prompt {
    Ready,
    /// Elegir entre N resultados.
    Choose(usize),
    /// Hay una tarea en curso (texto para mostrar); se puede escribir igual.
    Busy(&'static str),
    /// Esperando que se pegue un Client ID (spec 008).
    ClientId,
}

/// Lo que el motor le manda a la ventana.
#[derive(Debug, PartialEq)]
pub(crate) enum Output {
    Line(LineKind, String),
    /// `None` = no suena nada.
    NowPlaying(Option<NowPlaying>),
    Prompt(Prompt),
    /// Volumen actual, para la barra (se manda al arrancar y con cada
    /// cambio, suene algo o no).
    Volume(Volume),
    Clear,
    /// `exit`: la ventana tiene que cerrarse.
    Exit,
    /// La tapa de `url`, que es la del tema que sonaba al terminar de
    /// bajarla (solo con `Input::Covers(true)`).
    Cover {
        url: String,
        cover: Cover,
    },
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
///   cada `Output`, desde el hilo del motor. Con `tap`, lo que suena se
///   copia ahí (spec 010).
/// - Errores: `Internal` si el hilo no arranca. Si no arranca el runtime,
///   el error llega como `Output::Line` y el hilo termina.
pub(crate) fn spawn(
    wake: impl Fn() + Send + 'static,
    tap: Option<AudioTap>,
) -> Result<EngineHandle, AppError> {
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
            // Sin carpeta de datos, el volumen arranca en el default y no
            // se guarda.
            let volume_dir = config::data_dir().ok();
            let volume = volume_dir.as_deref().map(Volume::load).unwrap_or_default();
            let backend = SpotifyBackend { tap };
            runtime.block_on(Engine::new(backend, out, volume, volume_dir).run(in_rx));
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
const ONLY_TRACKS: &str = "⚠ Solo se pueden likear temas (esto es un episodio).";

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
    /// `playlists`: mis playlists, para elegir una.
    MyPlaylists {
        shuffle: bool,
        result: Result<MyPlaylists, AppError>,
    },
    /// Lista de temas lista para sonar. `new_player` viene si hubo que
    /// conectar (no había reproductor, se había caído la sesión o cambió
    /// la calidad), abierto con `volume` y `bitrate`.
    Prepared {
        shuffle: bool,
        new_player: Option<P>,
        volume: Volume,
        bitrate: Bitrate,
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
    cover: Option<String>,
    /// El tema que suena según el último `TrackChanged`; `None` hasta que
    /// llega.
    uri: Option<SpotifyUri>,
    /// Si `uri` está en Tus me gusta; `None` = no se sabe.
    liked: Option<bool>,
}

/// Descarga de una tapa: su URL y cómo terminó.
type CoverTask = Pin<Box<dyn Future<Output = (String, Result<Cover, AppError>)>>>;

/// Consulta de si un tema está en Tus me gusta: el tema y la respuesta.
type CheckTask = Pin<Box<dyn Future<Output = (SpotifyUri, Result<bool, AppError>)>>>;

/// `like` / `unlike` en curso.
type SaveTask = Pin<Box<dyn Future<Output = Saved>>>;

/// Cómo terminó un `like` (`save`) o `unlike`.
struct Saved {
    uri: SpotifyUri,
    /// Nombre del tema al pedirlo, para el mensaje.
    name: String,
    save: bool,
    result: Result<(), AppError>,
}

/// Estado del motor.
///
/// Invariantes:
/// - `events` es `Some` si y solo si `player` es `Some`, y es el canal de
///   ese reproductor.
/// - `playing` es `Some` solo con `player` `Some`.
/// - A lo sumo una tarea de fondo (`task`) a la vez; los controles de
///   reproducción no esperan a la tarea.
/// - `volume` es el del reproductor, si hay uno: se le aplica en cada
///   cambio y al conectar. Existe aunque no haya reproductor.
/// - `save_at` es `Some` si hay un cambio de volumen sin guardar.
/// - Con `asking`, `choosing` es `None`: pedir el Client ID
///   cancela la elección.
/// - `check_task` y `save_task` corren aparte de `task` (spec 011): a lo
///   sumo una consulta de ♥ (la del último tema) y un like/unlike a la
///   vez. Sus respuestas llevan la URI pedida y solo cambian el ♥ si
///   sigue sonando ese tema.
pub(crate) struct Engine<B: Backend> {
    backend: Rc<B>,
    player: Option<Rc<B::Player>>,
    events: Option<PlayerEventChannel>,
    playing: Option<Playing>,
    choosing: Option<(Vec<Hit>, Purpose)>,
    /// Se espera un Client ID: cada línea se toma como tal.
    asking: bool,
    task: Option<Task<B::Player>>,
    busy: &'static str,
    rng: StdRng,
    out: Outbox,
    shown: Option<NowPlaying>,
    shown_prompt: Prompt,
    volume: Volume,
    /// Dónde se guarda el volumen; `None` = no se guarda.
    volume_dir: Option<PathBuf>,
    save_at: Option<Instant>,
    shown_volume: Option<Volume>,
    /// Atajos de la ventana, para `help`. Vacío = sin sección.
    window_shortcuts: Vec<String>,
    /// Atajos globales activos, para `help`. Vacío = sin sección.
    global_shortcuts: Vec<String>,
    playback: PlaybackSettings,
    /// Calidad con la que se abrió `player`.
    player_bitrate: Bitrate,
    /// La ventana quiere las tapas (spec 010).
    covers: bool,
    /// Tapa bajándose; corre aparte de `task` (no ocupa al motor).
    cover_task: Option<CoverTask>,
    /// Última tapa pedida (bajándose o ya mandada): no se pide de nuevo.
    cover_asked: Option<String>,
    check_task: Option<CheckTask>,
    save_task: Option<SaveTask>,
}

impl<B: Backend + 'static> Engine<B> {
    fn new(backend: B, out: Outbox, volume: Volume, volume_dir: Option<PathBuf>) -> Engine<B> {
        Engine {
            backend: Rc::new(backend),
            player: None,
            events: None,
            playing: None,
            choosing: None,
            asking: false,
            task: None,
            busy: "",
            rng: StdRng::from_rng(&mut rand::rng()),
            out,
            shown: None,
            shown_prompt: Prompt::Ready,
            volume,
            volume_dir,
            save_at: None,
            shown_volume: None,
            window_shortcuts: Vec::new(),
            global_shortcuts: Vec::new(),
            playback: PlaybackSettings::default(),
            player_bitrate: config::AUDIO_BITRATE,
            covers: false,
            cover_task: None,
            cover_asked: None,
            check_task: None,
            save_task: None,
        }
    }

    /// Atiende comandos, eventos del reproductor y tareas hasta `exit` o
    /// hasta que se cierra `inputs`. Al salir corta el audio y guarda el
    /// volumen si quedó un cambio sin guardar.
    async fn run(mut self, mut inputs: UnboundedReceiver<Input>) {
        self.on_open();
        self.publish();
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
                () = save_due(self.save_at) => self.save_volume(),
                (url, result) = finish_cover(&mut self.cover_task) => {
                    self.cover_task = None;
                    self.on_cover(url, result);
                }
                (uri, result) = finish_lane(&mut self.check_task) => {
                    self.check_task = None;
                    self.on_checked(uri, result);
                }
                saved = finish_lane(&mut self.save_task) => {
                    self.save_task = None;
                    self.on_saved(saved);
                }
            }
            self.publish();
        }
        self.stop_playing();
        self.save_volume();
    }

    /// Al abrir la app: sin Client ID configurado, muestra la guía y lo
    /// pide (AC-1 de spec 008) en vez de fallar en el primer comando.
    fn on_open(&mut self) {
        if let ClientIdStatus::Missing { saved_invalid } = self.backend.client_id_status() {
            if saved_invalid {
                self.out.line(
                    LineKind::Warn,
                    format!(
                        "⚠ El Client ID guardado ({}) no es válido: hay que cargarlo de nuevo.",
                        config::CLIENT_ID_FILE
                    ),
                );
            }
            self.ask_client_id();
        }
    }

    /// Muestra la guía y pasa a esperar un Client ID (al abrir sin Client
    /// ID, o con `setup`).
    fn ask_client_id(&mut self) {
        self.choosing = None;
        self.asking = true;
        self.out.line(LineKind::Normal, config::setup_guide());
        self.out
            .line(LineKind::Dim, "Pegá el Client ID y Enter (Esc cancela).");
    }

    /// Una línea mientras se espera el Client ID: se valida y se guarda.
    /// Solo Esc sale de este modo (un comando mal tipeado no lo corta). Al
    /// guardarlo sigue directo con el login, salvo que `.env` / la variable
    /// de entorno lo tapen.
    fn on_client_id(&mut self, text: &str) {
        let id = match setup::validate(text) {
            Ok(id) => id,
            Err(reason) => {
                self.out.line(
                    LineKind::Warn,
                    format!("⚠ Ese Client ID {reason}. Probá de nuevo (Esc cancela)."),
                );
                return;
            }
        };
        match self.backend.set_client_id(&id) {
            Ok(applied) => {
                self.asking = false;
                self.out.line(LineKind::Ok, "✔ Client ID guardado.");
                if let Some(notice) = applied.notice() {
                    self.out.line(LineKind::Warn, notice);
                }
                if !applied.overridden_by_env {
                    self.login();
                }
            }
            Err(e) => self.out.error(&e),
        }
    }

    fn login(&mut self) {
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

    fn on_input(&mut self, input: Input) -> Flow {
        match input {
            Input::Line(text) => return self.on_line(&text),
            Input::TogglePause => self.control(Control::Pause),
            Input::Next => self.control(Control::Next),
            Input::Prev => self.control(Control::Prev),
            Input::Shuffle => self.control(Control::Shuffle),
            Input::ToggleLike => {
                let liked = self.playing.as_ref().and_then(|p| p.liked);
                self.set_liked(liked != Some(true));
            }
            Input::VolumeUp => self.volume_step(true, false),
            Input::VolumeDown => self.volume_step(false, false),
            Input::Cancel => {
                if std::mem::take(&mut self.asking) {
                    self.out.line(
                        LineKind::Dim,
                        "Configuración cancelada: el Client ID no cambió.",
                    );
                } else if self.choosing.take().is_some() {
                    self.out.line(LineKind::Dim, "Elección cancelada.");
                } else if self.task.take().is_some() {
                    self.out.line(LineKind::Dim, "Cancelado.");
                }
            }
            Input::Global(action) => self.global(action),
            Input::Shortcuts { window, global } => {
                self.window_shortcuts = window;
                self.global_shortcuts = global;
            }
            Input::Playback(playback) => self.playback = playback,
            Input::Covers(on) => {
                self.covers = on;
                if on {
                    self.fetch_cover();
                } else {
                    self.cover_task = None;
                    self.cover_asked = None;
                }
            }
        }
        Flow::Continue
    }

    /// Atajo global: lo mismo que el comando, sin escribir en la consola.
    fn global(&mut self, action: GlobalAction) {
        match action {
            GlobalAction::VolumeUp => self.volume_step(true, false),
            GlobalAction::VolumeDown => self.volume_step(false, false),
            // Sin nada sonando no hay nada que hacer, y tampoco que avisar:
            // la consola puede estar minimizada.
            _ if self.playing.is_none() => {}
            GlobalAction::TogglePause => self.control(Control::Pause),
            GlobalAction::Next => self.control(Control::Next),
            GlobalAction::Prev => self.control(Control::Prev),
            GlobalAction::Stop => self.stop_playing(),
        }
    }

    /// Ayuda de `help`, con los atajos vigentes si hay.
    fn help(&self) -> String {
        let mut text = shell::HELP.to_string();
        for (title, list) in [
            ("Atajos de la ventana:", &self.window_shortcuts),
            (
                "Atajos globales (andan con la app minimizada):",
                &self.global_shortcuts,
            ),
        ] {
            if list.is_empty() {
                continue;
            }
            text.push_str("\n\n");
            text.push_str(title);
            for line in list {
                text.push_str("\n  ");
                text.push_str(line);
            }
        }
        text
    }

    fn on_line(&mut self, text: &str) -> Flow {
        if self.asking {
            self.on_client_id(text);
            return Flow::Continue;
        }
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
                self.out.line(LineKind::Normal, self.help());
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
            ShellCommand::Like => self.set_liked(true),
            ShellCommand::Unlike => self.set_liked(false),
            ShellCommand::Playlists { shuffle } => self.my_playlists(shuffle),
            ShellCommand::Stop => {
                if self.playing.is_some() {
                    self.stop_playing();
                    self.out.line(LineKind::Dim, "⏹ Detenido.");
                } else {
                    self.out.line(LineKind::Warn, NOTHING_PLAYING);
                }
            }
            ShellCommand::Volume(VolumeCommand::Show) => {
                self.out.line(LineKind::Normal, volume_text(self.volume));
            }
            ShellCommand::Volume(VolumeCommand::Set(level)) => {
                self.change_volume(|v| v.set(level), true);
            }
            ShellCommand::Volume(VolumeCommand::Up) => self.volume_step(true, true),
            ShellCommand::Volume(VolumeCommand::Down) => self.volume_step(false, true),
            ShellCommand::Mute => self.change_volume(Volume::toggle_mute, true),
            ShellCommand::Clear => self.out.send(Output::Clear),
            ShellCommand::Exit => {
                self.stop_playing();
                self.out.send(Output::Exit);
                return Flow::Exit;
            }
            ShellCommand::Cli(Command::Login) => self.login(),
            ShellCommand::Cli(Command::Setup) => self.ask_client_id(),
            ShellCommand::Cli(Command::Version) => {
                self.out.line(
                    LineKind::Normal,
                    format!("spotify-terminal {}", config::VERSION),
                );
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

    fn my_playlists(&mut self, shuffle: bool) {
        let backend = self.backend.clone();
        if self.start_task(
            "buscando tus playlists",
            Box::pin(async move {
                let result = backend.my_playlists().await;
                Done::MyPlaylists { shuffle, result }
            }),
        ) {
            self.out.line(LineKind::Dim, "Buscando tus playlists…");
        }
    }

    /// `like` (`save`) o `unlike` del tema que suena.
    ///
    /// Si ya se sabe que está (o no está) en Tus me gusta, avisa sin
    /// pedir nada; si no se sabe, pide igual (guardar y quitar son
    /// idempotentes). Solo un like/unlike a la vez.
    fn set_liked(&mut self, save: bool) {
        let Some(playing) = &self.playing else {
            self.out.line(LineKind::Warn, NOTHING_PLAYING);
            return;
        };
        let Some(uri) = playing.uri.clone() else {
            self.out
                .line(LineKind::Warn, "⚠ Esperá a que empiece a sonar el tema.");
            return;
        };
        if !matches!(uri, SpotifyUri::Track { .. }) {
            self.out.line(LineKind::Warn, ONLY_TRACKS);
            return;
        }
        if self.save_task.is_some() {
            self.out
                .line(LineKind::Warn, "⚠ Esperá a que termine el like anterior.");
            return;
        }
        let name = playing.title.clone();
        match (playing.liked, save) {
            (Some(true), true) => {
                self.out.line(
                    LineKind::Normal,
                    format!("♥ {name} ya estaba en Tus me gusta."),
                );
                return;
            }
            (Some(false), false) => {
                self.out.line(
                    LineKind::Normal,
                    format!("{name} no estaba en Tus me gusta."),
                );
                return;
            }
            _ => {}
        }
        // Una consulta en vuelo podría pisar el ♥ con el estado de antes.
        self.check_task = None;
        let backend = self.backend.clone();
        self.save_task = Some(Box::pin(async move {
            let result = backend.set_saved(&uri, save).await;
            Saved {
                uri,
                name,
                save,
                result,
            }
        }));
    }

    fn on_saved(&mut self, saved: Saved) {
        let Saved {
            uri,
            name,
            save,
            result,
        } = saved;
        match result {
            Ok(()) => {
                if let Some(playing) = &mut self.playing {
                    if playing.uri.as_ref() == Some(&uri) {
                        playing.liked = Some(save);
                    }
                }
                let text = if save {
                    format!("♥ {name} — agregado a Tus me gusta")
                } else {
                    format!("♡ {name} — quitado de Tus me gusta")
                };
                self.out.line(LineKind::Ok, text);
            }
            Err(e) => self.out.error(&e),
        }
    }

    /// Consulta si el tema que suena está en Tus me gusta (una vez por
    /// tema; los episodios no se consultan). Una consulta anterior se
    /// abandona.
    fn check_liked(&mut self) {
        self.check_task = None;
        let Some(uri) = self.playing.as_ref().and_then(|p| p.uri.clone()) else {
            return;
        };
        if !matches!(uri, SpotifyUri::Track { .. }) {
            return;
        }
        let backend = self.backend.clone();
        self.check_task = Some(Box::pin(async move {
            let result = backend.is_saved(&uri).await;
            (uri, result)
        }));
    }

    /// Respuesta de la consulta del ♥. Un error no se avisa (el ♥ queda
    /// en "no se sabe"): pasaría en cada tema sin que se pueda hacer nada.
    fn on_checked(&mut self, uri: SpotifyUri, result: Result<bool, AppError>) {
        if let (Some(playing), Ok(liked)) = (&mut self.playing, result) {
            if playing.uri.as_ref() == Some(&uri) {
                playing.liked = Some(liked);
            }
        }
    }

    /// Resuelve qué temas suenan para `target` (conectando si hace falta).
    fn prepare(&mut self, target: SpotifyUri, shuffle: bool) {
        let backend = self.backend.clone();
        // Con otra calidad pedida se abre un reproductor nuevo: lo que
        // suena sigue hasta que el nuevo esté listo (spec 007).
        let bitrate = self.playback.bitrate;
        let existing = self
            .player
            .clone()
            .filter(|p| !p.session_lost() && self.player_bitrate == bitrate);
        let volume = self.volume;
        self.start_task(
            "preparando la reproducción",
            Box::pin(async move {
                let (player, connected) = match existing {
                    Some(player) => (player, false),
                    None => match backend.connect(volume, bitrate).await {
                        Ok(player) => (Rc::new(player), true),
                        Err(e) => {
                            return Done::Prepared {
                                shuffle,
                                new_player: None,
                                volume,
                                bitrate,
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
                    volume,
                    bitrate,
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
            Done::MyPlaylists { shuffle, result } => match result {
                Ok(list) if list.hits.is_empty() => {
                    self.out
                        .line(LineKind::Warn, "⚠ No tenés playlists guardadas.");
                }
                Ok(list) => {
                    for (i, hit) in list.hits.iter().enumerate() {
                        self.out.line(LineKind::Normal, select::hit_line(i, hit));
                    }
                    if list.hits.len() >= config::MY_PLAYLISTS_MAX && list.total > list.hits.len() {
                        self.out.line(
                            LineKind::Warn,
                            format!(
                                "⚠ Se muestran las primeras {}: {} quedan afuera.",
                                list.hits.len(),
                                list.total - list.hits.len()
                            ),
                        );
                    }
                    self.out.line(
                        LineKind::Dim,
                        format!(
                            "Elegí 1-{} y Enter (Enter solo = la primera, Esc cancela).",
                            list.hits.len()
                        ),
                    );
                    self.choosing = Some((list.hits, Purpose::Play { shuffle }));
                }
                Err(e) => self.out.error(&e),
            },
            Done::Prepared {
                shuffle,
                new_player,
                volume,
                bitrate,
                result,
            } => {
                if let Some(player) = new_player {
                    self.stop_playing();
                    self.player_bitrate = bitrate;
                    if volume != self.volume {
                        // Cambió mientras conectaba.
                        player.set_volume(self.volume);
                    }
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
            cover: None,
            uri: None,
            liked: None,
        });
    }

    /// Si la ventana quiere tapas, empieza a bajar la del tema que suena
    /// (una sola vez por URL). Una descarga anterior se abandona.
    fn fetch_cover(&mut self) {
        let Some(url) = self.playing.as_ref().and_then(|p| p.cover.clone()) else {
            return;
        };
        if !self.covers || self.cover_asked.as_ref() == Some(&url) {
            return;
        }
        self.cover_asked = Some(url.clone());
        let backend = self.backend.clone();
        self.cover_task = Some(Box::pin(async move {
            let result = backend.cover(&url).await;
            (url, result)
        }));
    }

    /// Terminó de bajarse una tapa: se manda solo si sigue sonando ese
    /// tema y la ventana las sigue queriendo. Un error no se avisa (spec
    /// 010: el vinilo gira sin tapa).
    fn on_cover(&mut self, url: String, result: Result<Cover, AppError>) {
        let current = self.playing.as_ref().and_then(|p| p.cover.as_deref());
        if let (true, Some(current), Ok(cover)) = (self.covers, current, result) {
            if current == url {
                self.out.send(Output::Cover { url, cover });
            }
        }
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
            Control::Prev => playing
                .queue
                .previous(playing.clock.elapsed(), self.playback.previous_threshold),
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
            self.on_track(track);
        }
        self.apply(step);
    }

    /// Empezó a cargar un tema: se muestra, se pide su tapa y, si es otro
    /// que el anterior, se consulta su ♥.
    fn on_track(&mut self, track: TrackInfo) {
        let Some(playing) = &mut self.playing else {
            return;
        };
        let text = format!(
            "♪ {}{} — {}",
            playing.queue.position(),
            track.name,
            track.artists
        );
        playing.title = track.name;
        playing.artists = track.artists;
        playing.duration = track.duration;
        playing.cover = track.cover;
        let changed = playing.uri.as_ref() != Some(&track.uri);
        if changed {
            playing.uri = Some(track.uri);
            playing.liked = None;
        }
        self.out.line(LineKind::Track, text);
        self.fetch_cover();
        if changed {
            self.check_liked();
        }
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

    /// Aplica `change` al volumen y al reproductor (si hay), y programa el
    /// guardado. `announce`: escribirlo en la consola (comandos sí, atajos
    /// no).
    /// Sube o baja un paso de volumen (el de los ajustes).
    fn volume_step(&mut self, up: bool, announce: bool) {
        let step = self.playback.volume_step;
        if up {
            self.change_volume(|v| v.up(step), announce);
        } else {
            self.change_volume(|v| v.down(step), announce);
        }
    }

    fn change_volume(&mut self, change: impl FnOnce(&mut Volume), announce: bool) {
        change(&mut self.volume);
        if let Some(player) = &self.player {
            player.set_volume(self.volume);
        }
        self.save_at = Some(Instant::now() + config::VOLUME_SAVE_DELAY);
        if announce {
            self.out.line(LineKind::Ok, volume_text(self.volume));
        }
    }

    /// Guarda el volumen si hay un cambio pendiente. Si falla, avisa y
    /// sigue: la reproducción no se entera.
    fn save_volume(&mut self) {
        if self.save_at.take().is_none() {
            return;
        }
        if let Some(dir) = &self.volume_dir {
            if let Err(e) = self.volume.save(dir) {
                self.out.line(
                    LineKind::Warn,
                    format!("⚠ No se pudo guardar el volumen: {e}"),
                );
            }
        }
    }

    /// Manda a la ventana lo que cambió de la barra, del volumen y del
    /// prompt.
    fn publish(&mut self) {
        if self.shown_volume != Some(self.volume) {
            self.shown_volume = Some(self.volume);
            self.out.send(Output::Volume(self.volume));
        }
        let now = self.playing.as_ref().map(|p| NowPlaying {
            title: p.title.clone(),
            artists: p.artists.clone(),
            position: p.queue.position().to_string(),
            duration: p.duration,
            clock: p.clock.clone(),
            state: p.state,
            shuffle: p.queue.shuffled(),
            queued: p.queue.queued(),
            cover: p.cover.clone(),
            liked: p.liked,
        });
        if now != self.shown {
            self.shown.clone_from(&now);
            self.out.send(Output::NowPlaying(now));
        }
        let prompt = match (&self.choosing, &self.task) {
            _ if self.asking => Prompt::ClientId,
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

/// Espera hasta `at`; sin plazo, no termina nunca.
async fn save_due(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

/// "🔊 Volumen: 70 %" o "🔇 Volumen: silenciado (70 %)".
fn volume_text(volume: Volume) -> String {
    let icon = if volume.muted() { "🔇" } else { "🔊" };
    format!("{icon} Volumen: {volume}")
}

/// Espera la descarga de tapa en curso; sin descarga, no termina nunca.
async fn finish_cover(task: &mut Option<CoverTask>) -> (String, Result<Cover, AppError>) {
    match task.as_mut() {
        Some(task) => task.await,
        None => std::future::pending().await,
    }
}

/// Espera la consulta o el like en curso; sin nada, no termina nunca.
async fn finish_lane<T>(task: &mut Option<Pin<Box<dyn Future<Output = T>>>>) -> T {
    match task.as_mut() {
        Some(task) => task.await,
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
    use std::cell::{Cell, RefCell};

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
        fn set_volume(&self, volume: Volume) {
            self.log.borrow_mut().push(format!("volume {volume}"));
        }
        fn session_lost(&self) -> bool {
            false
        }
    }

    struct FakeBackend {
        log: Log,
        premium: bool,
        client_id: ClientIdStatus,
        /// Lo que devuelve `set_client_id`.
        applied: setup::Applied,
        /// IDs de los temas en Tus me gusta.
        library: RefCell<Vec<String>>,
        /// Si fallan los pedidos a Tus me gusta.
        library_fails: Cell<bool>,
        /// Cuántas playlists devuelve `my_playlists` y cuántas dice que hay.
        playlists: (usize, usize),
    }

    impl Backend for FakeBackend {
        type Player = FakePlayer;

        fn client_id_status(&self) -> ClientIdStatus {
            self.client_id
        }
        fn set_client_id(&self, id: &setup::ClientId) -> Result<setup::Applied, AppError> {
            self.log
                .borrow_mut()
                .push(format!("client-id {}", id.as_str()));
            Ok(self.applied)
        }

        async fn login(&self) -> Result<(), AppError> {
            self.log.borrow_mut().push("login".into());
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
        async fn my_playlists(&self) -> Result<MyPlaylists, AppError> {
            self.log.borrow_mut().push("my-playlists".into());
            let (shown, total) = self.playlists;
            let hits = (1..=shown)
                .map(|i| Hit {
                    uri: SpotifyUri::from_uri(&format!(
                        "spotify:playlist:{:0>22}",
                        format!("p{i}")
                    ))
                    .unwrap(),
                    name: format!("p{i}"),
                    detail: "de yo".into(),
                })
                .collect();
            Ok(MyPlaylists { hits, total })
        }
        async fn is_saved(&self, uri: &SpotifyUri) -> Result<bool, AppError> {
            self.log.borrow_mut().push(format!("saved? {}", id(uri)));
            if self.library_fails.get() {
                return Err(AppError::Network("sin red".into()));
            }
            Ok(self.library.borrow().contains(&id(uri)))
        }
        async fn set_saved(&self, uri: &SpotifyUri, save: bool) -> Result<(), AppError> {
            let verb = if save { "save" } else { "unsave" };
            self.log.borrow_mut().push(format!("{verb} {}", id(uri)));
            if self.library_fails.get() {
                return Err(AppError::Network("sin red".into()));
            }
            let mut library = self.library.borrow_mut();
            library.retain(|t| *t != id(uri));
            if save {
                library.push(id(uri));
            }
            Ok(())
        }
        async fn connect(&self, volume: Volume, bitrate: Bitrate) -> Result<FakePlayer, AppError> {
            let line = if bitrate == config::AUDIO_BITRATE {
                format!("connect {volume}")
            } else {
                format!("connect {volume} {bitrate:?}")
            };
            self.log.borrow_mut().push(line);
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
        async fn cover(&self, url: &str) -> Result<Cover, AppError> {
            self.log.borrow_mut().push(format!("cover {url}"));
            if url.contains("falla") {
                return Err(AppError::Network("sin red".into()));
            }
            Ok(Cover {
                width: 1,
                height: 1,
                rgba: vec![1, 2, 3, 255],
            })
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
            client_id: ClientIdStatus::Ready,
            applied: setup::Applied {
                changed: true,
                overridden_by_env: false,
            },
            library: RefCell::default(),
            library_fails: Cell::new(false),
            playlists: (3, 3),
        };
        Harness {
            engine: Engine::new(backend, out, Volume::default(), None),
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

    impl Harness {
        /// Suena un tema cuya tapa es `url` (como si hubiera llegado su
        /// `TrackChanged`).
        async fn playing_with_cover(&mut self, url: &str) {
            if self.engine.playing.is_none() {
                self.type_line(&format!("play {}", album("a"))).await;
                self.confirm(1);
            }
            if let Some(playing) = self.engine.playing.as_mut() {
                playing.cover = Some(url.into());
            }
            self.engine.fetch_cover();
            self.engine.publish();
        }

        /// Termina la descarga de tapa en curso, si hay.
        async fn finish_cover(&mut self) {
            if let Some(task) = self.engine.cover_task.take() {
                let (url, result) = task.await;
                self.engine.on_cover(url, result);
            }
        }

        /// Suena `uri` (como si hubiera llegado su `TrackChanged`), sin
        /// esperar la consulta del ♥.
        async fn track_changed(&mut self, uri: SpotifyUri) {
            if self.engine.playing.is_none() {
                self.type_line(&format!("play {}", album("a"))).await;
                self.confirm(1);
            }
            let name = id(&uri);
            self.engine.on_track(TrackInfo {
                uri,
                name,
                artists: "Artista".into(),
                duration: Duration::from_secs(200),
                cover: None,
            });
            self.engine.publish();
        }

        /// Suena el tema `name` y ya se sabe su ♥.
        async fn playing_track(&mut self, name: &str) {
            self.track_changed(track(name)).await;
            self.finish_library().await;
        }

        /// Termina la consulta del ♥ y el like/unlike en curso.
        async fn finish_library(&mut self) {
            if let Some(task) = self.engine.check_task.take() {
                let (uri, result) = task.await;
                self.engine.on_checked(uri, result);
            }
            if let Some(task) = self.engine.save_task.take() {
                let saved = task.await;
                self.engine.on_saved(saved);
            }
            self.engine.publish();
        }

        fn liked(&self) -> Option<bool> {
            self.engine.shown.as_ref().and_then(|now| now.liked)
        }

        fn covers_sent(&self) -> Vec<String> {
            self.outputs()
                .into_iter()
                .filter_map(|o| match o {
                    Output::Cover { url, .. } => Some(url),
                    _ => None,
                })
                .collect()
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn tapas_solo_si_la_ventana_las_pide() {
        let mut h = harness(true);
        h.playing_with_cover("t1").await;
        h.finish_cover().await;
        assert!(h.covers_sent().is_empty());
        assert!(!h.take_log().iter().any(|l| l.starts_with("cover")));
        // Al pedirlas llega la del tema que suena, una sola vez.
        h.engine.on_input(Input::Covers(true));
        h.finish_cover().await;
        assert_eq!(h.covers_sent(), ["t1"]);
        h.playing_with_cover("t1").await;
        h.finish_cover().await;
        assert!(h.covers_sent().is_empty());
        // Tema nuevo, tapa nueva.
        h.playing_with_cover("t2").await;
        h.finish_cover().await;
        assert_eq!(h.covers_sent(), ["t2"]);
        let covers: Vec<String> = h
            .take_log()
            .into_iter()
            .filter(|l| l.starts_with("cover"))
            .collect();
        assert_eq!(covers, ["cover t1", "cover t2"]);
        // Sin pedirlas, no se bajan más.
        h.engine.on_input(Input::Covers(false));
        h.playing_with_cover("t3").await;
        assert!(h.engine.cover_task.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn una_tapa_que_llega_tarde_o_falla_no_se_manda() {
        let mut h = harness(true);
        h.engine.on_input(Input::Covers(true));
        h.playing_with_cover("t1").await;
        // Cambia el tema antes de que termine la descarga de t1.
        let late = h.engine.cover_task.take().expect("bajando t1");
        h.playing_with_cover("t2").await;
        let (url, result) = late.await;
        h.engine.on_cover(url, result);
        h.finish_cover().await;
        assert_eq!(h.covers_sent(), ["t2"]);
        // Un error no se avisa en la consola.
        h.playing_with_cover("falla").await;
        h.finish_cover().await;
        let out = h.outputs();
        assert!(!out.iter().any(|o| matches!(o, Output::Cover { .. })));
        assert!(
            !out.iter()
                .any(|o| matches!(o, Output::Line(LineKind::Error | LineKind::Warn, _)))
        );
    }

    fn has_line(outputs: &[Output], kind: LineKind, text: &str) -> bool {
        outputs
            .iter()
            .any(|o| matches!(o, Output::Line(k, t) if *k == kind && t.contains(text)))
    }

    #[tokio::test(flavor = "current_thread")]
    async fn like_agrega_el_tema_y_prende_el_corazon() {
        let mut h = harness(true);
        h.playing_track("t1").await;
        assert!(h.take_log().contains(&"saved? t1".to_string()));
        assert_eq!(h.liked(), Some(false));
        h.outputs();

        h.type_line("like").await;
        h.finish_library().await;
        assert_eq!(h.take_log(), ["save t1"]);
        assert!(has_line(
            &h.outputs(),
            LineKind::Ok,
            "♥ t1 — agregado a Tus me gusta"
        ));
        assert_eq!(h.liked(), Some(true));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn like_o_unlike_que_no_cambian_nada_no_piden() {
        let mut h = harness(true);
        h.engine.backend.library.borrow_mut().push("t1".into());
        h.playing_track("t1").await;
        assert_eq!(h.liked(), Some(true));
        h.take_log();

        h.type_line("like").await;
        assert!(h.engine.save_task.is_none());
        assert!(has_line(&h.outputs(), LineKind::Normal, "ya estaba"));

        h.type_line("unlike").await;
        h.finish_library().await;
        assert_eq!(h.take_log(), ["unsave t1"]);
        assert!(has_line(
            &h.outputs(),
            LineKind::Ok,
            "quitado de Tus me gusta"
        ));
        assert_eq!(h.liked(), Some(false));

        h.type_line("unlike").await;
        assert!(h.engine.save_task.is_none());
        assert!(has_line(&h.outputs(), LineKind::Normal, "no estaba"));
        assert!(h.take_log().is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn like_sin_saber_el_estado_pide_igual_y_descarta_la_consulta() {
        let mut h = harness(true);
        h.track_changed(track("t1")).await;
        assert!(h.engine.check_task.is_some());
        h.type_line("like").await;
        assert!(h.engine.check_task.is_none());
        h.finish_library().await;
        assert_eq!(h.liked(), Some(true));
        assert!(has_line(&h.outputs(), LineKind::Ok, "agregado"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn like_sin_nada_sonando_o_con_un_episodio_no_pide() {
        let mut h = harness(true);
        h.type_line("like").await;
        h.type_line("unlike").await;
        let outputs = h.outputs();
        assert!(has_line(&outputs, LineKind::Warn, "No suena nada"));
        assert!(h.engine.save_task.is_none());

        let episode = SpotifyUri::from_uri(&format!("spotify:episode:{:0>22}", "e1")).unwrap();
        h.track_changed(episode).await;
        assert!(h.engine.check_task.is_none());
        h.type_line("like").await;
        assert!(has_line(
            &h.outputs(),
            LineKind::Warn,
            "Solo se pueden likear temas"
        ));
        assert!(h.engine.save_task.is_none());
        assert!(!h.take_log().iter().any(|l| l.contains("save")));
        assert_eq!(h.liked(), None);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cambio_de_tema_consulta_de_nuevo_y_una_respuesta_vieja_no_pisa() {
        let mut h = harness(true);
        h.playing_track("t1").await;
        h.type_line("like").await;
        // Antes de que vuelva el like, ya suena otro tema.
        let save = h.engine.save_task.take().unwrap();
        h.track_changed(track("t2")).await;
        assert!(h.engine.check_task.is_some());
        h.engine.on_saved(save.await);
        h.engine.on_checked(track("t1"), Ok(true));
        h.engine.publish();
        assert_eq!(h.liked(), None);
        assert!(has_line(&h.outputs(), LineKind::Ok, "♥ t1 — agregado"));
        h.finish_library().await;
        assert_eq!(h.liked(), Some(false));

        // El mismo tema otra vez (p. ej. reiniciado) no se vuelve a consultar.
        h.track_changed(track("t2")).await;
        assert!(h.engine.check_task.is_none());
        assert_eq!(h.liked(), Some(false));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn un_like_por_vez() {
        let mut h = harness(true);
        h.playing_track("t1").await;
        h.type_line("like").await;
        h.type_line("unlike").await;
        assert!(has_line(&h.outputs(), LineKind::Warn, "like anterior"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn error_de_tus_me_gusta_se_ve_y_no_corta_lo_que_suena() {
        let mut h = harness(true);
        h.engine.backend.library_fails.set(true);
        h.playing_track("t1").await;
        // La consulta falla callada: el ♥ queda sin saber.
        assert_eq!(h.liked(), None);
        assert!(
            !h.outputs()
                .iter()
                .any(|o| matches!(o, Output::Line(LineKind::Error, _)))
        );
        h.type_line("like").await;
        h.finish_library().await;
        assert!(has_line(&h.outputs(), LineKind::Error, "sin red"));
        assert!(h.engine.playing.is_some());
        assert_eq!(h.liked(), None);
        assert!(!h.take_log().contains(&"stop".to_string()));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn boton_like_alterna_segun_el_corazon() {
        let mut h = harness(true);
        h.playing_track("t1").await;
        h.take_log();
        h.engine.on_input(Input::ToggleLike);
        h.finish_library().await;
        assert_eq!(h.take_log(), ["save t1"]);
        assert_eq!(h.liked(), Some(true));
        h.engine.on_input(Input::ToggleLike);
        h.finish_library().await;
        assert_eq!(h.take_log(), ["unsave t1"]);
        assert_eq!(h.liked(), Some(false));
        let outputs = h.outputs();
        assert!(has_line(&outputs, LineKind::Ok, "agregado a Tus me gusta"));
        assert!(has_line(&outputs, LineKind::Ok, "quitado de Tus me gusta"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn boton_shuffle_igual_que_el_comando() {
        let mut h = harness(true);
        h.type_line(&format!("play {}", album("a"))).await;
        h.confirm(1);
        h.engine.on_input(Input::Shuffle);
        h.engine.publish();
        assert!(has_line(&h.outputs(), LineKind::Ok, "Shuffle: sí"));
        assert!(h.engine.shown.as_ref().unwrap().shuffle);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn playlists_lista_y_elegir_reproduce() {
        let mut h = harness(true);
        h.type_line("playlists").await;
        let outputs = h.outputs();
        assert!(has_line(&outputs, LineKind::Dim, "Buscando tus playlists…"));
        assert!(has_line(&outputs, LineKind::Normal, "  1. p1 — de yo"));
        assert!(has_line(&outputs, LineKind::Normal, "  3. p3 — de yo"));
        assert!(outputs.contains(&Output::Prompt(Prompt::Choose(3))));
        h.type_line("9").await;
        assert!(has_line(&h.outputs(), LineKind::Warn, "del 1 al 3"));
        h.type_line("2").await;
        assert_eq!(h.take_log(), ["my-playlists", "connect 100 %", "play p2x0"]);

        h.type_line("pl").await;
        h.type_line("").await;
        assert_eq!(h.take_log(), ["my-playlists", "stop", "play p1x0"]);

        h.type_line("pl -s").await;
        h.type_line("3").await;
        assert!(h.engine.playing.as_ref().unwrap().queue.shuffled());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn playlists_vacia_avisa_y_no_espera_numero() {
        let mut h = harness(true);
        h.engine.backend = Rc::new(FakeBackend {
            playlists: (0, 0),
            ..Rc::into_inner(h.engine.backend).unwrap()
        });
        h.type_line("playlists").await;
        assert!(has_line(&h.outputs(), LineKind::Warn, "No tenés playlists"));
        assert!(h.engine.choosing.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn playlists_de_mas_se_cortan_en_el_tope_con_aviso() {
        let mut h = harness(true);
        let max = config::MY_PLAYLISTS_MAX;
        h.engine.backend = Rc::new(FakeBackend {
            playlists: (max, max + 7),
            ..Rc::into_inner(h.engine.backend).unwrap()
        });
        h.type_line("playlists").await;
        let outputs = h.outputs();
        assert!(has_line(&outputs, LineKind::Warn, "7 quedan afuera"));
        assert!(outputs.contains(&Output::Prompt(Prompt::Choose(max))));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn help_incluye_like_y_playlists() {
        let mut h = harness(true);
        h.type_line("help").await;
        let outputs = h.outputs();
        assert!(has_line(&outputs, LineKind::Normal, "like / unlike"));
        assert!(has_line(&outputs, LineKind::Normal, "playlists, pl"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn play_de_un_link_conecta_y_reproduce() {
        let mut h = harness(true);
        h.type_line(&format!("play {}", album("a"))).await;
        assert_eq!(h.take_log(), ["connect 100 %", "play ax0"]);
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
        assert_eq!(h.take_log(), ["connect 100 %", "play h2"]);

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
        assert_eq!(h.take_log(), ["connect 100 %"]);
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
        assert_eq!(h.take_log(), ["connect 100 %", "play ax0"]);
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

    fn volume_at(level: u8) -> Volume {
        let mut volume = Volume::default();
        volume.set(level);
        volume
    }

    #[tokio::test(flavor = "current_thread")]
    async fn volumen_sin_reproductor_se_aplica_al_conectar() {
        let mut h = harness(true);
        h.type_line("vol").await;
        let outputs = h.outputs();
        assert!(has_line(&outputs, LineKind::Normal, "Volumen: 100 %"));
        assert!(outputs.contains(&Output::Volume(Volume::default())));
        h.type_line("vol 30").await;
        let outputs = h.outputs();
        assert!(has_line(&outputs, LineKind::Ok, "Volumen: 30 %"));
        assert!(outputs.contains(&Output::Volume(volume_at(30))));
        assert!(h.take_log().is_empty());
        h.type_line(&format!("play {}", album("a"))).await;
        assert_eq!(h.take_log(), ["connect 30 %", "play ax0"]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn volumen_con_reproductor_y_despues_de_play_y_stop() {
        let mut h = harness(true);
        h.type_line(&format!("play {}", album("a"))).await;
        h.confirm(1);
        h.take_log();
        h.outputs();
        h.type_line("vol 50").await;
        h.engine.on_input(Input::VolumeUp);
        h.type_line("m").await;
        h.type_line("v -").await;
        assert_eq!(
            h.take_log(),
            [
                "volume 50 %",
                "volume 55 %",
                "volume silenciado (55 %)",
                "volume 50 %"
            ]
        );
        // El atajo no escribe en la consola; los comandos sí.
        let outputs = h.outputs();
        assert!(!has_line(&outputs, LineKind::Ok, "Volumen: 55 %"));
        assert!(has_line(&outputs, LineKind::Ok, "silenciado (55 %)"));
        // `play` nuevo, `stop` y `play` otra vez: mismo reproductor, mismo
        // volumen, sin volver a fijarlo.
        h.type_line(&format!("play {}", album("b"))).await;
        h.type_line("stop").await;
        h.type_line(&format!("play {}", album("c"))).await;
        assert_eq!(h.take_log(), ["stop", "play bx0", "stop", "play cx0"]);
        assert_eq!(h.engine.volume, volume_at(50));
    }

    /// Spec 007, AC-6: paso de volumen y umbral de "anterior" desde los
    /// ajustes, al instante.
    #[tokio::test(flavor = "current_thread")]
    async fn ajustes_de_reproduccion_cambian_paso_y_umbral() {
        let mut h = harness(true);
        h.type_line(&format!("play {}", album("a"))).await;
        h.confirm(1);
        h.type_line("n").await;
        h.confirm(2);
        h.type_line("vol 50").await;
        h.take_log();
        h.engine.on_input(Input::Playback(PlaybackSettings {
            volume_step: 20,
            previous_threshold: Duration::ZERO,
            ..PlaybackSettings::default()
        }));
        h.engine.on_input(Input::VolumeUp);
        h.type_line("v -").await;
        // Con umbral 0, "anterior" reinicia en vez de volver.
        std::thread::sleep(Duration::from_millis(5));
        h.type_line("prev").await;
        assert_eq!(h.take_log(), ["volume 70 %", "volume 50 %", "restart"]);
    }

    /// Spec 007, AC-6: otra calidad no toca lo que suena; el próximo `play`
    /// abre un reproductor nuevo con ella, y el siguiente lo reusa.
    #[tokio::test(flavor = "current_thread")]
    async fn calidad_nueva_desde_el_proximo_play() {
        let mut h = harness(true);
        h.type_line(&format!("play {}", album("a"))).await;
        h.confirm(1);
        h.take_log();
        h.engine.on_input(Input::Playback(PlaybackSettings {
            bitrate: Bitrate::Bitrate320,
            ..PlaybackSettings::default()
        }));
        assert!(h.take_log().is_empty(), "lo que suena sigue");
        h.type_line(&format!("play {}", album("b"))).await;
        assert_eq!(
            h.take_log(),
            ["connect 100 % Bitrate320", "stop", "play bx0"]
        );
        h.type_line(&format!("play {}", album("c"))).await;
        assert_eq!(h.take_log(), ["stop", "play cx0"]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn volumen_cambiado_mientras_conecta() {
        let mut h = harness(true);
        h.engine
            .on_input(Input::Line(format!("play {}", album("a"))));
        h.engine.on_input(Input::VolumeDown);
        h.settle().await;
        assert_eq!(h.take_log(), ["connect 100 %", "volume 95 %", "play ax0"]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn volumen_se_guarda_una_vez_y_avisa_si_falla() {
        let dir = std::env::temp_dir().join(format!(
            "spotify-terminal-test-{}-motor-volumen",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let mut h = harness(true);
        h.engine.volume_dir = Some(dir.clone());
        h.type_line("vol 40").await;
        h.engine.on_input(Input::VolumeUp);
        // Hasta que vence la espera no se escribe nada.
        assert!(h.engine.save_at.is_some());
        assert_eq!(Volume::load(&dir), Volume::default());
        h.engine.save_volume();
        assert!(h.engine.save_at.is_none());
        assert_eq!(Volume::load(&dir), volume_at(45));

        // Carpeta imposible (es un archivo): avisa y sigue.
        let file = dir.join(config::VOLUME_FILE);
        h.engine.volume_dir = Some(file);
        h.outputs();
        h.type_line("vol 10").await;
        h.engine.save_volume();
        assert!(has_line(
            &h.outputs(),
            LineKind::Warn,
            "No se pudo guardar el volumen"
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn al_salir_guarda_el_volumen_pendiente() {
        let dir = std::env::temp_dir().join(format!(
            "spotify-terminal-test-{}-motor-salir",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let mut h = harness(true);
        h.engine.volume_dir = Some(dir.clone());
        let (tx, rx) = mpsc::unbounded_channel();
        tx.send(Input::Line("vol 20".into())).unwrap();
        tx.send(Input::Line("exit".into())).unwrap();
        h.engine.run(rx).await;
        assert_eq!(Volume::load(&dir), volume_at(20));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn atajos_globales_con_algo_sonando_no_escriben() {
        let mut h = harness(true);
        h.type_line(&format!("play {}", album("a"))).await;
        h.confirm(1);
        h.take_log();
        h.outputs();
        for action in [
            GlobalAction::TogglePause,
            GlobalAction::Next,
            GlobalAction::Prev,
            GlobalAction::VolumeDown,
            GlobalAction::Stop,
        ] {
            h.engine.on_input(Input::Global(action));
        }
        h.engine.publish();
        assert_eq!(
            h.take_log(),
            ["pause", "play ax1", "play ax0", "volume 95 %", "stop"]
        );
        let outputs = h.outputs();
        assert!(!outputs.iter().any(|o| matches!(o, Output::Line(..))));
        // Stop vacía la barra, como el comando.
        assert!(outputs.contains(&Output::NowPlaying(None)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn atajos_globales_sin_nada_sonando_no_hacen_nada() {
        let mut h = harness(true);
        h.outputs();
        for action in [
            GlobalAction::TogglePause,
            GlobalAction::Next,
            GlobalAction::Prev,
            GlobalAction::Stop,
        ] {
            h.engine.on_input(Input::Global(action));
        }
        assert!(h.take_log().is_empty());
        assert!(!h.outputs().iter().any(|o| matches!(o, Output::Line(..))));
        // El volumen sí cambia, para cuando suene.
        h.engine.on_input(Input::Global(GlobalAction::VolumeUp));
        assert_eq!(h.engine.volume, volume_at(100));
        h.engine.on_input(Input::Global(GlobalAction::VolumeDown));
        assert_eq!(h.engine.volume, volume_at(95));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn help_lista_los_atajos_globales_activos() {
        let mut h = harness(true);
        h.type_line("help").await;
        assert!(!has_line(&h.outputs(), LineKind::Normal, "Atajos globales"));
        h.engine.on_input(Input::Shortcuts {
            window: vec!["Ctrl+F5          stop".into()],
            global: vec![
                "Ctrl+Alt+P  pausa / reanudar".into(),
                "Ctrl+Alt+→  siguiente".into(),
            ],
        });
        h.type_line("help").await;
        let outputs = h.outputs();
        assert!(has_line(&outputs, LineKind::Normal, "Atajos globales"));
        assert!(has_line(
            &outputs,
            LineKind::Normal,
            "Ctrl+Alt+→  siguiente"
        ));
        assert!(has_line(&outputs, LineKind::Normal, "Atajos de la ventana"));
        assert!(has_line(
            &outputs,
            LineKind::Normal,
            "Ctrl+F5          stop"
        ));
    }

    // --- Client ID (spec 008) ---

    const CLIENT_ID: &str = "0123456789abcdef0123456789abcdef";

    fn harness_without_client_id(saved_invalid: bool) -> Harness {
        let mut h = harness(true);
        Rc::get_mut(&mut h.engine.backend).unwrap().client_id =
            ClientIdStatus::Missing { saved_invalid };
        h.engine.on_open();
        h.engine.publish();
        h
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sin_client_id_al_abrir_muestra_la_guia_y_lo_pide() {
        let h = harness_without_client_id(false);
        let out = h.outputs();
        assert!(has_line(&out, LineKind::Normal, config::DASHBOARD_URL));
        assert!(has_line(&out, LineKind::Normal, config::REDIRECT_URI));
        assert!(out.contains(&Output::Prompt(Prompt::ClientId)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn client_id_guardado_invalido_avisa() {
        let h = harness_without_client_id(true);
        assert!(has_line(&h.outputs(), LineKind::Warn, "no es válido"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn con_client_id_al_abrir_no_pide_nada() {
        let mut h = harness(true);
        h.engine.on_open();
        h.engine.publish();
        assert!(!h.outputs().iter().any(|o| matches!(o, Output::Line(..))));
        assert_eq!(h.engine.shown_prompt, Prompt::Ready);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn client_id_invalido_insiste_sin_guardar() {
        let mut h = harness_without_client_id(false);
        h.outputs();
        // Un comando tampoco sale del modo: se valida como Client ID.
        for bad in ["hola", "play algo", ""] {
            h.type_line(bad).await;
        }
        assert!(has_line(&h.outputs(), LineKind::Warn, "Probá de nuevo"));
        assert!(h.take_log().is_empty());
        assert_eq!(h.engine.shown_prompt, Prompt::ClientId);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn client_id_valido_la_primera_vez_guarda_y_sigue_con_el_login() {
        let mut h = harness_without_client_id(false);
        h.type_line(&format!("  {CLIENT_ID}  ")).await;
        assert_eq!(
            h.take_log(),
            [format!("client-id {CLIENT_ID}"), "login".into()]
        );
        assert_eq!(h.engine.shown_prompt, Prompt::Ready);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn setup_guarda_y_sigue_con_el_login() {
        let mut h = harness(true);
        h.type_line("setup").await;
        assert_eq!(h.engine.shown_prompt, Prompt::ClientId);
        h.type_line(CLIENT_ID).await;
        assert_eq!(
            h.take_log(),
            [format!("client-id {CLIENT_ID}"), "login".into()]
        );
        assert_eq!(h.engine.shown_prompt, Prompt::Ready);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn setup_tapado_por_env_avisa_y_no_hace_login() {
        let mut h = harness(true);
        Rc::get_mut(&mut h.engine.backend).unwrap().applied = setup::Applied {
            changed: false,
            overridden_by_env: true,
        };
        h.type_line("setup").await;
        h.type_line(CLIENT_ID).await;
        assert_eq!(h.take_log(), [format!("client-id {CLIENT_ID}")]);
        assert!(has_line(&h.outputs(), LineKind::Warn, "tiene prioridad"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn esc_cancela_setup_sin_guardar() {
        let mut h = harness(true);
        h.type_line("setup").await;
        h.engine.on_input(Input::Cancel);
        h.engine.publish();
        assert!(has_line(&h.outputs(), LineKind::Dim, "no cambió"));
        assert!(h.take_log().is_empty());
        assert_eq!(h.engine.shown_prompt, Prompt::Ready);
        // Después de cancelar, las líneas vuelven a ser comandos.
        h.type_line(CLIENT_ID).await;
        assert!(h.take_log().iter().all(|l| !l.starts_with("client-id")));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn version_muestra_la_de_cargo() {
        let mut h = harness(true);
        h.type_line("version").await;
        assert!(has_line(
            &h.outputs(),
            LineKind::Normal,
            &format!("spotify-terminal {}", env!("CARGO_PKG_VERSION"))
        ));
    }
}
