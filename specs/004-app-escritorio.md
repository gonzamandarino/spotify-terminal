# 004 - App de escritorio con consola propia

## Estado
Verificado

## Contexto
Hoy el cliente es un comando de consola: cada `login`, `play`, `whoami` es
un proceso aparte que se lanza desde PowerShell, y mientras suena algo la
terminal queda tomada por la pantalla de reproducción (teclas de una letra).
Se pide una **app de escritorio**: se abre con doble clic y muestra una
ventana con una consola propia, estilizada, donde se escriben los comandos
directamente, sin pasar por PowerShell.

Decisión tomada al pedir el spec: la ventana es **propia, en Rust** (no
Windows Terminal con un perfil ni Tauri/WebView). Motivo: un solo `.exe`,
control total del look y menos memoria que un webview. Windows Terminal no
está instalado en la máquina de prueba.

## Preguntas / Supuestos

- **¿Con qué se dibuja la ventana?** → Respondido: ventana propia en Rust.
  Asumido: `eframe`/`egui` con el backend `glow` (OpenGL), no `wgpu`
  (motivo: `glow` pesa menos en binario y RAM; egui redibuja solo ante
  eventos, así que en reposo no gasta CPU). Se justifica en
  `docs/decisiones.md` como dependencia pesada.
  **Cambiado en T5:** con `glow` la ventana sola ocupaba 120 MB (driver
  OpenGL de NVIDIA). Se usa egui dibujado por CPU (`egui_software_backend`
  + `softbuffer` + `winit`): 19 MB. Ver `docs/decisiones.md`.
- **¿Reemplaza a la versión de consola?** → Asumido: no, conviven. Salen
  dos ejecutables del mismo crate: `spotify-terminal.exe` (el de hoy, sin
  cambios de uso) y `spotify-desktop.exe` (la ventana, sin consola negra
  detrás). Motivo: un `.exe` con subsistema "windows" pierde la salida de
  consola, así que no puede ser los dos a la vez.
- **¿Qué se escribe en la consola?** → Asumido: los mismos comandos que la
  CLI, sin el nombre del programa: `play …` (con `list`, `-s` y links),
  `login`, `logout`, `whoami`, `help`. Además, comandos nuevos para
  controlar lo que suena mientras se sigue escribiendo:

  | Comando | Alias | Qué hace |
  |---|---|---|
  | `pause` | | pausa / reanudar |
  | `next` | `n` | siguiente tema |
  | `prev` | `p` | anterior (misma regla de 3 s que spec 003) |
  | `shuffle` | `s` | shuffle sí / no |
  | `queue <tema>` | `a <tema>` | busca y encola (elegir con 1-5) |
  | `stop` | | corta la reproducción y vacía la cola |
  | `clear` | | limpia la consola |
  | `exit` | | cierra la app |

  Los alias de una letra son los mismos que las teclas de spec 003.
- **¿La música sigue mientras escribo otro comando?** → Asumido: sí. `play`
  ya no toma la consola: la reproducción corre de fondo y la consola queda
  libre. Un `play` nuevo reemplaza lo que sonaba (cola incluida), como en
  la app oficial.
- **¿Cómo se elige un resultado de búsqueda?** → Asumido: se imprime la
  lista numerada como en spec 002 y el prompt pasa a "Elegí 1-5"; se
  escribe el número + Enter (Enter solo = el primero), Esc cancela. Otro
  comando escrito ahí cancela la elección y se ejecuta.
- **¿Atajos de teclado en la ventana?** → Asumido: `Ctrl+Espacio` pausa,
  `Ctrl+→`/`Ctrl+←` siguiente/anterior, funcionan aunque haya texto a medio
  escribir. `↑`/`↓` recorren el historial de comandos de la sesión, `Tab`
  completa el nombre del comando. El historial no se guarda al cerrar.
- **¿Cómo se ve?** → Asumido (ajustable al revisar):
  - Tema oscuro tipo Spotify: fondo `#121212`, paneles `#181818`, acento
    verde `#1DB954`, texto `#E0E0E0`, secundario `#8A8A8A`, errores
    `#F15E6C`, avisos `#F5C451`. Colores en config, no en la lógica.
  - Fuente monoespaciada embebida en el binario: **JetBrains Mono** (OFL,
    ~270 KB), para que se vea igual en cualquier máquina.
  - Barra de título propia (sin el marco de Windows): nombre de la app,
    arrastrar para mover, minimizar y cerrar; bordes redondeados.
    **Cambiado en T6:** sin bordes redondeados (GDI no tiene
    transparencia por pixel): ventana rectangular con borde de 1 px. Se
    agregó maximizar (botón y doble clic) y cambiar el tamaño desde los
    bordes.
  - Arriba, la consola con scroll: prompt `♫ ›` en verde, eco de cada
    comando, salida en color por tipo (normal / éxito / aviso / error).
  - Abajo, barra fija de "sonando ahora": tema — artistas, posición en la
    lista (`[3/12]`/`[cola]`), barra de progreso con tiempo `1:23 / 3:45`,
    y los indicadores de shuffle y temas en cola. Vacía si no suena nada.
  - Ícono de la app (ventana y `.exe`).
- **¿Login desde la ventana?** → Asumido: `login` abre el navegador igual
  que hoy y la consola muestra "Esperando autorización en el navegador…".
  librespot-oauth imprime el link por stdout, que en la ventana no se ve:
  la consola muestra el mismo aviso para que no parezca colgada. Si falta
  `.env` o la sesión, la app abre igual y lo dice en la consola (hoy
  sería un error y salida).
- **¿Cómo se abre?** → Asumido: doble clic en `spotify-desktop.exe`, y un
  script `scripts/instalar-acceso-directo.ps1` que crea el acceso directo
  en el escritorio y el menú Inicio. Instalador `.msi`: fuera de alcance.
- **¿Una sola instancia?** → Asumido: no se controla en este spec (dos
  ventanas = dos reproductores independientes, como dos consolas hoy).
- **¿Cuánta memoria puede sumar la ventana?** → Asumido: objetivo ≤ 100 MB
  reproduciendo (60 MB de spec 001 + hasta 40 MB por la ventana) y ~0 % de
  CPU con la ventana en reposo. Si cuesta más, gana que se vea bien y no
  corte, y se documenta el número real (misma regla que spec 001).
- **Alcance** → Asumido: la ventana es otra "cara" de las funciones que ya
  existen. No entran funciones nuevas de Spotify (likes, playlists del
  usuario, generador): cada una sigue con su spec, y se suman a la consola
  cuando existan.

## Qué debe pasar (no cómo)
1. Doble clic en `spotify-desktop.exe` abre una ventana oscura con la
   consola lista para escribir y un mensaje de bienvenida con `help`. No
   aparece una consola negra detrás.
2. Todo lo que hoy se hace con `spotify-terminal.exe <comando>` se hace
   escribiendo `<comando>` en la ventana, con la misma salida en colores.
3. Mientras algo suena, la consola sigue aceptando comandos; la barra de
   abajo muestra qué suena y cómo avanza. Controlar la reproducción no la
   corta ni la entrecorta.
4. `spotify-terminal.exe` sigue funcionando igual que después de spec 003.

## Criterios de aceptación

- [x] **AC-1** — `spotify-desktop.exe` abre una ventana con barra de título
      propia (mover, minimizar, cerrar), la consola con el prompt y el foco
      en la línea de entrada; sin ventana de consola detrás.
- [x] **AC-2** — `help`, `login`, `logout` y `whoami` escritos en la
      ventana dan el mismo resultado que en la CLI; los errores (sin
      `.env`, sin sesión, sin red) se muestran en rojo en la consola y la
      app sigue abierta.
- [x] **AC-3** — `play <nombre>` y `play list <nombre>` muestran los 5
      resultados numerados; `2` + Enter reproduce el segundo, Enter solo
      el primero, Esc cancela.
- [x] **AC-4** — `play <link>` y `play -s <…>` funcionan como en la CLI
      (tema, álbum, playlist; arrancar mezclado).
- [x] **AC-5** — Con música sonando se puede escribir y ejecutar cualquier
      comando; `whoami` o una búsqueda no cortan ni entrecortan el audio.
- [x] **AC-6** — `pause`, `next`/`n`, `prev`/`p`, `shuffle`/`s` y
      `Ctrl+Espacio`, `Ctrl+→`, `Ctrl+←` se comportan como las teclas de
      spec 003 (incluida la regla de 3 s de "anterior").
- [x] **AC-7** — `queue <tema>`/`a <tema>` busca, se elige con 1-5 y el
      tema suena después del actual, antes del resto de la lista.
- [x] **AC-8** — Un `play` con algo sonando reemplaza la reproducción y la
      cola; `stop` corta y la barra de abajo queda vacía.
- [x] **AC-9** — La barra de "sonando ahora" muestra tema, artistas,
      posición en la lista, progreso y tiempo, shuffle y cantidad en cola,
      y se actualiza sola al cambiar de tema, pausar o avanzar.
- [x] **AC-10** — `↑`/`↓` recorren el historial de la sesión, `Tab`
      completa comandos, `clear` limpia y `exit` (o cerrar la ventana)
      corta el audio y termina el proceso.
- [x] **AC-11** — Un comando desconocido o mal escrito muestra el error de
      uso y la ayuda corta, sin cerrar nada.
- [x] **AC-12** — Con la ventana abierta y sin reproducir, la CPU queda en
      ~0 % (sin redibujar en loop); reproduciendo, la RAM total queda
      ≤ 100 MB en 10 min (o se documenta el valor real en
      `docs/decisiones.md`). Medido con `scripts/medir-consumo.ps1`.
- [x] **AC-13** — `spotify-terminal.exe` (CLI) pasa sus tests y se
      comporta igual que antes de este spec.
- [x] **AC-14** — `scripts/instalar-acceso-directo.ps1` crea accesos
      directos (escritorio y menú Inicio) con el ícono de la app.

## Riesgos / casos de falla
- **La UI bloquea el audio:** la ventana corre en el hilo principal (egui
  lo exige); la Web API y los comandos van en un hilo aparte con su
  runtime tokio, y la sesión de audio sigue en el suyo (decisión de spec
  001). Un redibujo lento no puede demorar las claves de audio.
- **Cierre con la música sonando:** cerrar la ventana tiene que parar el
  reproductor y soltar la sesión en el mismo orden que hoy (`Player::drop`);
  si no, puede quedar audio sonando sin ventana.
- **Panic en la ventana o en librespot:** hoy el hook restaura la terminal;
  en la ventana no hay terminal. El panic se escribe en un log en
  `%APPDATA%\spotify-terminal\` para que no desaparezca en silencio.
- **Sin OpenGL** (VM, driver roto): la ventana no abre; se muestra un
  cuadro de error nativo con el motivo en vez de salir en silencio.

## Plan técnico

*(Implementado con dos desvíos, anotados en "Preguntas / Supuestos": dibujo
por CPU en vez de `eframe`/`glow`, y ventana sin bordes redondeados. Con
eso, `desktop` queda en `mod.rs`, `window.rs` (loop de winit propio, porque
el del crate no soporta arrastrar ni cambiar el tamaño), `app.rs` y
`theme.rs`; la barra de título está dentro de `app.rs`.)*

- **Estructura del crate:**
  - `src/lib.rs` nuevo: expone `config`, `error`, `spotify`, `ui`, `app`,
    `desktop`. `src/main.rs` queda como el binario CLI (usa la lib).
  - `src/bin/desktop.rs`: binario `spotify-desktop`, con
    `#![windows_subsystem = "windows"]`; solo arranca `desktop::run()`.
- **Separar la cola de la terminal** (`src/ui/playback.rs` → `src/app/`):
  - `app::queue` — `Queue`, `Clock`, `Step`, `Entry` salen tal cual de
    `playback.rs` con sus tests (sin cambios de comportamiento).
    `Queue` pasa a tener sus temas (`Vec<SpotifyUri>`) en vez de un
    `&'a [SpotifyUri]`, para vivir en una tarea de fondo.
  - `ui::playback` sigue siendo la pantalla de la CLI, usando `app::queue`.
- **Núcleo de la consola** (`src/app/`), sin nada de egui:
  - `app::shell` — parseo de una línea escrita: reutiliza `cli::parse`
    (partiendo la línea respetando comillas) y agrega `ShellCommand`
    (`Pause`, `Next`, `Prev`, `Shuffle`, `Queue(String)`, `Stop`, `Clear`,
    `Exit`, `Pick(usize)`, `Cli(Command)`), alias y `complete(prefix)`
    para `Tab`.
  - `app::engine` — `Engine`, corre en un hilo "motor" con runtime tokio
    `current_thread`. Recibe `ShellCommand` por un canal `mpsc` y emite
    `Output` por otro: `Line { kind: Normal|Ok|Warn|Error, text }`,
    `NowPlaying(Option<NowPlaying>)`, `Prompt(PromptMode)`, `Clear`.
    Dueño del `Player` (se abre al primer `play` y se reutiliza), del
    `WebClient` y de la `Queue` activa; su loop `select!` junta comandos,
    eventos del reproductor y búsquedas en curso (la misma idea que
    `play_queue`, sin teclado). Cada `Output` hace `ctx.request_repaint()`
    a través de un callback, sin polling.
- **Ventana** (`src/desktop/`, única parte que depende de egui):
  - `desktop::mod` — `run()`: arma el `NativeOptions` (sin decoración,
    transparente para bordes redondeados, tamaño inicial y mínimo desde
    config, ícono), arranca el motor y `eframe::run_native`. Si falla,
    `MessageBoxW` con el error.
  - `desktop::app` — `DesktopApp: eframe::App`: guarda el scrollback
    (tope de líneas en config), la línea de entrada, el historial, el
    `NowPlaying`; en `update` drena el canal de `Output`, dibuja título,
    consola y barra de reproducción, y manda los comandos. Mientras suena,
    `request_repaint_after(1 s)` para el progreso; en pausa o sin nada,
    ninguno.
  - `desktop::theme` — colores, tamaños y fuente (JetBrains Mono embebida
    con `include_bytes!` desde `assets/fonts/`, con su licencia OFL).
  - `desktop::titlebar` — barra de título propia (arrastre con
    `ViewportCommand::StartDrag`, minimizar, cerrar).
- **Config** (`src/config.rs`): colores (`THEME_*`), tamaños de ventana,
  tope de scrollback, intervalo de repintado del progreso, tamaño de
  historial, nombre del archivo de log de panics.
- **Assets:** `assets/fonts/JetBrainsMono-Regular.ttf` + `OFL.txt`,
  `assets/icon.ico` / `.png`; ícono del `.exe` con `winresource` en
  `build.rs` (solo en Windows).
- **Dependencias nuevas** (a justificar en `docs/decisiones.md`):
  `eframe` (default-features off; `glow`, `default_fonts` off),
  `winresource` (build), `windows-sys` solo con la feature de
  `MessageBoxW` (ya la trae el árbol de dependencias; verificar versión).
- **Tests:**
  - `app::shell`: parseo de cada comando y alias, comillas, `Pick`,
    errores de uso, `complete`.
  - `app::queue`: los tests que ya existen, movidos.
  - `app::engine`: con un `Player` falso detrás de un trait chico
    (`Playback`: play/preload/pause/resume/restart/stop/events) —
    `play` nuevo reemplaza la cola, `stop` vacía `NowPlaying`, comandos de
    control sin nada sonando avisan sin error, búsqueda + `Pick` encola.
  - La ventana: sin tests automáticos; AC-1, AC-9, AC-10 y AC-12 a mano.
- **Contratos / docs:** doc-comments de `Engine`, `ShellCommand`, `Output`,
  `shell::parse_line`, `Queue` (al moverla), `desktop::run`.
  `docs/arquitectura.md`: nuevo mapa (lib + 2 binarios), hilo "motor",
  quién posee la `Queue` y el `Player` en la app de escritorio.
  `docs/decisiones.md`: egui/glow, dos binarios, fuente embebida, consumo
  medido. README: sección de la app de escritorio.

## Tareas

- [x] T1 — `lib.rs` + mover `Queue`/`Clock`/`Step` a `app::queue` (con
      sus tests); la CLI sigue igual (AC-13). Commit aparte.
- [x] T2 — Trait `Playback` sobre `Player` para poder testear el motor.
- [x] T3 — `app::shell`: parseo de línea, alias, `Tab`, con tests.
- [x] T4 — `app::engine`: loop de comandos + eventos + búsquedas, con
      tests con reproductor falso.
- [x] T5 — Spike: ventana mínima de eframe/glow sin decoración; medir RAM
      y CPU en reposo antes de seguir (si se va muy arriba de 40 MB, se
      vuelve a este spec a decidir).
- [x] T6 — `desktop`: consola, historial, colores por tipo de línea, `Tab`.
- [x] T7 — Barra de título propia y barra de "sonando ahora".
- [x] T8 — Fuente, ícono, `build.rs`, log de panics, `MessageBoxW`.
- [x] T9 — `scripts/instalar-acceso-directo.ps1`.
- [x] T10 — Pruebas a mano de todos los AC + medición (AC-12).
- [x] T11 — Docs: arquitectura, decisiones, README, changelog.

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

Automático: 94 tests (`app::shell`, `app::engine` con reproductor falso,
`app::queue`, `desktop::app`), `cargo fmt --check` y
`cargo clippy --all-targets -- -D warnings` sin avisos.

A mano (2026-09-27, `spotify-desktop.exe` release, Windows 10), sin audio
porque la CLI estaba sonando:
- **AC-1:** ventana con barra de título propia (minimizar, maximizar,
  cerrar, arrastre), foco en la entrada, sin consola detrás.
- **AC-2:** `help` y `whoami` bien; errores en rojo sin cerrar.
  `login`/`logout`: confirmado por vos.
- **AC-3:** `play list rock` → 5 playlists, prompt `1-5 ›`, `9` avisa
  fuera de rango, Esc cancela. Elegir un resultado y que suene: confirmado
  por vos con audio real ("sonó bien").
- **AC-10 (parcial):** ↑/↓ y Tab bien a velocidad humana (con teclas
  simuladas todas en el mismo frame, Tab se procesaba antes que el texto;
  se corrigió que dos ↑ en un frame contaran como uno). `exit` y cerrar
  con audio sonando: confirmado por vos.
- **AC-11:** `bailar`, `zz` → error de uso con "Escribí `help`…".
- **AC-12:** en reposo 26 MB de RAM y 0 ms de CPU en 10 s. Reproduciendo
  una playlist 10 min (`medir-consumo.ps1 -Proceso spotify-desktop`, 120
  muestras): RAM promedio 36,2 MB, máximo 36,9 MB (privada: máx.
  21,4 MB); CPU promedio 0,07 %, máximo 0,16 %. Tope: 100 MB.
- **AC-4 a AC-9, AC-13:** probados con audio real por vos ("anduvo
  todo"): links y `-s`, controles y atajos con música, encolar, `play`
  nuevo y `stop`, barra "sonando ahora", la CLI igual que antes. La
  barra se vio avanzar sola en la medición de AC-12 (tema, artistas,
  `[1/50]`, progreso).
- **AC-14:** el script creó el acceso directo (probado en una carpeta
  temporal): apunta al `.exe`, arranca en el repo, ícono del `.exe`.
