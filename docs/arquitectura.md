# Arquitectura

Vista de conjunto del sistema: qué módulo hace qué, quién depende de quién,
y qué reglas estructurales no se pueden romper. Se actualiza en el mismo
commit que cualquier cambio que la invalide — ver la regla de sincronización
en `CLAUDE.md`.

## Mapa de módulos y flujo de datos

El crate es una librería (`src/lib.rs`) con los módulos de abajo, y dos
binarios: la CLI (`src/main.rs` → `spotify-terminal.exe`) y la app de
escritorio (`src/bin/desktop.rs` → `spotify-desktop.exe`, spec 004).

### CLI

```
main ──> ui::cli (parseo de subcomandos → Command)
  │
  ├──> ui::select (lista numerada de resultados de búsqueda → índice elegido)
  │
  ├──> spotify::auth ──> librespot-oauth ──> accounts.spotify.com
  │         │   TokenKind::Audio (Client ID librespot)
  │         │   TokenKind::Web   (Client ID propio, .env)
  │         └──> caches %APPDATA%\spotify-terminal\token-{audio,web}.json
  │
  ├──> spotify::web::WebClient (token Web) ──> api.spotify.com/v1
  │         (/me, /search)
  │
  ├──> spotify::player (token Audio): connect + resolve_tracks
  │         ├──> hilo "sesion-audio" (runtime propio): Session de librespot,
  │         │      metadata de álbum/playlist (librespot-metadata)
  │         └──> hilos de librespot: reproductor + salida de audio (rodio/WASAPI)
  │
  ├──> app::volume (carga el último volumen y lo guarda al salir)
  │
  └──> ui::playback::play_queue ──> app::queue::Queue (qué suena después)
                                 ├──> spotify::player (play/preload/pause/
                                 │    restart/stop, eventos del reproductor)
                                 └──> spotify::web::search (tecla `a`, como
                                      future dentro del loop, sin bloquearlo)

config  <── usado por todos (constantes, rutas, umbrales)
error   <── usado por todos (AppError con mensajes para el usuario)
```

### App de escritorio

```
bin/desktop ──> desktop::run
                  ├──> desktop::settings::load (ajustes.json → Settings)
                  ├──> desktop::window (winit + softbuffer, dibujo por CPU con
                  │      egui_software_backend; barra de título propia)
                  │      └──> desktop::app::DesktopApp (consola: salida,
                  │             entrada, historial, Tab, barra "sonando";
                  │             dueña de los Settings vivos)
                  │               ├──> desktop::menu (barra de menús y
                  │               │      diálogos: editan Settings)
                  │               ├──> desktop::theme (Settings → estilo,
                  │               │      fuente y zoom de egui)
                  │               ├──> desktop::settings::save (diferido)
                  │               └──> desktop::hotkeys::replace (al
                  │                      cambiar un atajo global)
                  │                 │ Input (línea, atajos, Esc,
                  │                 │ Playback, Shortcuts)
                  │                 ▼
                  ├──> desktop::hotkeys (hilo "atajos": RegisterHotKey)
                  │                 │ Input::Global (con la app minimizada)
                  │                 ▼
                  └──> app::engine (hilo "motor") ──Output──> DesktopApp
                         ├──> app::shell (línea → ShellCommand; reusa cli)
                         ├──> app::queue::Queue (misma cola que la CLI)
                         ├──> app::volume::Volume (mismo volumen que la CLI)
                         └──> app::backend::Backend (costura para tests)
                                └── SpotifyBackend ──> spotify::auth / web /
                                                       player (como la CLI)
```

## Hilos
- **main** (runtime `current_thread`): login, Web API, UI y teclado
  (`crossterm::EventStream`, sin hilo lector propio).
- **sesion-audio** (`player::AudioRuntime`): toda la I/O de la sesión de
  librespot. Está separado para que un bloqueo en `main` no haga fallar la
  carga de un tema (las claves de audio tienen timeout de 1,5 s).
- **librespot**: el reproductor crea sus propios hilos (decodificación,
  carga de temas, salida de audio).
- **App de escritorio:** el hilo principal es la ventana (winit + egui);
  el **motor** (`app::engine`) corre en su hilo con un runtime tokio de un
  hilo, y los tokens se piden en un hilo de bloqueo (`spawn_blocking`)
  porque librespot-oauth espera el callback del navegador con una llamada
  bloqueante. La ventana y el motor se hablan solo por canales; el motor
  despierta a la ventana con `request_repaint`.
- **atajos** (`desktop::hotkeys`, solo Windows): registra los atajos
  globales y duerme en `GetMessageW` hasta que se aprieta uno; entonces
  manda un `Input::Global` al motor por el mismo canal que la ventana. Se
  cierra (y los libera) antes de esperar al motor, porque también es
  emisor de ese canal. Al cambiar un atajo global desde el menú (spec
  007) el hilo se cierra y se abre otro con la lista nueva.

## Quién posee qué estado
- **Caches de token:** solo `spotify::auth` los lee y escribe (escritura
  atómica: temporal + rename). El resto pide un token con
  `auth::get_valid_token(config, kind)`.
- **Sesión y reproductor:** `spotify::player::Player` es dueño de la sesión,
  del reproductor de librespot y del hilo de audio; al soltarse los cierra en
  ese orden. En la CLI, `main` lo crea y resuelve qué reproducir y
  `ui::playback` lo controla durante `play`. En la app de escritorio es del
  motor (`app::engine`): se abre con el primer `play`, se reutiliza para
  los siguientes y se suelta con `logout`, si se cae, o al cerrar.
- **Volumen:** `app::volume::Volume` (nivel 0-100 y mute, sin I/O salvo
  `load`/`save`). En la CLI lo tiene `main` y `play_queue` lo cambia; en
  la app de escritorio es del motor, existe aunque no haya reproductor y
  se aplica al conectar. El reproductor solo recibe el valor
  (`Player::set_volume`). Se guarda en `volumen.txt` de la carpeta de
  datos: la CLI al salir si cambió, el motor 2 s después del último
  cambio (`config::VOLUME_SAVE_DELAY`) y al cerrar.
- **Modo raw de la terminal:** `ui::RawMode` (lo usan `select` y
  `playback`); se restaura al soltarse o ante un panic.
- **Cola de reproducción:** `app::queue::Queue` (sin I/O, testeable sola)
  vive mientras dura `play_queue` (CLI) o hasta el próximo `play`/`stop`
  (app de escritorio, dentro del motor) y es la única dueña de qué suena
  después: orden de la lista
  (con o sin shuffle), temas encolados (`up_next`), historial para
  "anterior" (`timeline`) y el `play_request_id` vigente. Es nuestra, no
  de Spotify: la Web API no la ve.
- **Consola de la app de escritorio:** `desktop::app::DesktopApp` es dueña
  del texto mostrado (con el tope de líneas de los ajustes), la línea de
  entrada y el historial. No guarda estado de reproducción propio: muestra
  el último `NowPlaying` que mandó el motor.
- **Ajustes de la app de escritorio (spec 007):** `DesktopApp` es dueña
  de los `Settings` vivos. `desktop::menu` los edita directamente y la
  consola compara antes/después de cada frame para aplicar lo que cambió:
  estilo y fuente (`theme`), atajos globales (`hotkeys::replace`),
  reproducción (`Input::Playback` al motor, que guarda su copia) y lista
  de atajos para `help` (`Input::Shortcuts`). Solo `desktop::settings`
  lee y escribe `ajustes.json` (escritura atómica: temporal + rename), 1 s
  después del último cambio (`config::SETTINGS_SAVE_DELAY`) y al cerrar.
  Los valores de `config.rs` son los defaults; la CLI usa siempre esos.

## Reglas estructurales
- `spotify::web` solo recibe tokens `Web` y `spotify::player` solo `Audio`
  (con el token cruzado, la Web API da 429 y el audio no carga).
- Nada que use la sesión de librespot corre en el runtime de `main`: pasa por
  `AudioRuntime::run`.
- `app::*` no depende de egui ni de la terminal: la CLI (`ui`) y la app de
  escritorio (`desktop`) son dos caras sobre la misma lógica. Solo
  `desktop` usa egui/winit.
- La app de escritorio no usa la GPU: nada de OpenGL/DirectX (el driver
  sumaba ~120 MB, ver `docs/decisiones.md`).
- Los temas de un álbum o playlist se piden por la sesión de audio
  (`Player::resolve_tracks`), no por la Web API (ver `docs/decisiones.md`).
- Los errores de las librerías se traducen a `AppError` en el módulo que los
  recibe (`auth`, `web`, `player`, `playback`). `main` solo arma los de su
  propia lógica (`NotPremium`), los imprime y sale con código 1.

## Contratos de funciones/módulos públicos
El contrato completo (pre/postcondiciones, invariantes, qué no debe hacer)
vive como doc-comment junto a cada función pública, no acá. Esta sección
solo linkea (tabla función → archivo) para no duplicar — si hay diferencia
entre esta tabla y el código, el código es la fuente de verdad y hay que
corregir la tabla.

| Función/módulo | Archivo | Contrato en |
|---|---|---|
| `Config::load`, constantes | `src/config.rs` | doc-comment |
| `AppError` (una variante por caso) | `src/error.rs` | doc-comment |
| `auth::get_valid_token`, `auth::logout`, `auth::Token`, `auth::TokenKind` | `src/spotify/auth.rs` | doc-comment |
| `web::WebClient` (`current_user`, `search`), `web::User`, `web::SearchKind`, `web::Hit` | `src/spotify/web.rs` | doc-comment |
| `player::Player` (incl. `restart`, `set_volume`), `player::Resolved` | `src/spotify/player.rs` | doc-comment |
| `playback::play_queue` (teclas, cola, shuffle), `playback::restore_terminal_on_panic` | `src/ui/playback.rs` | doc-comment |
| `queue::Queue`, `queue::Step`, `queue::Clock`, `queue::on_player_event` (crate) | `src/app/queue.rs` | doc-comment |
| `volume::Volume` (`load`, `save`, `output`) | `src/app/volume.rs` | doc-comment |
| `shell::parse_line`, `shell::complete`, `shell::ShellCommand`, `shell::VolumeCommand` (crate) | `src/app/shell.rs` | doc-comment |
| `engine::spawn`, `engine::Input`, `engine::Output`, `engine::Engine` (crate) | `src/app/engine.rs` | doc-comment |
| `backend::Backend`, `backend::Playback` (crate) | `src/app/backend.rs` | doc-comment |
| `hotkeys::spawn`, `hotkeys::replace`, `hotkeys::from_settings`, `hotkeys::Hotkeys`, `hotkeys::Shortcut` (crate) | `src/desktop/hotkeys.rs` | doc-comment |
| `combo::Combo`, `combo::Key` (crate; `Combo::from_str` con contrato) | `src/desktop/combo.rs` | doc-comment |
| `settings::Settings` (`from_json`, `to_json`, `check_combo`, `assign`, `restore`), `settings::load`, `settings::save` (crate) | `src/desktop/settings.rs` | doc-comment |
| `engine::PlaybackSettings` (crate) | `src/app/engine.rs` | doc-comment |
| `menu::Menus` (`keyboard`, `bar`, `dialogs`, `wants_keyboard`), `menu::Command` | `src/desktop/menu.rs` | doc-comment |
| `theme::Theme::apply`, `theme::installed_fonts` | `src/desktop/theme.rs` | doc-comment |
| `engine::GlobalAction` (crate) | `src/app/engine.rs` | doc-comment |
| `desktop::run`, `desktop::show_fatal_error` | `src/desktop/mod.rs` | doc-comment |
| `config::data_dir` | `src/config.rs` | doc-comment |
| `cli::parse`, `cli::parse_command`, `cli::Command`, `cli::USAGE` | `src/ui/cli.rs` | doc-comment |
| `select::choose` | `src/ui/select.rs` | doc-comment |
