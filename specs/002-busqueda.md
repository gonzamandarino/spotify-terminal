# 002 - Buscar y reproducir por nombre (temas y playlists)

## Estado
En verificación

## Contexto
Hoy `play` solo acepta URI, link de `open.spotify.com` o ID. Para escuchar
algo hay que ir a la app oficial o al navegador a copiar el link, que es
justo lo que el proyecto quiere evitar. Este spec agrega búsqueda por nombre:
`play <texto>` busca temas y `play list <texto>` busca playlists, muestra
las 5 mejores coincidencias y reproduce la que se elija.

Es la función objetivo "Elegir y reproducir una canción específica
(búsqueda / selección)" de `CLAUDE.md`, más la búsqueda de playlists.

**Factibilidad:** el endpoint `GET /v1/search` sigue disponible para apps en
modo desarrollo. Desde feb/mar 2026 su `limit` máximo es 10 y el default 5
([changelog feb 2026](https://developer.spotify.com/documentation/web-api/references/changes/february-2026)),
así que pedir 5 resultados entra en el límite. La búsqueda no requiere
scopes extra.

## Preguntas / Supuestos

- **¿Qué pasa después de mostrar el top 5?** → Asumido: se muestra una
  lista numerada del 1 al 5 y se elige con la tecla del número; Enter elige
  el 1 (la mejor coincidencia); `q`/Esc cancela sin reproducir (motivo: el
  pedido dice "busque el top 5", lo que implica elegir; Enter = 1 deja el
  caso más común a una tecla).
- **¿Cómo se distingue una búsqueda de un URI/link/ID?** → Asumido: si el
  texto después de `play` es un solo argumento que ya se reconoce como URI,
  link o ID (comportamiento de spec 001), se reproduce directo, sin buscar.
  Cualquier otra cosa se busca como tema (motivo: no rompe lo que ya anda).
- **¿Sintaxis de playlists?** → `play list <texto>` (pedido de vos). También
  se acepta `play playlist <texto>` (asumido: es lo que uno escribe sin
  pensar, y no cuesta nada).
- **Tema cuyo nombre empieza con "list"** (ej. "List of Demands") →
  Asumido: `play list of demands` busca playlists. Para buscar ese tema se
  usa comillas: `play "list of demands"` (un solo argumento que no es la
  palabra `list` sola). Caso raro; no justifica otra sintaxis.
- **¿Varias palabras sin comillas?** → Sí: `play never gonna give you up`
  une los argumentos con espacios (hoy da "demasiados argumentos").
- **¿Qué se reproduce al elegir un tema?** → Asumido: solo ese tema, igual
  que `play <URI de tema>` hoy. Radio / cola automática quedan para otro
  spec.
- **¿Qué muestra cada resultado?** → Asumido: temas → "nombre — artistas
  (álbum, duración)"; playlists → "nombre — dueño (N temas)". Suficiente
  para distinguir versiones/covers sin ensuciar la pantalla.
- **¿Un subcomando `search` aparte, sin reproducir?** → Asumido: no. La
  búsqueda existe para elegir qué reproducir; si hace falta sola, otro spec.
- **¿Resultados no reproducibles en el país de la cuenta?** → Asumido: se
  pide con `market=from_token` para que Spotify filtre lo que no se puede
  escuchar desde la cuenta.
- **¿Playlists editoriales/algorítmicas de Spotify (ej. "Today's Top
  Hits", "Discover Weekly")?** → *Riesgo a validar en T1 (spike):* desde
  nov 2024 las apps nuevas no ven playlists de Spotify por la Web API, así
  que pueden no aparecer en la búsqueda. La reproducción de una playlist va
  por `librespot-metadata` (spec 001), no por la Web API, así que si
  aparece en los resultados debería sonar. Si no aparecen, se documenta como
  limitación en este spec y en el README; no se implementa un rodeo.
- **¿Cantidad de resultados configurable?** → Se fija en 5 como constante en
  `config.rs` (`SEARCH_LIMIT`), con tope 10 por el límite de la API.

## Qué debe pasar (no cómo)
1. `play <texto>` (una o varias palabras) que no sea URI/link/ID busca
   temas y muestra hasta 5 resultados numerados, del de mejor coincidencia
   al de peor, según el orden que devuelve Spotify.
2. `play list <texto>` (o `play playlist <texto>`) hace lo mismo con
   playlists.
3. Con un número del 1 al N (o Enter para el 1) se reproduce el resultado
   elegido, con los mismos controles que hoy (espacio = pausa/reanudar,
   `q` = salir). Una playlist se reproduce entera como en spec 001.
4. `q` o Esc en la lista sale sin reproducir y con código 0.
5. Sin resultados, el cliente lo dice ("no encontré temas/playlists para
   «…»") y sale con código distinto de 0.
6. `play <URI | link | ID>` sigue funcionando exactamente como en spec 001.

## Criterios de aceptación

- [x] **AC-1** — `play <nombre de tema>` (ej. `play never gonna give you up`,
      sin comillas) muestra como máximo 5 temas numerados con nombre,
      artistas, álbum y duración, en el orden de Spotify.
- [x] **AC-2** — `play list <nombre>` y `play playlist <nombre>` muestran
      como máximo 5 playlists numeradas con nombre, dueño y cantidad de
      temas.
- [ ] **AC-3** — Elegir un número válido (o Enter = 1) reproduce ese tema o
      esa playlist entera; pausa, reanudar y `q` funcionan igual que en
      spec 001.
- [ ] **AC-4** — En la lista, `q`/Esc sale con código 0 sin abrir la sesión
      de audio; una tecla fuera de rango se ignora (no elige nada ni sale).
- [x] **AC-5** — Una búsqueda sin resultados informa "no encontré…" con el
      texto buscado y sale con código ≠ 0.
- [x] **AC-6** — `play` con URI, link o ID (tema, álbum o playlist) no hace
      búsqueda y reproduce como en spec 001 (tests de parseo de 001 siguen
      pasando).
- [x] **AC-7** — `play`, `play list` o `play playlist` sin texto →
      error de uso con código ≠ 0.
- [ ] **AC-8** — Errores de red, 401 y 429 durante la búsqueda dan los
      mismos mensajes con instrucciones que el resto de la Web API
      (spec 001, AC-7), sin stack trace.
- [ ] **AC-9** — Consumo: una búsqueda es un solo pedido HTTP (sin polling ni
      "búsqueda mientras escribís"); con la reproducción en marcha, el
      consumo sigue dentro de lo medido en spec 001 (RAM ≤ 60 MB, CPU < 2 %).

## Riesgos / casos de falla

- **Envío de datos a terceros:** el texto buscado se manda a Spotify (es
  el propósito). No se guarda historial de búsquedas localmente ni se loguea.
- **Cuota de la Web API (429):** una búsqueda = un pedido; nada de reintentos
  en loop. Con 429 se informa `Retry-After` y se sale (igual que spec 001).
- **Playlists de Spotify invisibles para apps nuevas:** ver supuestos; se
  valida en T1 y, si aplica, se documenta como limitación.
- **Resultado elegido que después no suena** (restricción de región,
  playlist privada ajena): se usa el manejo de errores de `play` de
  spec 001 (tema no disponible / se saltea en listas).
- **Terminal en modo raw al elegir:** si el proceso se corta durante la
  selección, la terminal se restaura (mismo mecanismo que durante `play`).

## Plan técnico
*(aprobado antes de implementar; los desvíos quedan en "Notas de verificación")*

- **Archivos que toca:**
  - `src/config.rs` — `SEARCH_LIMIT: u8 = 5` (con test que verifica ≤ 10).
  - `src/spotify/web.rs` — `WebClient::search_tracks(&str)` y
    `WebClient::search_playlists(&str)` sobre `GET /search`
    (`type=track|playlist`, `limit=SEARCH_LIMIT`, `market=from_token`),
    reutilizando `status_error`. Structs `TrackHit` y `PlaylistHit` con lo
    necesario para mostrar + `uri: SpotifyUri`. Las playlists pueden venir
    como `null` dentro de `items` (pasa en la API): se descartan.
  - `src/ui/cli.rs` — `Command::Search { kind: SearchKind, query: String }`;
    `parse` une los argumentos de `play`, reconoce `list`/`playlist` como
    primer palabra y cae a búsqueda si `parse_playable` no reconoce el
    texto. `USAGE` actualizado.
  - `src/ui/select.rs` (nuevo) — muestra la lista numerada y lee una tecla
    (`1..N`, Enter, `q`/Esc) con `crossterm`, en modo raw con la misma
    restauración que `playback`. Devuelve `Option<usize>`.
  - `src/main.rs` — `Command::Search`: buscar → sin resultados =
    `AppError::NoResults` → elegir → si se eligió, seguir el mismo camino
    que `Command::Play(uri)` (se extrae a una función para no duplicarlo).
    El chequeo de Premium y la sesión de audio se abren **después** de
    elegir, para que cancelar no cueste nada.
  - `src/error.rs` — variante `NoResults { kind, query }`.
  - `README.md`, `CHANGELOG.md`, `docs/arquitectura.md` (nuevo módulo
    `ui::select`, `web` hace búsqueda).
- **Tests:**
  - Unitarios: parseo (`play a b c` → búsqueda "a b c"; `play list x`,
    `play playlist x`; `play "list of demands"` → tema; URI/link/ID → `Play`;
    `play list` sin texto → error); deserialización de respuestas de
    `/search` de ejemplo (incluida playlist `null`); formato de cada línea
    de resultado (duración `m:ss`); `SEARCH_LIMIT <= 10`.
  - Manuales (evidencia en "Notas de verificación"): AC-1 a AC-5, AC-8
    (sin red), AC-9 con `scripts/medir-consumo.ps1`.
- **Contratos y arquitectura:** doc-comments Pre/Post/No debe en
  `search_tracks`, `search_playlists`, `select::choose`, `Command::Search`;
  tabla de contratos y mapa de módulos en `docs/arquitectura.md`.

## Tareas

- [x] T1 — Spike: `GET /search` a mano con el token Web (temas y
      playlists, incluida una editorial de Spotify); anotar el resultado y
      confirmar el riesgo de playlists editoriales
- [x] T2 — `config::SEARCH_LIMIT` + `web::search_tracks` /
      `search_playlists` con tests de deserialización (AC-8)
- [x] T3 — `cli::parse` con `Command::Search` y tests de parseo (AC-6, AC-7)
- [x] T4 — `ui::select` + integración en `main` (AC-1 a AC-5)
- [ ] T5 — Prueba manual completa + medición de consumo (AC-9)
- [x] T6 — README, changelog, `docs/arquitectura.md`

## Definition of Done

- [ ] Todos los AC tildados, o el estado es `Reabierto (parcial)` con el
      motivo explícito
- [x] Tests corren y pasan
- [x] Contratos de funciones públicas y doc de arquitectura actualizados si
      el spec cambió una firma, comportamiento o el mapa de módulos
- [x] Decisiones de diseño relevantes documentadas
- [x] Changelog actualizado
- [x] Sin constantes/umbrales hardcodeados fuera de su lugar de config
- [x] Sin secretos ni credenciales en el diff
- [x] `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test` pasan

## Notas de verificación
*(al cerrar: qué se probó y resultado — por AC cuando no sea obvio)*

- **T1 — spike (2026-09-26)**, `GET /search` a mano con el token Web:
  - Temas: `never gonna give you up` → 5 resultados, Rick Astley primero;
    traen `is_playable` con `market=from_token`.
  - Playlists: las editoriales/algorítmicas de Spotify llegan como `null`
    dentro de `items` ("today's top hits": 3 de 5 `null`; "this is soda
    stereo": 4 de 5). Riesgo confirmado → limitación documentada en
    `docs/decisiones.md` y el README.
  - La cantidad de temas de una playlist ahora viene en `items.total` (antes
    `tracks.total`, cambio de feb 2026); se aceptan los dos.
  - `limit=11` → 400: el máximo es 10.
  - Búsqueda sin coincidencias → `items: []`, `total: 0`.
- **Desvíos del plan (menores):**
  - Las playlists se piden con `limit=10` y se muestran las 5 primeras que no
    son `null` (ver `docs/decisiones.md`); temas con `limit=5`. Nueva
    constante `SEARCH_API_MAX_LIMIT`.
  - En vez de `search_tracks`/`search_playlists` con structs propios, un solo
    `WebClient::search(SearchKind, &str)` que devuelve `Hit { uri, name,
    detail }`: `ui::select` no necesita distinguir temas de playlists.
    `SearchKind` vive en `spotify::web`.
  - `SEARCH_LIMIT` se valida al compilar (`const` assert: ≥ 1, ≤ 9 por la
    tecla única, ≤ el máximo de la API) en vez de con un test (clippy lo
    pide así).
  - `RawMode` pasó de `ui::playback` a `ui/mod.rs` para compartirlo con
    `ui::select`.
  - `play` con un solo argumento que empieza con `spotify:` o contiene
    `open.spotify.com/` y no es válido sigue dando error de uso (como en
    spec 001) en vez de buscarse como texto.
  - `WebClient::current_user` y `search` comparten `get_json` (mismo manejo
    de 401/429/5xx → AC-8 por construcción).
- **Probado con el binario release (2026-09-26):**
  - **AC-1:** `play never gonna give you up` → 5 temas numerados con
    artistas, álbum y duración, en el orden de Spotify.
  - **AC-2:** `play list rock nacional` → 5 playlists con dueño y cantidad
    de temas. `play playlist …` y `play list …` dan el mismo comando (test
    `busqueda_de_playlists`).
  - **AC-5:** `play zzqxjvkwpqzzqxjv` → "no encontré temas para
    «zzqxjvkwpqzzqxjv»", `exit=1`.
  - **AC-6:** `play spotify:artist:…` sigue dando error de uso (`exit=1`);
    tests de parseo de spec 001 intactos y pasando, más
    `uri_link_o_id_no_busca`. El camino de reproducción es el mismo código
    de spec 001 (extraído a `main::play`).
  - **AC-7:** `play list` → "falta el nombre de la playlist" + uso,
    `exit=1`; `play` y `play "  "` cubiertos por tests.
  - 48 tests; `fmt --check` y `clippy --all-targets -D warnings` OK.
- **Pendiente (lo tenés que probar vos, necesita teclado y audio):** AC-3
  (elegir y que suene; tema y playlist), AC-4 (`q`/Esc en la lista → exit 0;
  tecla fuera de rango se ignora), AC-8 (sin red: mensaje de conexión) y AC-9
  (`scripts/medir-consumo.ps1` con una búsqueda + reproducción).

