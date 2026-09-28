# 013 - Reproducir mis me gusta

## Estado
En verificación

## Contexto
El spec 011 dejó afuera reproducir "Tus me gusta": no es una playlist en la
API, así que no aparece en `playlists` y no tiene link para `play <link>`.
Es la lista que más se escucha y hoy solo se puede desde la app oficial.

**Factibilidad (changelog de la Web API, feb 2026):**
- `GET /me/tracks` **sigue disponible** (lo eliminado fue `PUT`/`DELETE`
  `/me/tracks` y `contains`, ver spec 011). Paginado, máx. 50 por pedido,
  ordenado del más reciente al más viejo. Scope `user-library-read`, que
  ya está en `config::WEB_SCOPES` desde el spec 011: no hace falta volver
  a loguearse.
- librespot no expone la colección como contexto (`Playlist::get` no
  sirve): la lista de URIs se arma con la Web API y se entrega a la cola
  igual que los temas de una playlist resuelta.
- Costo: un pedido por cada 50 temas. 1000 me gusta = 20 pedidos, una
  sola vez por comando (no es polling).

## Preguntas / Supuestos

- **¿Cómo se pide?** → Asumido: comando `likes` en la consola, con `-s`
  para arrancar mezclado (como `play -s` y `playlists -s`). No se agrega
  como ítem de `playlists`. Motivo: meterlo en esa lista corre la
  numeración del spec 011 y mezcla algo que no es una playlist.
- **¿En qué orden?** → Asumido: el de la app oficial, del más reciente al
  más viejo.
- **¿Se espera a tener todos los temas antes de que suene?** → Asumido:
  sí. Se traen todas las páginas y después arranca, como una playlist.
  Mientras tanto se ve `Cargando tus me gusta… N/total`. Motivo: el
  shuffle (`-s` o `s`) necesita la lista entera y la cola del spec 003 no
  está pensada para crecer mientras suena. Arrancar con la primera página
  y seguir cargando de fondo queda para otro spec si la espera molesta.
- **¿Y si tengo muchísimos?** → Asumido: tope en config (`LIKES_MAX`,
  2000 = 40 pedidos). Si hay más, se reproducen los 2000 más recientes y
  se avisa cuántos quedaron afuera (igual que `MY_PLAYLISTS_MAX`).
- **¿Temas locales o no disponibles?** → Asumido: los que la API marca
  como locales se omiten y se cuentan en el aviso, como en una playlist.
  Los no disponibles en la región siguen el camino de siempre (el
  reproductor los salta, `MAX_CONSECUTIVE_UNAVAILABLE`).
- **¿Si likeo/deslikeo mientras suena?** → Asumido: la lista es una foto
  del momento en que se escribió `likes`. `like`/`unlike` cambian el ♥
  pero no agregan ni sacan temas de lo que está sonando.
- **¿Si otro comando llega mientras carga?** → Asumido: igual que con
  `play`: la carga usa el carril `task` del motor y lo que ya sonaba
  sigue sonando hasta que la lista nueva esté lista.

## Qué debe pasar (no cómo)

`likes` carga los temas de Tus me gusta y los reproduce en el mismo orden
que la app oficial, con la barra, la cola, siguiente/anterior, shuffle y
like funcionando igual que con una playlist. `likes -s` arranca mezclado.

## Criterios de aceptación

- [ ] **AC-1** — `likes` reproduce Tus me gusta empezando por el último
      tema likeado, y sigue en el orden de la app oficial.
- [ ] **AC-2** — Con más de 50 me gusta se reproducen todos (hasta
      `LIKES_MAX`); con más que el tope, se avisa cuántos no entran.
- [ ] **AC-3** — `likes -s` arranca con un tema al azar y el resto
      mezclado; `s`, siguiente, anterior y encolar (`queue`) funcionan
      como con una playlist.
- [ ] **AC-4** — Mientras carga se ve `Cargando tus me gusta…` con el
      avance, y lo que sonaba antes no se corta hasta que arranca la lista.
- [ ] **AC-5** — Sin me gusta, avisa `Tus me gusta no tiene temas para
      reproducir.` y no
      corta lo que sonaba. Los temas locales se omiten y se cuentan.
- [ ] **AC-6** — Errores de red, 401/403 o 429 a mitad de la carga se
      muestran con `status_error`, no cortan lo que sonaba y no dejan una
      lista a medias sonando.
- [ ] **AC-7** — Con el token que ya tiene los scopes del spec 011, `likes`
      no pide login de nuevo.
- [ ] **AC-8** — `help` y Tab (completar) incluyen `likes`; `likes` con un
      argumento que no sea `-s` avisa el uso.

## Riesgos / casos de falla

- **Cuota de la Web API:** una biblioteca grande hace muchos pedidos
  seguidos. Tope duro `LIKES_MAX` en config; un 429 corta la carga con el
  error de siempre en vez de reintentar en loop.
- **Carga interrumpida:** si falla una página, se descarta todo lo cargado
  (no suena una lista incompleta sin avisar) y lo anterior sigue sonando.
- Solo lectura: no modifica la biblioteca.

## Plan técnico

- Archivos que toca: `src/config.rs`, `src/spotify/web.rs`,
  `src/app/backend.rs`, `src/app/shell.rs`, `src/app/engine.rs`,
  `src/desktop/app.rs` (el aviso de tarea en curso pasa a `String`),
  `README.md`, `docs/arquitectura.md`, `docs/decisiones.md`, `CHANGELOG.md`,
  `Cargo.toml`/`Cargo.lock`.
- Funciones/estructuras nuevas:
  - `config`: `LIKES_PAGE` (50) y `LIKES_MAX` (2000), con chequeo al
    compilar como `MY_PLAYLISTS_*`.
  - `web::WebClient::liked_tracks(max, progress) -> LikedTracks { tracks:
    Vec<SpotifyUri>, local: usize, total: usize }`: pagina
    `GET /me/tracks` hasta que no haya `next` o se llegue a `max`;
    `progress` recibe cuántos lleva, para el mensaje de carga. Un error en
    cualquier página devuelve `Err` (nada parcial).
  - El avance llega al motor por un `tokio::sync::watch` y se muestra en
    `Prompt::Busy`, que pasa de `&'static str` a `String`.
  - `Backend::liked_tracks` (y en el `FakeBackend`).
  - `shell::ShellCommand::Likes { shuffle }`; `likes` / `likes -s`.
    `HELP` y `NAMES` al día.
  - `engine`: `prepare` recibe un origen `Source::Uri(SpotifyUri)` |
    `Source::Liked`; con `Liked` pide `liked_tracks` en vez de
    `backend.resolve` y arma el mismo `Resolved` (`skipped` = locales +
    los que quedan afuera del tope), así el resto del camino (`Prepared`,
    cola, shuffle) no cambia. Sin temas → `NothingToPlay`.
- Tests necesarios:
  - `web`: parseo de una página de `/me/tracks` (con `next`, `null`,
    `is_local`), corte por tope.
  - `shell`: `likes`, `likes -s`, argumento inválido.
  - `engine` (fake): reproduce en orden (AC-1), varias páginas y tope con
    aviso (AC-2), `-s` mezcla (AC-3), vacío no corta lo que suena (AC-5),
    error a mitad de carga no deja lista parcial (AC-6).
  - AC-4 (visual), AC-7 y AC-1 contra la app oficial: prueba manual con
    la cuenta.
- Contratos a crear/actualizar: doc-comments de `liked_tracks` en
  `WebClient` y `Backend`, `Source` y `prepare` en `Engine`;
  `docs/arquitectura.md` (endpoint nuevo de `web`). `docs/decisiones.md`:
  Tus me gusta vía Web API + carga completa antes de sonar.
- Versión que publica: `0.5.0` (spec nuevo)

## Tareas

- [x] T1 — `config`: `LIKES_PAGE`, `LIKES_MAX`.
- [x] T2 — `web`: `liked_tracks` + tests de parseo.
- [x] T3 — `backend`: `liked_tracks` en `Backend`, `SpotifyBackend` y el
      fake.
- [x] T4 — `shell`: `likes [-s]`, `HELP`, `NAMES` + tests.
- [x] T5 — `engine`: `Source`, `prepare` con `Liked`, mensaje de carga +
      tests.
- [x] T6 — Docs, contratos, changelog 0.5.0, versión; fmt/clippy/test.
- [ ] T7 — Prueba manual con la cuenta (AC-1, AC-4, AC-7).

## Definition of Done

- [ ] Todos los AC tildados, o el estado es `Reabierto (parcial)` con el
      motivo explícito
- [ ] Tests corren y pasan
- [ ] Contratos de funciones públicas y `docs/arquitectura.md` actualizados
      si el spec cambió una firma, comportamiento o el mapa de módulos
- [ ] Decisiones de diseño relevantes documentadas en `docs/decisiones.md`
- [ ] Changelog actualizado: sección `## [0.5.0] - fecha`
- [ ] Versión subida en `Cargo.toml` y `Cargo.lock`; el Release lo publica
      CI al mergear
- [ ] Sin constantes/umbrales hardcodeados fuera de `src/config.rs`
- [ ] Sin secretos ni credenciales en el diff
- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test` pasan

## Notas de verificación

- Tests automáticos (motor con backend falso, parseo de `web` y `shell`):
  orden y siguiente (AC-1), locales y tope con aviso (AC-2), `-s` (AC-3),
  avance en el aviso de tarea y cancelar (AC-4), vacío y error de red sin
  cortar lo que suena (AC-5, AC-6), `likes` en `help`/Tab y argumentos
  inválidos (AC-8).
- Pendiente: prueba manual con la cuenta (T7: AC-1 contra la app oficial,
  AC-4 visual, AC-7 sin login nuevo).
