# 003 - Shuffle, siguiente/anterior y cola de reproducción

## Estado
Reabierto (parcial)

AC-12 (medición de consumo de 30 min) no se verificó: se decidió no
hacer esa prueba en este spec. Ver "Notas de verificación".

## Contexto
Hoy `play` de un álbum o playlist reproduce los temas en orden, de principio
a fin, y lo único que se puede hacer mientras suena es pausar o salir. Faltan
los controles básicos de cualquier reproductor: mezclar (shuffle), saltar al
siguiente tema o volver al anterior. También se pidió **investigar** si se
pueden encolar temas.

**Investigación: encolar temas.**
- La Web API tiene `POST /me/player/queue`, pero solo agrega a la cola de un
  dispositivo Spotify Connect que Spotify controla. Nuestro reproductor es
  `librespot` embebido **sin** Spotify Connect (spec 001): no aparece como
  dispositivo y Spotify no conoce su cola. Ese endpoint no sirve acá (y
  además gastaría cuota de la Web API).
- La cola ya es nuestra: `ui::playback::Queue` decide qué tema sigue y se lo
  pide al reproductor. Encolar es agregar temas a esa estructura, sin API
  nueva. Lo único que se necesita de Spotify es la búsqueda de spec 002
  para elegir qué encolar.
- Lo que cuesta de verdad es la interfaz: hay que escribir texto (la
  búsqueda) y elegir un resultado **mientras la música sigue sonando**, sin
  cortar los eventos del reproductor. Es factible con el mismo loop
  `select!` de `playback`: la búsqueda corre como un future más del loop.
- Lo que **no** es posible sin Spotify Connect: ver o modificar esta cola
  desde el celular u otra app de Spotify. Queda fuera de alcance.
- Conclusión: es factible y se incluye en este spec (ver supuestos).

## Preguntas / Supuestos

- **¿Encolar entra en este spec o solo se investiga?** → Asumido: entra
  (motivo: la investigación dio factible y chico en API; el costo está en la
  UI, que se diseña igual para siguiente/anterior). Si preferís dejarlo para
  otro spec, se sacan AC-7 a AC-10 y las tareas T5-T6.
- **¿Cómo se activa el shuffle?** → Asumido: tecla `s` durante la
  reproducción, que prende y apaga. Además `play --shuffle <…>` (o `-s`)
  arranca mezclado, empezando por un tema al azar (como "Reproducción
  aleatoria" de la app). Motivo: con la tecla sola, el primer tema siempre
  es el primero de la lista.
- **¿Qué hace el shuffle con el tema que suena?** → Asumido, igual que
  Spotify: el tema actual sigue sonando; se mezclan los que faltan. Al
  apagarlo, se sigue en el orden original a partir del tema actual.
- **¿Shuffle solo para playlists?** → Asumido: para cualquier lista
  (playlist o álbum); con un solo tema no hace nada y lo avisa.
- **¿Teclas?** → Asumido: `n` o → = siguiente; `p` o ← = anterior; `s` =
  shuffle; `a` = agregar a la cola; espacio y `q` como hoy.
- **¿"Anterior" vuelve al tema anterior o reinicia el actual?** → Asumido,
  igual que Spotify: si el tema actual lleva más de 3 s sonando, vuelve al
  principio; si no, pasa al anterior. En el primer tema, siempre vuelve al
  principio. El umbral va en `config.rs`.
- **¿"Anterior" en qué orden?** → Asumido: el tema que sonó antes (historial
  de lo escuchado), no el anterior de la lista original. Con shuffle
  prendido es lo único que tiene sentido, y sin shuffle da lo mismo.
- **¿"Siguiente" en el último tema?** → Asumido: termina, como cuando el
  último tema termina solo (sin repetir; repetir es otro spec).
- **¿Cómo se encola?** → Asumido: `a` abre una línea de búsqueda dentro de
  la pantalla de reproducción; se escribe el nombre del tema, Enter busca y
  muestra los 5 resultados como en spec 002; con 1-5 o Enter se agrega a la
  cola y Esc cancela. La música no se corta en ningún momento.
- **¿Dónde entra lo encolado?** → Asumido, igual que Spotify: después del
  tema actual y antes del resto de la lista, en el orden en que se
  encoló. El shuffle no lo mezcla.
- **¿Solo temas o también playlists/álbumes en la cola?** → Asumido: solo
  temas (motivo: es el uso común, y encolar 200 temas de una playlist
  desordena todo). `a` busca temas.
- **¿Se puede encolar con `play` de un solo tema?** → Asumido: sí. Si hay
  temas encolados, al terminar el tema no se sale: siguen los encolados.
- **¿Se ve la cola?** → Asumido: la línea de estado muestra el shuffle
  (prendido/apagado) y cuántos temas hay encolados; al encolar se imprime
  "➕ En cola: nombre — artistas". Una vista completa de la cola queda para
  la TUI.
- **¿Persiste la cola al salir?** → No. Vive lo que dura `play`.

## Qué debe pasar (no cómo)
1. Durante la reproducción de una lista, `s` mezcla los temas que faltan
   (el actual sigue) y vuelve a apretarse para volver al orden original
   desde el tema actual. `play --shuffle <…>` arranca mezclado desde un tema
   al azar.
2. `n` / → pasa al siguiente tema (encolado primero, si hay); `p` / ←
   vuelve al principio del tema o al anterior, según los segundos que lleve.
3. `a` permite buscar un tema por nombre sin cortar la música, elegirlo de
   los 5 mejores y agregarlo a la cola; suena después del actual.
4. La línea de estado muestra si el shuffle está prendido y cuántos temas
   hay en cola.

## Criterios de aceptación

- [x] **AC-1** — Con una playlist o álbum sonando, `s` prende el shuffle:
      el tema actual sigue sin cortarse y los siguientes salen en orden
      aleatorio, sin repetir ni saltear ninguno de la lista.
- [x] **AC-2** — `s` otra vez apaga el shuffle: después del tema actual
      sigue el que le correspondía en el orden original.
- [x] **AC-3** — `play --shuffle <playlist|álbum>` (y `-s`) arranca por un
      tema al azar y sigue mezclado; con un tema solo, o `s` con un tema
      solo, avisa que no hay nada que mezclar y sigue sonando.
- [x] **AC-4** — `n` / → pasa al siguiente tema sin silencio notable si
      estaba precargado; en el último tema, termina como si hubiera
      terminado solo.
- [x] **AC-5** — `p` / ← con más de `config::PREVIOUS_RESTART_THRESHOLD`
      de tema reinicia el actual; con menos, vuelve al tema que sonó antes
      (también con shuffle); en el primer tema, reinicia.
- [x] **AC-6** — Apretar `n` / `p` varias veces seguido rápido no saltea
      de más ni deja la cola en un estado inconsistente (los eventos viejos
      del reproductor se ignoran, como en spec 001).
- [x] **AC-7** — `a` abre una línea de búsqueda; mientras se escribe, la
      música sigue y los eventos del reproductor se atienden (el paso al
      tema siguiente no se demora). Backspace borra; Esc cancela.
- [x] **AC-8** — Enter en la búsqueda muestra hasta 5 temas; elegir uno lo
      encola y se imprime "➕ En cola: …"; suena inmediatamente después del
      tema actual, antes del resto de la lista, y varios encolados suenan en
      el orden en que se encolaron. El shuffle no los mezcla.
- [x] **AC-9** — Con `play` de un solo tema y temas encolados, al terminar
      el tema siguen los encolados en vez de salir.
- [x] **AC-10** — Una búsqueda que falla (sin red, sin resultados, 429)
      muestra el error en una línea y vuelve a la reproducción sin cortar
      la música ni salir.
- [x] **AC-11** — La línea de estado muestra shuffle prendido/apagado y la
      cantidad de temas en cola, y la ayuda de teclas incluye `n`, `p`, `s`
      y `a` (también en `help`).
- [ ] **AC-12** — Consumo en reproducción continua (30 min con algunos
      `n`/`p`/`s`) dentro de lo de spec 001 (RAM ≤ 60 MB, CPU < 2 %), sin
      cortes.

## Riesgos / casos de falla

- **Cortes de audio:** la búsqueda de `a` no puede bloquear el loop de
  eventos (se precarga el siguiente tema por eventos): corre como future
  dentro del `select!`, con el timeout de la Web API (`HTTP_TIMEOUT`).
- **Cuota de la Web API:** una búsqueda = un pedido y solo al apretar
  Enter (nada de buscar mientras se escribe).
- **Saltos rápidos:** cada `n`/`p` pide un tema nuevo; la cola solo acepta
  eventos del último `play_request_id` (mecanismo de spec 001).
- **Temas no disponibles:** siguen la regla de spec 001 (se saltean con
  aviso; `MAX_CONSECUTIVE_UNAVAILABLE` seguidos cortan).

## Plan técnico
*(aprobado antes de implementar; los desvíos quedan en "Notas de verificación")*

- **Archivos que toca:**
  - `Cargo.toml` — `rand` 0.9 como dependencia directa: ya la compila
    `librespot-playback` (misma versión), no suma código. Se registra en
    `docs/decisiones.md`.
  - `src/config.rs` — `PREVIOUS_RESTART_THRESHOLD: Duration = 3 s`.
  - `src/ui/cli.rs` — `Command::Play { target, shuffle }` y
    `Command::Search { kind, query, shuffle }`; `--shuffle`/`-s` después de
    `play`. `USAGE` con las teclas nuevas.
  - `src/spotify/player.rs` — `Player::restart()` (seek a 0).
  - `src/ui/playback.rs` — `Queue` pasa a tener:
    - `tracks` (lista original) + `order: Vec<usize>` (orden de
      reproducción; con shuffle, los índices que faltan mezclados);
    - `up_next: VecDeque<SpotifyUri>` (lo encolado, sale primero);
    - `history: Vec<SpotifyUri>` (lo que sonó, para "anterior");
    - `toggle_shuffle(&mut impl Rng)`, `next()`, `previous(elapsed)`,
      `enqueue(uri)`, y el cálculo de qué precargar mirando `up_next`
      primero. Sigue sin I/O, para testearlo.
  - Tiempo sonando del tema actual: se toma de `position_ms` de
    `Playing`/`Paused`/`Seeked` + `Instant` desde el último `Playing`.
  - Modo de entrada del loop: `Normal | Typing(String) | Choosing(Vec<Hit>)`
    y un `Option<Pin<Box<dyn Future>>>` con la búsqueda en curso como rama
    más del `select!`. `play_queue` recibe `&WebClient` para buscar.
  - `src/ui/select.rs` — se extraen el formato de la lista y el mapeo de
    teclas para reutilizarlos dentro de `playback` (sin su propio loop).
  - `src/main.rs` — pasa `shuffle` y `&web` a `play`.
  - `README.md`, `CHANGELOG.md`, `docs/arquitectura.md` (la cola, el
    historial y la búsqueda dentro de `playback`), `docs/decisiones.md`.
- **Tests:**
  - Unitarios de `Queue` con un RNG de semilla fija: shuffle es una
    permutación de lo que falta y deja el actual; apagarlo retoma el orden
    original; `next` saca de `up_next` primero; `previous` según umbral e
    historial; `next` en el último → fin; eventos viejos ignorados tras
    varios `next` seguidos; encolados en orden y no mezclados; un tema solo
    con encolados no termina.
  - Parseo de `--shuffle`/`-s`; mapeo de teclas nuevas (incluidas flechas).
  - Manuales: AC-1 a AC-12 con playlists/álbumes reales; AC-12 con
    `scripts/medir-consumo.ps1 -Minutos 30`.
- **Contratos y arquitectura:** doc-comments de `Queue` (invariantes:
  `order` es una permutación de los índices de `tracks`; `up_next` nunca se
  mezcla), `play_queue` (nuevas teclas, búsqueda sin bloquear),
  `Player::restart`, `cli::Command`; `docs/arquitectura.md` actualizado.

## Tareas

- [x] T1 — `rand` + `PREVIOUS_RESTART_THRESHOLD` + `--shuffle` en `cli`
      con tests (AC-3 parseo)
- [x] T2 — `Queue` nueva (order, historial, `next`/`previous`, shuffle) con
      tests (AC-1, AC-2, AC-5, AC-6)
- [x] T3 — Teclas `n`/`p`/`s` + `Player::restart` + línea de estado en
      `playback` (AC-1 a AC-6, AC-11)
- [x] T4 — `up_next` + `enqueue` en `Queue` con tests (AC-8, AC-9)
- [x] T5 — Modo búsqueda dentro de `playback` (`a`, escribir, elegir) sin
      bloquear el loop (AC-7, AC-8, AC-10)
- [ ] T6 — Prueba manual completa + medición de 30 min (AC-12)
- [x] T7 — README, changelog, `docs/arquitectura.md`, `docs/decisiones.md`

## Definition of Done

- [x] Todos los AC tildados, o el estado es `Reabierto (parcial)` con el
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

- **Implementación (2026-09-27), T1-T5 y T7.** 68 tests; `fmt --check` y
  `clippy --all-targets -D warnings` OK. `help` muestra las teclas nuevas.
- **Desvíos del plan (menores):**
  - El historial es una línea de tiempo (`timeline` + `cursor`): "anterior"
    mueve el cursor y "siguiente" retoma hacia adelante los temas a los que
    se volvió, antes del resto de la lista. Lo encolado igual va primero:
    se inserta justo después del tema actual (test
    `encolar_despues_de_volver_suena_despues_del_actual`).
  - Si cambia lo que sigue después del aviso de precarga (shuffle o
    encolar), se precarga de nuevo lo que corresponde (`preload_due` +
    `Queue::repreload`), para no perder el paso sin silencio.
  - En la búsqueda de `a`, `q` y espacio se escriben (no salen ni pausan);
    Esc cancela y Ctrl+C sale en cualquier modo.
  - `-s`/`--shuffle` se acepta en cualquier lugar después de `play`.
  - `Token::for_tests()` (solo en tests) para armar un `WebClient` en los
    tests de teclado de `playback`.
  - La línea de estado es corta (`▶ Sonando  shuffle: no  cola: 0
    [espacio n p s a q]`) para no pasar de 80 columnas; el detalle de
    teclas está en `help` y el README.
- **Cobertura con tests unitarios** (la verificación final de cada AC es
  manual, con audio):
  - AC-1/AC-2: `shuffle_mezcla_lo_que_falta_sin_repetir_ni_saltear`,
    `apagar_shuffle_retoma_el_orden_original`.
  - AC-3: `arrancar_mezclado_usa_toda_la_lista`, `un_solo_tema_no_se_mezcla`,
    `shuffle_en_cualquier_lugar` (parseo).
  - AC-4/AC-5/AC-6: `siguiente_y_en_el_ultimo_termina`,
    `anterior_segun_el_tiempo_sonando`,
    `anterior_con_shuffle_vuelve_a_lo_que_sono`,
    `siguiente_varias_veces_rapido_ignora_eventos_viejos`, `reloj_del_tema`.
  - AC-7/AC-8/AC-10: `escribir_la_busqueda_para_encolar`,
    `enter_busca_y_esc_cancela_la_busqueda`,
    `elegir_un_resultado_lo_encola`,
    `lo_encolado_suena_despues_del_actual_en_orden`,
    `el_shuffle_no_mezcla_lo_encolado`.
  - AC-9: `tema_unico_con_encolados_no_termina`.
- **Cierre (2026-09-27):** AC-1 a AC-11 los diste por probados con audio
  real al pedir el cierre (sin detalle por AC); la cobertura con tests
  unitarios está arriba.
- **AC-12 sin verificar, a propósito:** decidiste no hacer la medición de
  30 min con `scripts/medir-consumo.ps1`. Por eso el estado es
  `Reabierto (parcial)` y T6 queda sin tildar. Lo que se sabe: la cola
  agrega solo unos vectores de URIs y un `Instant`, sin hilos ni polling
  nuevos (la búsqueda de `a` es un pedido por Enter), así que no se espera
  un cambio respecto de spec 001 (~19 MB, 0,07 % CPU), pero no está medido.
  Si se quiere cerrar, alcanza con correr la medición y tildar AC-12.
