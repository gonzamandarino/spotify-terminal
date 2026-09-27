# Arquitectura

Vista de conjunto del sistema: qué módulo hace qué, quién depende de quién,
y qué reglas estructurales no se pueden romper. Se actualiza en el mismo
commit que cualquier cambio que la invalide — ver la regla de sincronización
en `CLAUDE.md`.

## Mapa de módulos y flujo de datos

El crate es una librería (`src/lib.rs`) con los módulos de abajo, y un
binario: la CLI (`src/main.rs`).

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
  └──> ui::playback::play_queue ──> app::queue::Queue (qué suena después)
                                 ├──> spotify::player (play/preload/pause/
                                 │    restart/stop, eventos del reproductor)
                                 └──> spotify::web::search (tecla `a`, como
                                      future dentro del loop, sin bloquearlo)

config  <── usado por todos (constantes, rutas, umbrales)
error   <── usado por todos (AppError con mensajes para el usuario)
```

## Hilos
- **main** (runtime `current_thread`): login, Web API, UI y teclado
  (`crossterm::EventStream`, sin hilo lector propio).
- **sesion-audio** (`player::AudioRuntime`): toda la I/O de la sesión de
  librespot. Está separado para que un bloqueo en `main` no haga fallar la
  carga de un tema (las claves de audio tienen timeout de 1,5 s).
- **librespot**: el reproductor crea sus propios hilos (decodificación,
  carga de temas, salida de audio).

## Quién posee qué estado
- **Caches de token:** solo `spotify::auth` los lee y escribe (escritura
  atómica: temporal + rename). El resto pide un token con
  `auth::get_valid_token(config, kind)`.
- **Sesión y reproductor:** `spotify::player::Player` es dueño de la sesión,
  del reproductor de librespot y del hilo de audio; al soltarse los cierra en
  ese orden. `main` lo crea y resuelve qué reproducir; `ui::playback` lo
  controla durante `play`.
- **Modo raw de la terminal:** `ui::RawMode` (lo usan `select` y
  `playback`); se restaura al soltarse o ante un panic.
- **Cola de reproducción:** `app::queue::Queue` (sin I/O, testeable sola)
  vive mientras dura `play_queue` y es la única dueña de qué suena después: orden de la lista
  (con o sin shuffle), temas encolados (`up_next`), historial para
  "anterior" (`timeline`) y el `play_request_id` vigente. Es nuestra, no
  de Spotify: la Web API no la ve.

## Reglas estructurales
- `spotify::web` solo recibe tokens `Web` y `spotify::player` solo `Audio`
  (con el token cruzado, la Web API da 429 y el audio no carga).
- Nada que use la sesión de librespot corre en el runtime de `main`: pasa por
  `AudioRuntime::run`.
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
| `player::Player` (incl. `restart`), `player::Resolved` | `src/spotify/player.rs` | doc-comment |
| `playback::play_queue` (teclas, cola, shuffle), `playback::restore_terminal_on_panic` | `src/ui/playback.rs` | doc-comment |
| `queue::Queue`, `queue::Step`, `queue::Clock` (crate) | `src/app/queue.rs` | doc-comment |
| `cli::parse`, `cli::Command`, `cli::USAGE` | `src/ui/cli.rs` | doc-comment |
| `select::choose` | `src/ui/select.rs` | doc-comment |
