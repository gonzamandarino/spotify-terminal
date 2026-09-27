# 010 - Visualización de la canción

## Estado
En implementación

## Contexto
Se pide una forma de **ver** la canción que suena en la app de escritorio,
con varias opciones elegibles como parte de la personalización (spec 009):

1. Onda de sonido.
2. Barras de sonido (espectro).
3. Vinilo girando con la tapa del disco del tema.

La app tiene que seguir siendo liviana (CLAUDE.md: bajo consumo es un
requisito, y la reproducción gana siempre). Antes de escribir este spec
se midió un prototipo descartable (no quedó en el código) con las tres
visualizaciones animadas con datos falsos en un panel de 280 pt, en
release, en una PC de 16 núcleos:

| Visualización | 30 fps | 15 fps | 10 fps | RAM |
|---|---|---|---|---|
| Ninguna (hoy) | 0 % | 0 % | — | 27 MB |
| Onda | 15,6 % | 6,7 % | — | +1,5 MB |
| Barras (32) | 17,7 % | 8,9 % | — | +1,5 MB |
| Vinilo (textura 300 px) | 16,3 % | 6,6 % | 3,2 % | +2 MB |
| Barras / vinilo en 1600×900, 15 fps | — | 13,0 / 8,5 % | — | 43 MB |

(% de un núcleo.) El costo es redibujar la ventana por CPU en cada cuadro
(`egui_software_backend`): crece con los fps y con el tamaño de la
ventana. El análisis del audio (copiar muestras, una FFT de 1024 puntos
15 veces por segundo) es despreciable al lado.

## Preguntas / Supuestos

- **¿Dónde se ve?** → Respondido por vos: en un panel a la derecha de la
  consola. Asumido: ancho fijo en config (280 pt), alto de la consola.
  Si la ventana es tan angosta que la consola quedaría con menos de un
  mínimo de config, el panel se oculta solo y vuelve al agrandarla.
- **¿Cómo se elige?** → Respondido por vos: submenú **Visualización**
  dentro de Personalización (spec 009), con Ninguna / Onda / Barras /
  Vinilo. Asumido: **Ninguna** por defecto (quien no la quiere no paga
  nada). Se guarda en `ajustes.json` como el resto (spec 007).
- **¿A cuántos fps?** → Respondido por vos: 15 fps, en config (no en el
  menú).
- **¿Cuándo se anima?** → Asumido: solo con música sonando y la ventana
  no minimizada. En pausa queda quieto el último cuadro (el vinilo deja
  de girar); sin nada sonando, onda plana, barras en cero y vinilo sin
  tapa, quietos. Quieto = sin redibujos: CPU ~0 % como hoy.
- **¿El vinilo a cuántas rpm?** → Asumido: ~10 rpm (config), no 33⅓.
  Motivo: a 15 fps y 33 rpm salta 13° por cuadro y se ve a los tirones;
  a 10 rpm son 4°.
- **¿Qué imagen y de dónde?** → Asumido: la tapa del álbum (o del
  podcast) que librespot ya trae en el evento de cambio de tema
  (`AudioItem::covers`, URLs de `i.scdn.co`), la de tamaño más cercano a
  300 px. Se descarga una vez por tema, sin la Web API. Si falla, el
  vinilo sale sin tapa y no se avisa en la consola (no es un error que el
  usuario pueda arreglar).
- **¿Se muestra la tapa recortada en círculo?** → Asumido: sí, como
  etiqueta del disco. Las guías de marca de Spotify piden no recortar ni
  alterar las tapas; para un cliente personal se acepta y se anota en
  `docs/decisiones.md`.
- **¿Las barras y la onda dependen del volumen?** → Asumido: no. El
  volumen se aplica antes de la salida de audio; se compensa para que la
  forma sea la misma a 20 % que a 100 % (con 0 % o mute, plana).
- **¿Desfase entre lo que se ve y lo que suena?** → Asumido: las
  muestras se toman al entrar a la salida de audio, antes de su buffer,
  así que el dibujo va adelantado. Se mide en la implementación; si se
  nota (≥ ~100 ms), se atrasa el dibujo con un valor de config.
- **¿La CLI?** → Asumido: no cambia; no copia muestras ni descarga tapas
  (cero costo).
- **¿Versión y Release?** → Spec nuevo → minor: `0.2.0`. Va en la misma
  rama que spec 009 (sin mergear, a pedido tuyo). Asumido: al publicar,
  el changelog junta 0.1.1 y 0.2.0 en una sola sección `[0.2.0]` (0.1.1
  nunca se publicó).

## Qué debe pasar (no cómo)
1. En Personalización → Visualización se elige Ninguna, Onda, Barras o
   Vinilo; se aplica al instante y queda guardado.
2. Con una visualización elegida, a la derecha de la consola se ve la
   canción: la onda o las barras se mueven con lo que suena; el vinilo
   gira con la tapa del disco del tema.
3. Cuando no suena nada (pausa, stop, ventana minimizada) no se anima y
   la app vuelve a su consumo de reposo.
4. La música nunca se corta por la visualización.

## Criterios de aceptación

- [x] **AC-1** — Personalización tiene el submenú **Visualización** con
      Ninguna / Onda / Barras / Vinilo; la elegida se marca, se aplica en
      el mismo frame, persiste al reabrir y "Restaurar todo" vuelve a
      Ninguna. Con Ninguna no hay panel.
- [x] **AC-2** — Con otra opción, hay un panel de ancho de config a la
      derecha de la consola, del alto de la consola. Si la ventana es tan
      angosta que la consola quedaría bajo el mínimo de config, el panel
      no se muestra; al agrandarla vuelve.
- [x] **AC-3** — **Onda:** dibuja las últimas muestras que salieron al
      audio. Con silencio es una línea plana; con un tono puro es una
      senoidal (test con muestras sintéticas).
- [x] **AC-4** — **Barras:** espectro en N bandas (config) de graves a
      agudos, con caída suave. Un tono de 1 kHz levanta la banda que
      contiene 1 kHz más que las demás; silencio → todas en cero (test).
- [x] **AC-5** — **Vinilo:** gira a las rpm de config mientras suena y se
      detiene en pausa; lleva la tapa del disco del tema que suena y la
      cambia con el tema. Sin tapa (falla la descarga, tema sin tapa)
      gira un disco sin imagen, sin avisos en la consola.
- [x] **AC-6** — Forma independiente del volumen: la misma música a 20 %
      y a 100 % se ve igual (test); con 0 % o mute, plana / en cero.
- [x] **AC-7** — Se anima a ≤ 15 fps (config) solo mientras suena y la
      ventana no está minimizada. En pausa, stop, sin música o con
      Ninguna: CPU ~0 % en reposo, como spec 007 AC-14.
- [ ] **AC-8** — Consumo reproduciendo en 920×580, con cada
      visualización: CPU ≤ 10 % de un núcleo y RAM ≤ 100 MB (tope de spec
      004), medido por PID. 20 cambios de tema con Vinilo no acumulan
      memoria (se suelta la tapa anterior).
- [ ] **AC-9** — La reproducción no espera nunca a la visualización: si
      la ventana está ocupada, se pierden muestras para el dibujo, no
      audio. 10 minutos de música con cada visualización sin cortes
      (verificación tuya con audio real).
- [ ] **AC-10** — El dibujo no va visiblemente adelantado ni atrasado
      respecto de lo que suena (verificación tuya; el desfase medido y la
      compensación quedan en "Notas de verificación").
- [x] **AC-11** — `ajustes.json` con un modo de visualización inválido
      abre con Ninguna y avisa la clave (como spec 007 AC-9).
- [x] **AC-12** — La CLI (`spotify-terminal.exe`) no cambia.

## Riesgos / casos de falla
- **Cortes de audio:** la copia de muestras corre en el hilo de audio de
  librespot. Nunca bloquea (si el buffer está tomado, se saltea ese
  paquete) y no reserva memoria por paquete.
- **Descarga de tapas colgada o lenta:** corre fuera del hilo de la
  ventana y del de audio, con el timeout de red de config; una tapa que
  llega tarde (ya cambió el tema) se descarta.
- **Tapas grandes:** se pide la de ~300 px; si Spotify devuelve una más
  grande se achica antes de subirla como textura. Tope de bytes de
  descarga en config.
- **Consumo con ventana grande:** el costo crece con el tamaño (13 % de
  un núcleo en 1600×900 con barras). El panel tiene ancho fijo; el alto
  sigue a la ventana.

## Plan técnico

Sin dependencias nuevas salvo habilitar el formato JPEG de `image` (ya
está en el árbol, lo trae el portapapeles de `egui-winit` con BMP/PNG):
`image = { version = "0.25", default-features = false, features =
["jpeg"] }`, que suma `zune-jpeg` (Rust puro, chico). La FFT se escribe a
mano (radix-2, 1024 puntos, ~60 líneas) en vez de sumar `rustfft`.

### Archivos que toca

| Archivo | Cambio |
|---|---|
| `Cargo.toml`, `Cargo.lock` | `image` con `jpeg`. Versión `0.2.0`. |
| `src/config.rs` | `viz::FPS = 15`, `PANEL_WIDTH = 280`, `MIN_CONSOLE_WIDTH`, `BARS = 32`, `FFT_SIZE = 1024`, rango de frecuencias de las barras, caída de las barras, `VINYL_RPM = 10`, `COVER_SIZE = 300`, `COVER_MAX_BYTES`, `LATENCY` (desfase a compensar, se fija al medir). |
| `src/spotify/tap.rs` *(nuevo)* | `AudioTap`: buffer circular compartido con las últimas `FFT_SIZE` muestras (mono, f32) y el volumen con que se tomaron. `push` nunca bloquea (`try_lock`); `snapshot` copia a un buffer del lector. |
| `src/spotify/player.rs` | `Player::connect(volume, bitrate, tap: Option<AudioTap>)`: con tap, la salida de audio se envuelve en `TapSink`, que reenvía cada paquete tal cual y copia sus muestras al tap. Sin tap, igual que hoy. |
| `src/app/queue.rs` | `TrackInfo` suma `cover: Option<String>` (la URL de ~300 px de `AudioItem::covers`). |
| `src/app/backend.rs`, `src/app/engine.rs` | `Backend::connect` pasa el tap. `NowPlaying` suma `cover`. Nuevo `Input::Covers(bool)` (la ventana pide tapas solo con Vinilo) y `Output::Cover { url, image }`: el motor descarga y decodifica la tapa en su hilo (reqwest ya está) y la manda como RGBA de ~300 px. |
| `src/desktop/viz/mod.rs` *(nuevo)* | `Visualizer`: modo, estado de las barras (caída), textura de la tapa; `show(ui, rect, now, tap)` dibuja el modo elegido y dice si hace falta el próximo cuadro. |
| `src/desktop/viz/spectrum.rs` *(nuevo)* | FFT radix-2 + ventana de Hann + agrupado en bandas logarítmicas + normalización por volumen. Puro, testeable. |
| `src/desktop/settings.rs` | `visualization: VizMode` (`"visualizacion": "ninguna" \| "onda" \| "barras" \| "vinilo"`), con validación y restaurar. |
| `src/desktop/menu.rs` | Submenú Visualización en Personalización. |
| `src/desktop/app.rs`, `src/desktop/mod.rs` | Crea el tap y lo pasa al motor; panel derecho (antes del `CentralPanel`); `request_repaint_after(1/FPS)` solo si suena y hay modo; `Input::Covers` al cambiar de modo; recibe `Output::Cover` y la sube como textura (suelta la anterior). |
| `src/ui/playback.rs`, CLI | Solo pasa `None` como tap (AC-12). |
| `docs/decisiones.md`, `docs/arquitectura.md`, `docs/glosario.md`, `CHANGELOG.md`, `README.md` | Ver "Contratos y docs". |

### Tests

- `spectrum`: silencio → ceros; tono de 1 kHz → pico en su banda; misma
  señal escalada por el volumen → mismas bandas (AC-4, AC-6); la FFT
  contra una DFT directa en 64 puntos.
- `tap`: `push` con el buffer tomado no bloquea y no pierde audio (el
  paquete sigue al sink); el buffer guarda las últimas N muestras;
  estéreo → mono.
- `TapSink`: reenvía el paquete idéntico al sink interno (AC-9).
- `queue`: `TrackInfo` elige la tapa más cercana a 300 px.
- `engine`: `Output::Cover` solo con `Input::Covers(true)`; una tapa de
  un tema que ya no suena se descarta (backend falso).
- `settings`: modo inválido → Ninguna + aviso (AC-11); restaurar.
- `viz`: sin música / en pausa no pide cuadro siguiente (AC-7); panel
  oculto bajo el ancho mínimo (AC-2).
- Manual con audio real: AC-1, AC-3–AC-5 a la vista, AC-8 medido por PID
  con cada modo, AC-9 y AC-10 (tuyos), AC-12.

### Contratos y docs
- Contratos nuevos: `AudioTap` (`push`, `snapshot`), `TapSink`,
  `spectrum::bands`, `Visualizer::show`, `Output::Cover`,
  `Input::Covers`. Actualizar: `Player::connect`, `Backend::connect`,
  `NowPlaying`, `TrackInfo`.
- `docs/arquitectura.md`: flujo muestras (hilo de audio → `AudioTap` →
  ventana) y tapas (motor → ventana); módulo `desktop::viz`.
- `docs/decisiones.md`: JPEG en `image` (y por qué no `rustfft`), 15 fps
  y solo animar al sonar, tapa recortada en círculo, compensación de
  desfase.
- `docs/glosario.md`: "visualización".

### Versión
`0.2.0` (minor: spec nuevo). Sin Release hasta que decidas mergear la
rama (junto con spec 009).

### Cambios respecto del plan (al implementar)

- `Backend::connect` no cambió de firma: el tap lo guarda
  `SpotifyBackend { tap }` (lo crea `engine::spawn(wake, tap)`), así el
  motor y sus tests no se enteran. `Backend` suma `cover(url)` (con falso
  en los tests).
- En `ajustes.json` es `"visualizacion": {"modo": "onda"}` (sección con
  clave, como el resto del archivo), no un texto suelto.
- `AudioTap` se puede apagar (`set_enabled`): solo copia con Onda o
  Barras. Con Vinilo o Ninguna el hilo de audio no hace nada extra.
- La descarga de tapas corre en un futuro propio del motor, aparte de su
  tarea de fondo: no muestra "ocupado" ni frena comandos. Una tapa que
  llega cuando ya cambió el tema se descarta.
- La tapa se decodifica en un hilo de bloqueo (`spawn_blocking`), no en
  el del motor.
- Se corrigió de paso un doc-comment de spec 009 que había quedado
  pegado a la función equivocada en `desktop::app` (`text_size` /
  `strong`).

## Tareas

- [x] **T1** — `AudioTap` + `TapSink` + `connect` con tap opcional; CLI
      con `None`. Tests (AC-9, AC-12).
- [x] **T2** — `spectrum`: FFT, bandas, normalización por volumen. Tests
      (AC-3, AC-4, AC-6).
- [x] **T3** — Ajuste `visualizacion` y submenú Visualización (AC-1,
      AC-11).
- [x] **T4** — Panel derecho con Onda y Barras, repintado a 15 fps solo
      sonando (AC-2, AC-3, AC-4, AC-7).
- [x] **T5** — Tapas: URL en `TrackInfo`, descarga y decodificación en el
      motor, `Input::Covers` / `Output::Cover`, textura en la ventana.
      Vinilo (AC-5).
- [x] **T6** — Medir el desfase con audio real y compensarlo si hace
      falta (AC-10).
- [ ] **T7** — Verificación con audio real y consumo por PID con cada
      modo (AC-8, AC-9).
- [x] **T8** — Contratos, decisiones, arquitectura, glosario, README,
      changelog y versión `0.2.0`.

## Definition of Done

- [ ] Todos los AC tildados, o el estado es `Reabierto (parcial)` con el
      motivo explícito
- [x] Tests corren y pasan
- [x] Contratos de funciones públicas y doc de arquitectura actualizados si
      el spec cambió una firma, comportamiento o el mapa de módulos
- [x] Decisiones de diseño relevantes documentadas
- [x] Changelog actualizado: sección `## [0.2.0] - fecha` de la versión del
      spec
- [x] Versión subida en `Cargo.toml` y `Cargo.lock`; el Release lo publica
      CI al mergear
- [x] Sin constantes/umbrales hardcodeados fuera de su lugar de config
- [x] Sin secretos ni credenciales en el diff
- [x] `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test` pasan

## Notas de verificación

Parcial (2026-09-27), antes de probar con audio real. 189 tests
(`cargo test`), `cargo fmt --check` y `cargo clippy --all-targets -- -D
warnings` limpios. Capturas con `spotify-desktop.exe` (debug) sin
reproducir, y consumo en release por PID:

- **AC-1:** Personalización → Visualización con Ninguna / Onda / Barras /
  Vinilo (captura, navegando con teclado); elegir Vinilo mostró el panel
  enseguida. Persistencia y restaurar:
  `settings::tests::visualizacion_valida_o_de_fabrica`; "Restaurar todo"
  vuelve a `Settings::default()` (Ninguna).
- **AC-2:** panel de 280 pt a la derecha en 920×580; en 620×400 no se
  muestra. `viz::tests::el_panel_solo_con_modo_y_lugar`.
- **AC-3:** `viz::tests::onda_plana_con_silencio_y_senoidal_con_un_tono`;
  sin música, línea plana (captura).
- **AC-4:** `spectrum::tests::un_tono_de_1_khz_levanta_su_banda`,
  `silencio_da_todo_en_cero`, `la_fft_coincide_con_una_dft_directa`;
  caída en `viz::tests::las_barras_suben_de_golpe_y_bajan_suave`.
- **AC-6:** `tap::tests::guarda_las_ultimas_en_mono_y_sin_volumen` y
  `con_volumen_cero_no_inventa_senal`.
- **AC-7:** `viz::tests::solo_pide_cuadros_mientras_suena` (Sonando sí;
  pausa, cargando y sin tema no). En release, 30 s en reposo con cada
  modo: 0,00 % de CPU, 27–28 MB.
- **AC-11:** con `"modo": "espiral"` la app abrió sin panel y con el
  aviso `visualizacion.modo` (captura).
- **AC-12:** la CLI pasa `None` como tap; `tap::tests::apagado_no_copia`.
- **Tapas (parte de AC-5):** `engine::tests::tapas_solo_si_la_ventana_las_pide`,
  `una_tapa_que_llega_tarde_o_falla_no_se_manda`,
  `queue::tests::la_tapa_mas_cercana_a_300`, `cover::tests::*`. Vinilo
  sin tema: disco con etiqueta del acento y una marca (captura).

Con audio real (2026-09-27, a pedido tuyo, volumen 10 %, release):

- **AC-5:** con Vinilo, 20 `play` de temas de distintos discos (Thriller,
  Abbey Road, Rumours, Nevermind, Back in Black): la tapa de cada uno
  aparece en la etiqueta, girada, y cambia con el tema (capturas).
  Quieto en pausa: `viz::tests::solo_pide_cuadros_mientras_suena` (el
  ángulo solo avanza sonando).
- **T6 / desfase:** instrumento temporal (no quedó en el código) en
  `TapSink::write` anotando muestras escritas vs. tiempo: en régimen, las
  muestras entran a la salida en promedio **335 ms** antes de sonar
  (entre 134 y 595 ms según el momento), coherente con los 26 paquetes de
  ~580 muestras que guarda la salida rodio de librespot (~342 ms).
  `config::viz::LATENCY` = 340 ms.
- **Pendiente:**
  - **AC-8:** la primera corrida midió Onda y Barras sin música (el
    álbum de prueba no existe en Spotify) y dio 0,5 / 5,9 / 8,1 % para
    Onda / Barras / Vinilo sin estar sonando; aislado después (Barras
    elegido por menú, sin música) dio 0,00 %, así que no se pudo
    atribuir. 20 cambios de tema con Vinilo: 40,9 → 43,9 MB (no alcanza
    para saber si se estabiliza). La segunda corrida (con música, pausa
    y 40 cambios) no arrancó: Windows no le dio el foco a la ventana
    mientras usabas la PC y el script se frena antes de mandar teclas.
  - **AC-9 y AC-10:** tuyos, con audio.
