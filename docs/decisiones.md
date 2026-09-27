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

