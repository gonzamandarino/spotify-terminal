# 011 - Likear el tema que suena y elegir entre mis playlists

## Estado
En plan

## Contexto
Dos funciones objetivo de `CLAUDE.md` que todavía no existen:

1. **Likear / deslikear** el tema que está sonando (biblioteca "Tus me
   gusta"). Hoy para eso hay que ir a la app oficial.
2. **Ver mis playlists** y elegir una para reproducir. Hoy solo se puede
   `play list <nombre>` (búsqueda pública, spec 002) o pegar un link: no
   hay forma de ver las playlists propias y las que sigo.

**Factibilidad (changelog de la Web API, feb 2026):**
- `PUT /me/tracks`, `DELETE /me/tracks` y `GET /me/tracks/contains` se
  **eliminaron**; los reemplazan `PUT /me/library`, `DELETE /me/library` y
  `GET /me/library/contains`, que reciben URIs de Spotify
  ([changelog feb 2026](https://developer.spotify.com/documentation/web-api/references/changes/february-2026)).
  Scopes: `user-library-modify` (guardar/quitar) y `user-library-read`
  (consultar).
- `GET /me/playlists` sigue disponible (paginado, máx. 50 por página).
  Scopes: `playlist-read-private` y `playlist-read-collaborative`.
- Los objetos playlist ya no traen `tracks` sino `items`, y solo para
  playlists del usuario; las demás traen solo metadatos. Para listar
  alcanza con nombre, dueño, cantidad de temas (si viene) y URI; la
  reproducción sigue por el camino actual de `play <link>`.

Hoy `config::WEB_SCOPES` tiene solo `user-read-private`. Agregar scopes
hace que el token cacheado deje de alcanzar y se pida login de nuevo una
vez (`get_valid_token` ya lo maneja).

## Preguntas / Supuestos

- **¿Dónde viven los comandos?** → Asumido: en la consola de la app de
  escritorio (`app::shell`), como `queue`. La CLI suelta no se toca.
- **¿`like` alterna o solo likea?** → Asumido: `like` solo agrega y
  `unlike` solo quita. Motivo: un comando que alterna, escrito dos veces
  por error, deslikea sin querer y no se ve. (El botón del spec 012 sí
  alterna, porque muestra el estado.)
- **¿Se muestra si el tema que suena ya está likeado?** → Asumido: sí, un
  ♥ en la barra "sonando" (lleno si está en Tus me gusta, apagado si no).
  El estado se consulta una vez por cambio de tema (`/me/library/contains`)
  y se actualiza al likear/deslikear desde la app. Si se cambia desde otra
  app mientras suena, se ve al siguiente tema (sin polling).
- **¿Qué pasa con un episodio de podcast?** → Asumido: `like` avisa que
  solo se pueden likear temas y no hace nada. Motivo: "Tus me gusta" es de
  temas; los episodios van a otra sección.
- **¿Qué es "mis playlists guardadas"?** → Asumido: lo que devuelve
  `GET /me/playlists`: las creadas por mí y las que sigo, en el orden de
  Spotify. "Tus me gusta" **no** aparece (no es una playlist en la API) —
  reproducirla queda para otro spec.
- **¿Cómo se elige?** → Asumido: `playlists` (alias `pl`) lista todas,
  numeradas, con nombre, dueño y cantidad de temas si la API la da; se
  elige escribiendo el número, igual que en la búsqueda (spec 002):
  Enter = la primera, otro comando cancela, `Esc` cancela. `playlists -s`
  arranca mezclado, como `play -s`.
- **¿Y si tengo muchas?** → Asumido: se traen todas las páginas (50 por
  pedido) hasta un tope de config (`MY_PLAYLISTS_MAX`, 500) para no
  llenar la consola ni hacer pedidos sin fin. Si se llega al tope, se avisa
  cuántas quedaron afuera. Filtrar por nombre (`playlists <texto>`) queda
  afuera de este spec.
- **¿Se cachea la lista?** → Asumido: no; se pide cada vez que se escribe
  el comando (es un pedido explícito, no polling, y así ve cambios).

## Qué debe pasar (no cómo)

Con algo sonando, `like` guarda el tema en Tus me gusta y la consola lo
confirma; `unlike` lo quita. La barra "sonando" muestra si el tema actual
está likeado. `playlists` muestra la lista numerada de mis playlists y al
elegir un número empieza a sonar esa playlist como con `play <link>`.

## Criterios de aceptación

- [ ] **AC-1** — Con un tema sonando (o en pausa), `like` lo agrega a Tus
      me gusta (se ve en la app oficial) y la consola muestra
      `♥ <tema> — agregado a Tus me gusta`.
- [ ] **AC-2** — `like` sobre un tema que ya estaba likeado no falla: avisa
      que ya estaba y no hace un pedido de guardado.
- [ ] **AC-3** — `unlike` quita el tema actual de Tus me gusta y lo
      confirma; sobre uno que no estaba, avisa y no hace nada.
- [ ] **AC-4** — Sin nada sonando, `like` / `unlike` avisan
      `Nada sonando` y no hacen pedidos. Con un episodio, avisan que solo se
      likean temas.
- [ ] **AC-5** — La barra "sonando" muestra ♥ lleno si el tema actual está
      en Tus me gusta y apagado si no; cambia al instante tras `like` /
      `unlike` y se recalcula en cada cambio de tema con un solo pedido.
- [ ] **AC-6** — `playlists` (y `pl`) lista mis playlists numeradas desde 1,
      con nombre, dueño y cantidad de temas cuando la API la informa.
      Mientras carga se ve `Buscando tus playlists…`.
- [ ] **AC-7** — Escribir un número válido reproduce esa playlist igual que
      `play <link de la playlist>` (misma barra, cola, siguiente/anterior);
      Enter solo elige la 1; un número fuera de rango avisa y sigue
      esperando; otro comando o `Esc` cancela. `playlists -s` arranca
      mezclado.
- [ ] **AC-8** — Con más de 50 playlists se ven todas (hasta
      `MY_PLAYLISTS_MAX`); si hay más que el tope, se avisa cuántas no se
      muestran. Sin playlists, se avisa y no queda esperando un número.
- [ ] **AC-9** — La primera vez tras actualizar, el token viejo (sin los
      scopes nuevos) lleva a login una sola vez; después `like` y
      `playlists` andan sin volver a pedirlo.
- [ ] **AC-10** — Errores de red, 401/403 o 429 se muestran en la consola
      con el mensaje de siempre (`status_error`) y no cortan lo que suena.
- [ ] **AC-11** — `help` y Tab (completar) incluyen `like`, `unlike` y
      `playlists`.

## Riesgos / casos de falla

- **Escritura en la cuenta real:** `like`/`unlike` modifican la biblioteca
  del usuario. Solo actúan sobre el tema que suena en ese momento (nunca
  en lote) y cada cambio se confirma en la consola. `unlike` es un comando
  aparte para que no pase por error.
- **Tema cambia mientras se likea:** el pedido lleva la URI del tema que
  sonaba al escribir el comando, no la que suene cuando vuelva la
  respuesta; el ♥ solo se actualiza si esa URI sigue siendo la actual.
- **API caída o lenta sostenida:** los pedidos van fuera del hilo de
  reproducción; el audio no espera a la Web API. Si falla la consulta de
  `contains`, el ♥ queda en "desconocido" (apagado) sin avisar en cada tema.

## Plan técnico

- Archivos que toca: `src/config.rs`, `src/spotify/web.rs`,
  `src/app/backend.rs`, `src/app/queue.rs`, `src/app/shell.rs`,
  `src/app/engine.rs`, `src/desktop/app.rs` (solo el ♥ en la barra),
  `docs/arquitectura.md`, `docs/decisiones.md`, `CHANGELOG.md`,
  `Cargo.toml`/`Cargo.lock`.
- Funciones/estructuras nuevas:
  - `config`: `WEB_SCOPES` suma `user-library-read`,
    `user-library-modify`, `playlist-read-private`,
    `playlist-read-collaborative`; `MY_PLAYLISTS_PAGE` (50) y
    `MY_PLAYLISTS_MAX` (500).
  - `web::WebClient::my_playlists(max) -> MyPlaylists { hits, total }`:
    pagina `GET /me/playlists` hasta que no haya `next` o se llegue a
    `max`. La conversión playlist → `Hit` se comparte con `search`.
  - `web::WebClient::is_saved(uri) -> bool` (`GET /me/library/contains`)
    y `set_saved(uri, bool)` (`PUT` / `DELETE /me/library?uris=`). Un
    helper `send_empty` para pedidos sin cuerpo de respuesta, con el mismo
    manejo de errores que `get_json` (`status_error`).
  - `Backend`: `my_playlists`, `is_saved`, `set_saved` (y en el
    `FakeBackend` de los tests).
  - `queue::TrackInfo.uri` (el `track_id` del `TrackChanged`).
  - `shell::ShellCommand::{Like, Unlike, Playlists { shuffle }}`;
    `playlists`/`pl` aceptan `-s`. `HELP` y `NAMES` al día.
  - `engine`: `Playing` guarda `uri` y `liked: Option<bool>` (`None` =
    desconocido); `NowPlaying.liked`. Dos carriles nuevos, aparte de
    `task` (como `cover_task`), para que no choquen con una búsqueda o un
    `play`: `check_task` (consulta ♥; se reemplaza en cada cambio de tema)
    y `save_task` (like/unlike; uno a la vez, otro mientras tanto avisa
    "esperá"). Los resultados llevan la URI con la que se pidieron y solo
    tocan el ♥ si sigue siendo el tema actual. `playlists` usa `task` con
    `Done::MyPlaylists` y reutiliza `choosing` con
    `Purpose::Play { shuffle }`.
  - AC-2: si el ♥ todavía es desconocido (consulta en curso o fallida),
    `like` hace el `PUT` igual (es idempotente) y confirma "agregado".
- Tests necesarios:
  - `web`: parseo de una página de `/me/playlists` (con `null`, `next`,
    formato viejo `tracks`), de la respuesta de `contains`.
  - `shell`: `like`, `unlike`, `playlists`, `pl -s`, argumentos de más.
  - `engine` (fake): like/unlike con ♥ conocido y desconocido, ya estaba /
    no estaba sin pedido (AC-2, AC-3), nada sonando y episodio (AC-4),
    cambio de tema recalcula ♥ y descarta respuestas viejas (AC-5),
    `playlists` lista, elegir reproduce, Enter = 1, fuera de rango, vacía,
    tope con aviso (AC-6..AC-8), error de API no corta lo que suena
    (AC-10).
  - AC-1, AC-9 y la parte visual de AC-5: prueba manual con la cuenta.
- Contratos a crear/actualizar: doc-comments de los métodos nuevos de
  `WebClient` y `Backend`, `NowPlaying`, invariantes de `Engine` (carriles
  nuevos); `docs/arquitectura.md` (endpoints nuevos de `web`, scopes).
  `docs/decisiones.md`: `/me/library` en vez de `/me/tracks`, `like` y
  `unlike` separados.
- Versión que publica: `0.3.0` (spec nuevo)

## Tareas

- [ ] T1 — `config`: scopes y constantes nuevas.
- [ ] T2 — `web`: `my_playlists`, `is_saved`, `set_saved`, `send_empty` +
      tests de parseo.
- [ ] T3 — `backend`: métodos nuevos en `Backend`, `SpotifyBackend` y el
      fake.
- [ ] T4 — `queue`: `TrackInfo.uri`.
- [ ] T5 — `shell`: comandos, `HELP`, `NAMES` + tests.
- [ ] T6 — `engine`: like/unlike con `check_task`/`save_task` y ♥ en
      `NowPlaying` + tests.
- [ ] T7 — `engine`: `playlists` + tests.
- [ ] T8 — `desktop`: ♥ en la barra "sonando".
- [ ] T9 — Docs, contratos, changelog 0.3.0, versión; fmt/clippy/test.
- [ ] T10 — Prueba manual con la cuenta (AC-1, AC-5, AC-9).

## Definition of Done

- [ ] Todos los AC tildados, o el estado es `Reabierto (parcial)` con el
      motivo explícito
- [ ] Tests corren y pasan
- [ ] Contratos de funciones públicas y `docs/arquitectura.md` actualizados
      si el spec cambió una firma, comportamiento o el mapa de módulos
- [ ] Decisiones de diseño relevantes documentadas en `docs/decisiones.md`
- [ ] Changelog actualizado: sección `## [0.3.0] - fecha`
- [ ] Versión subida en `Cargo.toml` y `Cargo.lock`; el Release lo publica
      CI al mergear
- [ ] Sin constantes/umbrales hardcodeados fuera de `src/config.rs`
- [ ] Sin secretos ni credenciales en el diff
- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test` pasan

## Notas de verificación
