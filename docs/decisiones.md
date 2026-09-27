# Decisiones

Una entrada por decisión: fecha, decisión, alternativas, motivo, y
**Estado** (`Vigente` | `Reemplazada por <fecha/spec>`). No se borran
entradas viejas — se marcan reemplazadas.

## 2026-09-26 — Lenguaje: Rust
- **Decisión:** el cliente se escribe en Rust.
- **Alternativas:** Go (`go-librespot`), Python (con reproductor externo).
- **Motivo:** `librespot` es Rust y se embebe sin puente; binario único y el
  menor consumo de memoria.
- **Estado:** Vigente (spec 001)

## 2026-09-26 — Audio: `librespot` embebido en el mismo proceso
- **Decisión:** el cliente es a la vez dispositivo de reproducción, usando
  las crates de `librespot`, sin depender de otra app de Spotify abierta.
- **Alternativas:** controlar un dispositivo Spotify Connect existente vía
  Web API; `spotifyd` como proceso aparte.
- **Motivo:** un solo proceso liviano que reemplaza a la app oficial.
  Requiere Premium (confirmado). Respaldo si Spotify rompe `librespot`:
  controlar un dispositivo Connect vía Web API.
- **Estado:** Vigente (spec 001)

## 2026-09-26 — Consumo: RAM baja, pero la reproducción manda
- **Decisión:** objetivo ≤ 60 MB de RAM reproduciendo. Si reducir memoria
  (buffers de audio, caché) provoca cortes o demoras audibles, se prioriza
  la reproducción y se documenta el consumo real acá.
- **Motivo:** pedido explícito — "bajo, pero que no afecte la reproducción".
- **Estado:** Vigente (spec 001)

## 2026-09-26 — Sin agente mentor de Rust
- **Decisión:** se elimina `rust-mentor` y su registro de aprendizaje.
- **Motivo:** costo (tokens/tiempo) sin beneficio suficiente — pedido de vos.
  Se mantienen las reglas de calidad de Rust (`fmt`, `clippy`, sin `unwrap`).
- **Estado:** Vigente

## 2026-09-26 — La sesión de audio usa el Client ID de librespot (Web API: ver abajo)
- **Decisión:** la sesión de `librespot` se autentica con un token emitido
  para el Client ID de librespot (`65b708073fc0480ea92a077233ca87bd`).
- **Evidencia:** spike T2 del spec 001 — con nuestro Client ID, `login5`
  rechaza el token (`INVALID_CREDENTIALS`) y ningún tema carga.
- **Web API:** se eligió primero usar ese mismo token (un solo login).
  **Reemplazado** por la entrada "Dos tokens" de abajo.
- **Estado:** Vigente

## 2026-09-26 — Dos tokens: audio (librespot) y Web API (Client ID propio)
- **Decisión:** el token de audio usa el Client ID de librespot; el de la Web
  API usa el Client ID propio (`SPOTIFY_CLIENT_ID` en `.env`).
- **Evidencia:** T4 del spec 001 — con el Client ID de librespot, `GET /me`
  dio 429 permanente (6 intentos en 5 min respetando `Retry-After`); con el
  propio, 200. El Client ID de librespot lo comparten todos sus usuarios y su
  cuota de Web API está agotada.
- **Alternativa descartada:** usar solo APIs internas de librespot (sin Web
  API): no oficial, sin búsqueda documentada, likes/edición inciertos.
- **Costo:** la primera vez se autoriza dos veces; después, nunca.
- **Estado:** Vigente (reemplaza el "un solo login" de la entrada anterior)

## 2026-09-26 — Temas de álbum/playlist por la sesión de audio
- **Decisión:** `play` de un álbum o playlist obtiene la lista de temas con
  `librespot-metadata` sobre la sesión de audio ya abierta, no con la Web
  API.
- **Motivo:** no gasta cuota de la Web API ni pide scopes nuevos, y la
  sesión ya está abierta para reproducir. La crate ya era dependencia.
- **Alternativa:** `GET /albums/{id}/tracks` y `/playlists/{id}/items` con el
  token Web; queda para cuando haga falta paginar o mostrar más datos
  (spec de playlists).
- **Estado:** Vigente (spec 001, T7)

## 2026-09-26 — La sesión de audio corre en un hilo propio
- **Decisión:** la `Session` de librespot se crea y atiende en un hilo con
  su propio runtime tokio de un solo hilo (`player::AudioRuntime`), no en
  el runtime de `main`.
- **Motivo:** librespot lanza las tareas de la sesión en el runtime de quien
  la crea, y los pedidos de clave de audio tienen timeout de 1,5 s.
  Cualquier bloqueo de `main` (login, disco, render de la futura TUI)
  podía hacer fallar la carga de un tema. Gana la reproducción.
- **Costo:** un hilo más (stack chico, sin trabajo cuando no hay I/O).
- **Estado:** Vigente (spec 001, revisión de código)

## 2026-09-26 — Dependencias chicas agregadas en la revisión de código
- **`crossterm` feature `event-stream`** + **`futures-util`** (sin
  features): las teclas se leen como `Stream` dentro del `select!`. Reemplaza
  un hilo lector propio que quedaba vivo y se comía la próxima tecla.
- **`cpal`**: ver si hay dispositivo de salida antes de abrir la sesión
  (si no hay, rodio hace panic dentro de librespot).
- **`oauth2`** (solo dev): test que fija el texto de error del que depende
  la detección de "refresh token rechazado".
- Las tres ya venían como dependencias de librespot: no se compila nada
  nuevo.
- Se quitó `env_logger` (solo lo usaba el spike, que se borró).
- **Estado:** Vigente (spec 001)


## 2026-09-26 — Búsqueda: un pedido a `/search`, playlists con `limit` 10
- **Decisión:** `play <texto>` y `play list <texto>` hacen un solo
  `GET /search` con `market=from_token`. Temas: `limit=5`. Playlists:
  `limit=10` (el máximo en modo desarrollo) y se muestran las 5 primeras
  que no vienen `null`.
- **Motivo:** el spike (spec 002, T1) mostró que Spotify manda `null` en
  lugar de las playlists editoriales/algorítmicas (las apps en modo
  desarrollo no las ven): con `limit=5` a veces quedaban 1 o 2 resultados.
  Pedir 10 sigue siendo un solo pedido.
- **Limitación aceptada:** las playlists de Spotify ("Today's Top Hits",
  "This Is…", "Discover Weekly") no aparecen en la búsqueda. Se pueden
  reproducir igual con su link, porque la lista de temas va por la sesión
  de audio.
- **Estado:** Vigente (spec 002)

## 2026-09-26 — Cola de reproducción propia, no la de Spotify Connect
- **Decisión:** shuffle, siguiente/anterior y encolar se resuelven en
  `ui::playback::Queue`, que decide qué tema pedirle a librespot. No se usa
  `POST /me/player/queue` ni los endpoints de `/me/player/*`.
- **Motivo:** esos endpoints controlan un dispositivo Spotify Connect, y
  nuestro reproductor es librespot embebido sin Spotify Connect (spec 001):
  Spotify no conoce su cola. Además no gastan cuota de la Web API.
- **Costo:** la cola no se ve ni se maneja desde el celular ni desde la app
  oficial, y se pierde al salir.
- **Alternativa:** activar Spotify Connect en librespot (`Spirc`) y usar la
  cola de Spotify; más memoria y otro spec.
- **Estado:** Vigente (spec 003)

## 2026-09-26 — `rand` como dependencia directa
- **Decisión:** `rand` 0.9 para mezclar la cola (`SliceRandom::shuffle`).
- **Motivo:** ya la compila librespot con la misma versión y las mismas
  features: no suma código al binario.
- **Estado:** Vigente (spec 003)


## 2026-09-27 — App de escritorio: ventana propia en Rust
- **Decisión:** la app de escritorio es una ventana propia hecha en Rust
  (egui), que dibuja su propia consola.
- **Alternativas:** Windows Terminal con un perfil propio (no está
  instalado en la máquina de prueba y el look queda limitado al perfil);
  Tauri + xterm.js (WebView2: el más pesado, ~100 MB o más).
- **Motivo:** elegido por vos al pedir el spec 004: un solo `.exe` y
  control total del look.
- **Estado:** Vigente (spec 004)

## 2026-09-27 — La ventana se dibuja por CPU, sin GPU
- **Decisión:** egui 0.34 + `egui_software_backend` (rasteriza por CPU) +
  `softbuffer` (muestra la imagen con GDI) + `winit` (ventana). El loop de
  la ventana es nuestro (`desktop::window`); del crate solo se usa el
  rasterizador.
- **Evidencia (spike T5):** la misma ventana mínima con `eframe`/`glow`
  (OpenGL) ocupaba **120 MB** en reposo: el driver de NVIDIA carga
  `nvgpucomp64.dll` (~106 MB) y `nvoglv64.dll` (~47 MB). Por CPU:
  **19 MB**. Con la consola completa: 26 MB en reposo y 0 ms de CPU en
  10 s; reproduciendo 10 min, máximo 36,9 MB y 0,07 % de CPU promedio
  (AC-12).
- **Costos:** `egui_software_backend` es joven (0.0.3) y fija egui en 0.34.
  Cada redibujo cuesta CPU (con caché por zonas: solo se rehace lo que
  cambió). Por eso el cursor de texto no titila (titilar redibujaba todo
  cada medio segundo: ~6 % de un núcleo en reposo), y la barra de progreso
  se redibuja una vez por segundo solo mientras suena algo.
- **Sin bordes redondeados:** GDI no maneja transparencia por pixel; la
  ventana es rectangular con un borde de 1 px. Cambio respecto del supuesto
  del spec.
- **Respaldo:** si el crate se abandona o rompe, volver a `eframe`/`glow`
  cuesta ~100 MB más pero cambia solo `desktop::window`.
- **Estado:** Vigente (spec 004)

## 2026-09-27 — Dos ejecutables del mismo crate
- **Decisión:** `spotify-terminal.exe` (CLI, sin cambios de uso) y
  `spotify-desktop.exe` (ventana, subsistema "windows": sin consola negra
  detrás). La lógica está en la librería (`src/lib.rs`).
- **Motivo:** un `.exe` con subsistema "windows" pierde la salida de
  consola; no puede ser CLI y ventana a la vez.
- **Estado:** Vigente (spec 004)

## 2026-09-27 — Dependencias de la app de escritorio
- `egui` (con `default_fonts` como respaldo de símbolos), `egui-winit`
  (con portapapeles para copiar/pegar en la línea de entrada),
  `egui_software_backend` (solo el rasterizador), `winit`, `softbuffer`,
  `bytemuck` (ver el buffer de softbuffer como pixeles, sin `unsafe`).
- `windows-sys` 0.52 (la misma que ya trae winit) solo para `MessageBoxW`:
  mostrar un error fatal sin consola.
- `winresource` (solo al compilar, en Windows): ícono de los `.exe`.
- Fuente JetBrains Mono (licencia OFL, `assets/fonts/OFL.txt`) embebida:
  ~540 KB entre normal y negrita. Se ve igual en cualquier PC.
- El `.exe` de la ventana pesa ~9 MB (la CLI, ~4,5 MB).
- **Estado:** Vigente (spec 004)

## 2026-09-27 — El motor de la app de escritorio corre en su propio hilo
- **Decisión:** `app::engine` atiende comandos, eventos del reproductor y
  tareas de red en un hilo "motor" con runtime tokio de un hilo. La ventana
  (hilo principal) y el motor se hablan solo por canales.
- **Motivo:** la ventana tiene que seguir respondiendo mientras se busca o
  se espera un login, y el reproductor tiene que seguir recibiendo sus
  eventos (el siguiente tema no puede esperar a un redibujo).
- **Hallazgo:** `librespot-oauth` espera el callback del navegador con una
  llamada bloqueante dentro de una función `async`. En la CLI no importa;
  en el motor habría frenado la cola. Los tokens se piden con
  `spawn_blocking`. Si se cancela un login con Esc, ese hilo sigue
  esperando el callback hasta que se completa o se cierra la app (un
  segundo `login` avisaría que el puerto está ocupado).
- **Estado:** Vigente (spec 004)
