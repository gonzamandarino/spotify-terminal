# 001 - Stack, reproductor de audio y login con Spotify

## Estado
`En implementación`

## Contexto
Es la base de todo el proyecto: sin lenguaje elegido, sin forma de emitir
audio y sin sesión autenticada contra Spotify no se puede implementar
ninguna de las funciones objetivo (playlists, likes, shuffle, elegir canción,
generador de playlists). Este spec fija esas tres decisiones y entrega lo
mínimo que las prueba de punta a punta: iniciar sesión una vez, reabrir el
cliente sin volver a loguearse, y reproducir/pausar un tema desde la
terminal.

El objetivo del proyecto es consumir menos recursos que la app oficial, así
que el consumo se mide desde este primer spec y no se deja "para después".

Referencia: ya existen clientes de terminal (`ncspot`, `spotify-player`,
ambos en Rust sobre `librespot`). No se reutilizan como dependencia directa,
pero sirven para comparar consumo y enfoques.

## Preguntas / Supuestos

- **¿Tenés Spotify Premium?** → Sí (respuesta de vos).
- **¿Lenguaje?** → Rust (respuesta de vos). Ver `docs/decisiones.md`.
- **¿Qué emite el audio?** → `librespot` embebido en el mismo proceso.
  Asumido: vos eligió Rust a partir de esta propuesta y no la objetó.
  Respaldo: controlar un dispositivo Spotify Connect vía Web API.
- **¿Presupuesto de recursos?** → "Bajo, pero que no afecte la
  reproducción" (respuesta de vos). Se traduce en: objetivo ≤ 60 MB de RAM
  reproduciendo; si bajar de ahí causa cortes, gana la reproducción y se
  documenta el consumo real. CPU < 2 % promedio en reproducción sin
  interacción (asumido).
- **¿Qué plataforma?** → Asumido: Windows 10 primero (máquina de
  desarrollo). Linux/macOS no se prueban en este spec.
- **¿Credenciales de la app de Spotify?** → **Dos tokens** (respuesta de
  vos, opción 2, tras T2 y T4): el de audio con el Client ID de librespot
  (constante en `config.rs`) y el de la Web API con el Client ID propio
  (`SPOTIFY_CLIENT_ID` en `.env`, redirect `http://127.0.0.1:8898/login`).
  Historia: primero se eligió un solo login con el Client ID de librespot
  (opción 1), pero en T4 la Web API respondió 429 permanente con ese ID.
  OAuth Authorization Code + PKCE vía `librespot-oauth`, sin client secret.
  Si un spec necesita un scope nuevo, el cliente detecta que el token
  cacheado no lo tiene y pide login de nuevo.
- **¿Qué UI tiene este spec?** → Asumido: comandos de línea mínimos
  (`login`, `logout`, `whoami`, `play <uri>`) y, durante `play`, teclas
  simples (espacio = pausa/reanudar, `q` = salir). La TUI real es otro spec
  (`tui-agent`); acá no se invierte en interfaz.
- **¿El token de nuestra app sirve para la sesión de `librespot`?** →
  *Riesgo abierto, se valida en la tarea T2 (spike):* hay reportes de que
  la sesión de streaming de `librespot` solo acepta tokens emitidos para
  ciertos client IDs. Si el nuestro no sirve, se usa el flujo OAuth propio
  de `librespot-oauth` para la sesión de audio y el nuestro solo para la
  Web API (dos tokens en el mismo cache). La decisión va a
  `docs/decisiones.md`.

## Qué debe pasar (no cómo)
1. La primera vez que se ejecuta el cliente, si no hay sesión guardada, abre
   el navegador (o muestra la URL) para autorizar con Spotify, recibe el
   callback en localhost y guarda el token localmente.
2. Las siguientes ejecuciones usan el token guardado y lo renuevan solo
   cuando vence, sin pedir login de nuevo.
3. Ya autenticado, el cliente muestra en la terminal el nombre del usuario
   y permite reproducir un tema dado (por URI o ID de Spotify), pausarlo y
   reanudarlo, con el audio saliendo por esta PC.
4. Existe un comando para cerrar sesión que borra el token guardado.
5. Si falta el `Client ID` o el login/la sesión fallan (cuenta sin
   Premium, sin red, login cancelado), el cliente sale con un mensaje claro
   que dice qué hacer, sin stack trace.

## Criterios de aceptación

- [x] **AC-1** — Con cache de token vacío, el cliente completa el login
      OAuth PKCE y crea el archivo de cache de token fuera del repo.
- [x] **AC-2** — Con cache válido, una segunda ejecución no pide login y
      muestra el nombre del usuario.
- [x] **AC-3** — Con access token vencido y refresh token válido, el cliente
      lo renueva solo y sigue funcionando sin intervención.
- [x] **AC-4** — Dado un URI de tema, el audio suena por esta PC; pausa y
      reanudación funcionan desde la terminal.
- [ ] **AC-5** — Reproducción continua de 30 minutos (varios temas
      seguidos por `play` de un álbum o playlist) sin cortes ni saltos
      audibles. Este criterio tiene prioridad sobre AC-6.
- [ ] **AC-6** — Consumo medido durante AC-5: RAM (working set) ≤ 60 MB y
      CPU < 2 % promedio. Si para cumplir AC-5 hace falta más RAM, se
      acepta con el valor real y el motivo registrados en
      `docs/decisiones.md` (en ese caso el AC se tilda con esa nota).
- [ ] **AC-7** — Sin `SPOTIFY_CLIENT_ID`, o si Spotify rechaza el login o
      la sesión (sin Premium, sin red, login cancelado), el cliente sale con
      código distinto de 0 y un mensaje que indica qué hacer.
- [x] **AC-8** — El comando `logout` borra el cache de token; la siguiente
      ejecución vuelve a pedir login.
- [x] **AC-9** — Ni `.env` ni el cache de token aparecen en `git status`
      después de loguearse.

## Riesgos / casos de falla

- **Envío de datos a terceros:** se autentica contra Spotify. Pedir solo los
  scopes necesarios para este spec (`user-read-private`, `streaming`,
  `user-read-playback-state`, `user-modify-playback-state`); los scopes de
  likes/playlists se agregan en sus specs.
- **Credenciales locales:** el refresh token da acceso a la cuenta. Se guarda
  solo en `%APPDATA%\spotify-terminal\`, nunca en logs ni en el repo.
- **Interrupción a mitad del login:** si se corta el callback, no debe quedar
  un cache corrupto (escritura atómica: archivo temporal + rename); la
  próxima ejecución reintenta el login desde cero.
- **Spotify API caída o sin red de forma sostenida:** el cliente informa el
  error y sale, o reintenta con backoff exponencial acotado (máximo de
  intentos en config), sin loop agresivo que consuma CPU/red.
- **Cambios de política de Spotify:** `librespot` no es oficial y Spotify
  puede romperlo. Se fija la versión exacta en `Cargo.lock` y se registra
  el respaldo en `docs/decisiones.md`.
- **Toolchain en Windows:** `librespot` y sus dependencias (TLS, audio)
  compilan con el toolchain MSVC; si falta algo (Build Tools de Visual
  Studio, etc.) se documenta en el README como prerequisito.

## Plan técnico
*(propuesta — pendiente de revisión de vos antes de escribir código)*

- **Archivos que toca:**
  - `Cargo.toml`, `Cargo.lock` — crate binaria `spotify-terminal`, edición
    2021, perfil release con `opt-level = "s"`, `lto = true`,
    `codegen-units = 1` (binario chico, menos memoria).
  - `.env.example` — `SPOTIFY_CLIENT_ID=` sin valor.
  - `src/main.rs` — parseo de subcomandos y arranque del runtime `tokio`.
  - `src/config.rs` — carga de `.env` y constantes configurables (puerto de
    callback, ruta de cache, scopes, bitrate, reintentos). Único lugar de
    umbrales/constantes.
  - `src/error.rs` — tipo de error de la app con mensajes para el usuario.
  - `src/spotify/mod.rs`, `src/spotify/auth.rs` — OAuth PKCE, cache de
    token, refresh.
  - `src/spotify/player.rs` — sesión `librespot` + reproductor.
  - `src/spotify/web.rs` — llamadas a la Web API (solo `GET /me` en este
    spec).
  - `src/ui/cli.rs` — subcomandos y lectura de teclas durante `play`.
  - `scripts/medir-consumo.ps1` — muestrea working set y CPU del proceso
    cada 5 s durante N minutos y resume promedio/máximo (para AC-5/AC-6).
  - `README.md` — prerequisitos (rustup, Build Tools), cómo crear la app en
    Spotify Dashboard, cómo correr.
- **Crates (versiones a fijar en T1, verificando la última estable):**
  `librespot-core`, `librespot-playback` (backend `rodio`),
  `librespot-oauth`, `librespot-metadata` si hace falta; `tokio`
  (features mínimas: `rt`, `macros`, `sync`, `time`), `reqwest` con
  `rustls` o el cliente HTTP que ya trae `librespot` para evitar dos stacks
  TLS; `serde`/`serde_json` para el cache; `dotenvy`; `directories` para
  `%APPDATA%`; `crossterm` para teclas; `thiserror`. Sin `clap`
  (subcomandos a mano: son 4, no justifican la dependencia).
- **Funciones/estructuras nuevas:**
  - `config::Config::load() -> Result<Config, AppError>`
  - `auth::TokenCache` (serializable) y
    `auth::get_valid_token(&Config) -> Result<Token, AppError>` (usa cache,
    refresca si vence, si no hay cache corre el login).
  - `auth::logout(&Config) -> Result<(), AppError>`
  - `player::Player::connect(&Config, &Token)`, `.load(uri)`, `.pause()`,
    `.resume()`, `.stop()`
  - `web::current_user(&Token) -> Result<User, AppError>`
- **Tests necesarios:**
  - Unitarios: `Config::load` sin `Client ID` → error esperado (AC-7);
    `TokenCache` expira correctamente según `expires_at` (base de AC-3);
    serialización/escritura atómica del cache; parseo de URI/ID de tema.
  - Manuales (evidencia en "Notas de verificación"): AC-1, AC-2, AC-4,
    AC-5, AC-6 (con `scripts/medir-consumo.ps1`), AC-8, AC-9. AC-3 se
    fuerza editando `expires_at` del cache a una fecha pasada.
- **Contratos y arquitectura:** doc-comments con Pre/Post/Invariantes en
  las funciones públicas de `auth`, `player`, `web` y `config`. Se
  completa `docs/arquitectura.md` con el primer mapa de módulos:
  `ui → spotify::{auth, player, web} → config`, y la regla de que solo
  `auth` lee/escribe el cache de token.

## Tareas
*(desglose del plan, se van tildando)*

- [x] T0 — Instalar `rustup` (toolchain stable MSVC) y Build Tools de Visual
      Studio; verificar `cargo --version`
- [x] T1 — `cargo init`, `Cargo.toml` con perfil release y crates fijadas,
      `.env.example`, `config.rs` + `error.rs` con tests (AC-7)
- [x] T2 — Spike: sesión `librespot` reproduciendo un URI fijo con un token
      obtenido a mano; resolver el riesgo de client ID y registrarlo en
      `docs/decisiones.md`
- [x] T3 — `auth.rs`: login PKCE + callback local + cache atómico + refresh
      + logout, con tests (AC-1, AC-3, AC-8)
- [x] T4 — `web.rs`: `current_user` + subcomando `whoami` (AC-2)
- [x] T5 — `player.rs` + subcomando `play` con pausa/reanudar por teclado
      (AC-4)
- [ ] T6 — Mensajes de error de usuario (sin Premium, sin red, login
      cancelado) (AC-7)
- [ ] T7 — `scripts/medir-consumo.ps1` + medición de 30 min (AC-5, AC-6),
      ajustando buffers si hay cortes
- [ ] T8 — `README.md`, `docs/arquitectura.md`, changelog, verificación de
      `git status` (AC-9)

## Definition of Done

- [ ] Todos los AC tildados, o el estado es `Reabierto (parcial)` con el
      motivo explícito
- [ ] Tests corren y pasan
- [ ] Contratos de funciones públicas y doc de arquitectura actualizados si
      el spec cambió una firma, comportamiento o el mapa de módulos
- [ ] Decisiones de diseño relevantes documentadas (lenguaje, reproductor,
      flujo OAuth, client ID de la sesión de audio) en `docs/decisiones.md`
- [ ] Changelog actualizado
- [ ] Sin constantes/umbrales hardcodeados fuera de su lugar de config
- [ ] Sin secretos ni credenciales en el diff
- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test` pasan

## Notas de verificación
*(al cerrar: qué se probó y resultado — por AC cuando no sea obvio)*

- **T0** (2026-09-26): rustc/cargo 1.98.1, toolchain
  `stable-x86_64-pc-windows-msvc`; crate de prueba compila y enlaza con MSVC.
- **T1 — desvíos del plan (menores):** edición 2024 en lugar de 2021 (es la
  actual y la que usa el Book); las crates se agregan en la tarea que las
  usa en vez de todas en T1, para no compilar dependencias sin uso. Versiones
  relevadas en crates.io el 2026-09-26: `librespot-*` 0.8.0 (TLS
  `native-tls` → SChannel en Windows, sin OpenSSL), `tokio` 1.53.1,
  `crossterm` 0.29.0, `serde` 1.0.229, `reqwest` 0.13.5. `.env` usaba la
  clave `CLIENT_ID`; se renombró a `SPOTIFY_CLIENT_ID` (valor intacto).
- **AC-7 (versión original, ya reemplazada):** binario corrido desde una
  carpeta sin `.env` y sin la variable → mensaje con instrucciones, `exit=1`. Cubierto también por los tests
  `client_id_ausente_es_error` y `client_id_vacio_o_con_espacios_es_error`.
- **T2 — spike (2026-09-26), `examples/spike_librespot.rs`:**
  - Con **nuestro** Client ID: el login OAuth y `Session::connect` funcionan
    (el AP autentica al usuario), pero al cargar audio `login5` responde
    `FaultyRequest(INVALID_CREDENTIALS)` → el tema queda `Unavailable`. El
    riesgo abierto se confirmó.
  - Con el Client ID de librespot (`65b7…87bd`, mismo redirect
    `http://127.0.0.1:8898/login`): carga y reproduce (evento `Playing`,
    salida WASAPI a los auriculares).
  - Consumo reproduciendo (build debug): working set 25 MB, memoria privada
    8 MB, CPU 0,33 % promedio en 10 s.
  - Estrategia de tokens (uno vs. dos Client IDs): ver `docs/decisiones.md`.
- **T3 (2026-09-26):** `src/spotify/auth.rs` + subcomandos `login`/`logout`.
  Se usa `librespot-oauth` (PKCE + callback local + refresh) en vez de
  implementar el flujo a mano. 9 tests unitarios pasan; `fmt`/`clippy` OK.
  - **AC-1:** cache borrado → `login` abre el navegador → `token.json` creado
    en `%APPDATA%\spotify-terminal\` (fuera del repo), `exit=0`.
  - **AC-3:** `expires_at` del cache puesto 10 s en el pasado → `login` sin
    navegador, access token distinto, vence en 3598 s, refresh token
    conservado. El token renovado trae todos los scopes del Client ID de
    librespot (library, playlists, playback...), no solo los pedidos.
  - **AC-8:** `logout` → cache borrado; segundo `logout` → "No había una
    sesión guardada"; `login` siguiente vuelve a pasar por el navegador.
  - **AC-9:** `git status --ignored` después del login: `.env` ignorado (ya no
    se usa, se puede borrar), el cache vive fuera del repo.
- **T4 (2026-09-26):** `src/spotify/web.rs` (`GET /me`) + `whoami`.
  - Con el token del Client ID de librespot, `/me` devolvió **429 Too Many
    Requests** 6 veces seguidas en 5 minutos respetando `Retry-After`
    (56–59 s): la cuota de ese Client ID compartido está agotada. Con un
    token del Client ID propio, el mismo pedido dio 200. → Se pasó a dos
    tokens (opción 2): `auth::TokenKind::{Audio, Web}`, caches
    `token-audio.json` y `token-web.json`; `login` autoriza ambos, `logout`
    borra ambos. Vuelve `.env.example`.
  - **AC-2:** después de `login`, `whoami` → "Gonza Mandarino (…), plan:
    premium" sin abrir el navegador. 16 tests OK, `fmt`/`clippy` OK.
- **T5 / AC-4 (2026-09-26):** `play https://open.spotify.com/track/4uLU…`
  → "♪ Never Gonna Give You Up — Rick Astley", el audio sale por la PC;
  espacio pausa y reanuda, `q` sale ("Fin."). Confirmado por vos.
- **T6 (2026-09-26, código):** cada falla tiene su variante de `AppError`
  con instrucciones, y `main` sale con código 1:
  - login cancelado en el navegador → `LoginCancelled`; puerto 8898
    ocupado → `LoginPortBusy`; sin red → `Network`. Mientras espera el
    callback avisa que se corta con Ctrl+C.
  - sin red al renovar el token → `Network`. Antes se abría el navegador
    para loguearse de nuevo, que tampoco iba a andar. Solo un rechazo de
    Spotify (`invalid_grant`) lleva al login.
  - Web API: 401 → `SessionRejected` (logout + login), 429 →
    `RateLimited` con `Retry-After`, 5xx → "problema de Spotify".
  - sesión de audio: credenciales rechazadas → `SessionRejected`; el resto
    → `Network`. Tema no disponible, álbum inexistente y salida de audio
    ausente tienen su propio mensaje.
  - Probado a mano: `play spotify:artist:…` → uso + `exit=1`;
    `play spotify:album:000…` → "no se encontró el álbum…", `exit=1`.
    La primera versión mostraba "revisá la conexión" en ese caso, así que
    se separó `metadata_error` de `session_error`.
- **T7 (2026-09-26, código):** `play` acepta álbumes y playlists (URI o
  link). Los temas se piden con `librespot-metadata` por la sesión de audio
  (ver `docs/decisiones.md`); la cola precarga el siguiente tema
  (`TimeToPreloadNextTrack`) para que no haya silencio entre temas. Un tema
  no disponible dentro de una lista se saltea con aviso.
  `scripts/medir-consumo.ps1` probado contra otro proceso (1 min, OK).

