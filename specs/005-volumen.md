# 005 - Control de volumen

## Estado
Verificado

## Contexto
Hoy la app suena siempre al 100 % (el reproductor se arma con
`NoOpVolume`): para bajar la música hay que ir al mezclador de Windows o
al volumen general del sistema, que también baja todo lo demás. Se pide
poder controlar el volumen de la app desde la propia app.

## Preguntas / Supuestos

- **¿Qué volumen se controla?** → Asumido: el volumen **de la app**, no el
  de Windows ni el de otro dispositivo Spotify Connect (no hay: el audio
  sale de librespot embebido). Bajarlo no toca el volumen del sistema ni
  el de otras apps. Motivo: es lo que hace la app oficial y lo que evita
  ir al mezclador.
- **¿Qué comandos?** → Asumido, en la consola de escritorio:

  | Comando | Alias | Qué hace |
  |---|---|---|
  | `vol` | `v` | muestra el volumen actual |
  | `vol <0-100>` | `v <0-100>` | lo fija en ese porcentaje |
  | `vol +` / `vol -` | `v +` / `v -` | sube / baja un paso |
  | `mute` | `m` | silencia / vuelve al volumen de antes |

  El paso de `+`/`-` va en config (asumido: 5 %).
- **¿Atajos de teclado en la ventana?** → Asumido: `Ctrl+↑` / `Ctrl+↓`
  suben / bajan un paso, aunque haya texto a medio escribir (misma lógica
  que `Ctrl+←`/`Ctrl+→` de spec 004). `↑`/`↓` sin Ctrl siguen siendo el
  historial.
- **¿Y en la CLI (`spotify-terminal.exe play …`)?** → Asumido: sí, en la
  pantalla de reproducción `+` y `-` suben / bajan un paso y la pantalla
  muestra el volumen. Motivo: comparten el reproductor y es poco trabajo;
  `m` no se agrega (sin uso asignado hoy, pero para no crecer el alcance).
- **¿Se recuerda al cerrar?** → Asumido: sí. El último volumen se guarda
  en la carpeta de datos (`%APPDATA%\spotify-terminal\`) y la próxima vez
  arranca ahí, en la CLI y en la ventana. Sin archivo (primera vez) o
  ilegible → 100 % (lo de hoy) sin error. Motivo: arrancar siempre al
  100 % después de haberlo bajado puede aturdir.
- **¿Qué curva?** → Asumido: logarítmica (la default de librespot), para
  que cada paso de 5 % se escuche parecido en todo el rango; con lineal,
  de 50 a 100 casi no cambia nada. `0` es silencio total.
- **¿`mute` y después `vol +`?** → Asumido: `vol +`/`vol -`/`vol N` sacan
  del mute (`+`/`-` parten del volumen de antes del mute). `vol` en mute
  muestra "silenciado (70 %)".
- **¿Dónde se ve?** → Asumido: en la barra de "sonando ahora" (ej.
  `vol 70 %`, o `mute`), y cada cambio imprime una línea corta en la
  consola solo si fue por comando (no por atajo, para no llenarla).
- **¿Afecta a los temas que siguen?** → Sí: el volumen se mantiene al
  cambiar de tema, con `play` nuevo, `stop` y al reabrir el reproductor.
- **Fuera de alcance:** normalización de volumen entre temas, ecualizador,
  y atajos globales (con la ventana minimizada o sin foco): ver spec 006,
  que suma `Ctrl+Alt+↑`/`Ctrl+Alt+↓` para el volumen.

## Qué debe pasar (no cómo)
1. Con música sonando, `vol 30` baja el volumen de la app al instante, sin
   cortes ni saltos en el audio, y el resto de las apps sigue igual.
2. Se puede subir, bajar y silenciar con comandos y atajos, y la barra de
   abajo siempre muestra el volumen actual.
3. Al cerrar y volver a abrir (ventana o CLI), suena al último volumen.

## Criterios de aceptación

- [x] **AC-1** — `vol N` (0-100) fija el volumen de la app; se nota en el
      audio en menos de ~0,5 s, sin cortes ni clics audibles, y no cambia
      el volumen de Windows ni el de otras apps.
- [x] **AC-2** — `vol` / `v` muestra el volumen actual; `vol +` y `vol -`
      suben / bajan un paso (config) y quedan en 0-100 (en 100, `+` no
      pasa de 100; en 0, `-` no baja de 0).
- [x] **AC-3** — `mute` / `m` silencia; otro `mute` o cualquier `vol …`
      lo saca, volviendo al volumen de antes (o al nuevo, si fue `vol N`).
- [x] **AC-4** — `Ctrl+↑` / `Ctrl+↓` en la ventana suben / bajan un paso,
      aunque haya texto escrito en la entrada; `↑`/`↓` sin Ctrl siguen
      recorriendo el historial.
- [x] **AC-5** — La barra de "sonando ahora" muestra el volumen (o `mute`)
      y se actualiza con cada cambio.
- [x] **AC-6** — El volumen se mantiene al pasar de tema, con `play`
      nuevo, `stop` + `play`, y al cerrar y reabrir la app.
- [x] **AC-7** — Primera vez (sin volumen guardado) o archivo ilegible →
      arranca al 100 %, sin error visible.
- [x] **AC-8** — `vol` con argumento inválido (`vol 150`, `vol -3`,
      `vol abc`, `vol 1 2`) muestra un error de uso y no cambia nada.
- [x] **AC-9** — En la CLI, `+` / `-` en la pantalla de reproducción
      suben / bajan un paso y la pantalla muestra el volumen; el volumen
      guardado se comparte con la ventana.
- [x] **AC-10** — Cambiar el volumen seguido (mantener `Ctrl+↑`) no
      suma consumo sostenido: la CPU vuelve a ~0 % al soltar y la RAM
      reproduciendo sigue dentro del tope de spec 004 (≤ 100 MB).

## Riesgos / casos de falla
- **Subir de golpe al máximo** (bug en el parseo o en el archivo guardado
  con un valor fuera de rango): el valor se acota a 0-100 al leerlo y al
  fijarlo, nunca se confía en lo guardado.
- **Guardar el volumen en cada paso** (mantener `Ctrl+↑`) escribiría a
  disco muchas veces por segundo: se guarda al cerrar y/o con una espera
  corta después del último cambio, no en cada tecla.
- **Error al guardar** (disco lleno, permisos): se avisa en la consola y la
  app sigue; la reproducción no se entera.

## Plan técnico

*(Implementado con dos desvíos: el volumen le llega a la ventana en su
propio `Output::Volume`, no dentro de `NowPlaying`, porque la barra lo
muestra también sin nada sonando; y al conectar, el motor vuelve a fijar
el volumen solo si cambió mientras conectaba.)*

- **Reproductor** (`src/spotify/player.rs`): el `NoOpVolume` se reemplaza
  por el `SoftMixer` de librespot (ya viene en `librespot-playback`, sin
  dependencias nuevas), con la curva logarítmica por defecto
  (`VolumeCtrl::default()`). El volumen se aplica a cada muestra antes de
  la salida de audio, así que el cambio se oye en el próximo bloque (unos
  ms) sin reabrir nada. Hay que fijarlo al abrir el reproductor, porque el
  `SoftMixer` arranca en 50 %.
  - `Player::connect(token, volume: Volume)`: arranca con ese volumen.
  - `Player::set_volume(&self, volume: Volume)`: nuevo; convierte 0-100 a
    la escala de librespot (0-65535) y a 0 si está en mute.
- **Estado del volumen** (`src/app/volume.rs`, nuevo, sin I/O en la lógica):
  - `Volume { level: u8, muted: bool }`. Invariante: `level` ≤ 100.
  - `set(n)`, `up()`, `down()` (paso de config, acotados a 0-100; los tres
    sacan del mute), `toggle_mute()`, `output() -> u16` (lo que va al
    mixer), `Display` ("70 %", "silenciado (70 %)").
  - `load(dir) -> Volume` (sin archivo, ilegible o fuera de rango → 100 %,
    nunca error) y `save(&self, dir) -> Result<(), AppError>`. Es un
    archivo de texto con el número, en `config::data_dir()`. El mute no se
    guarda: al reabrir suena al último nivel.
- **Motor** (`src/app/engine.rs`, `backend.rs`, `shell.rs`):
  - `Playback::set_volume`, y `Backend::connect(volume)` pasa el volumen
    al abrir.
  - `ShellCommand::Volume(VolumeCommand)` con `Show | Set(u8) | Up | Down`
    y `ShellCommand::Mute`. Alias `v` y `m`, errores de uso de AC-8, y se
    suman al `HELP` y a `NAMES` (Tab).
  - `Input::VolumeUp` / `Input::VolumeDown` para los atajos. No escriben
    en la consola; el comando sí.
  - El `Engine` es dueño del `Volume`, que se carga al arrancar el hilo y
    existe aunque no haya reproductor. Si hay reproductor, cada cambio le
    llega; si no, se aplica al conectar. `NowPlaying` suma `volume` para
    la barra.
  - Guardado con espera: cada cambio arma un plazo
    (`config::VOLUME_SAVE_DELAY`, 2 s) y el `select!` del loop guarda
    cuando vence, sin polling. Al salir del loop se guarda si quedó algo
    pendiente. Si falla, sale un aviso amarillo en la consola.
  - El volumen funciona sin nada sonando: `vol 30` antes de `play` queda
    para cuando suene.
- **Ventana** (`src/desktop/app.rs`): `Ctrl+↑` / `Ctrl+↓` en los atajos
  que se atienden antes que la entrada, y `vol 70 %` / `mute` en la
  barra de "sonando ahora".
- **CLI** (`src/main.rs`, `src/ui/playback.rs`): `main` carga el volumen
  y se lo pasa a `Player::connect`; `play_queue` suma `Action::VolumeUp`
  y `Action::VolumeDown` (`+`/`-`, también `=` porque `+` va con Shift en
  el teclado español) y lo muestra en la línea de estado. Guarda al
  salir.
- **Config** (`src/config.rs`): `VOLUME_STEP` (5), `VOLUME_DEFAULT`
  (100), `VOLUME_FILE` ("volumen.txt"), `VOLUME_SAVE_DELAY` (2 s).
- **Tests:**
  - `app::volume`: límites de `up`/`down`, mute y vuelta, `output()` en 0,
    100 y mute, `load` con archivo ausente, basura o `150`, `save` + `load`
    ida y vuelta (en carpeta temporal).
  - `app::shell`: `vol`, `vol 0`, `vol 100`, `vol +`, `v -`, `m` y los
    errores de AC-8.
  - `app::engine` con el reproductor falso: `vol` sin reproductor se
    aplica al conectar, `vol N` con reproductor llama a `set_volume`, el
    volumen sigue después de `play` nuevo y `stop`, `NowPlaying.volume`, y
    el guardado con la espera (reloj de tokio en pausa).
  - `ui::playback`: `+`, `=` y `-` → acción de volumen.
  - A mano: AC-1 (se oye sin cortes y el mezclador de Windows no cambia),
    AC-4, AC-5, AC-6 al reabrir, AC-9 y AC-10 (medición).
- **Contratos / docs:** doc-comments de `Volume`, `load`/`save`,
  `Player::connect` (firma nueva), `Player::set_volume`, `Playback`,
  `Backend::connect`. En `docs/arquitectura.md`, el módulo `app::volume`
  y que el volumen es del motor (ventana) o de `main` (CLI). En
  `docs/decisiones.md`, softvol de librespot en vez del volumen de
  Windows por app (WASAPI). README y changelog.

## Tareas
*(desglose del plan, se van tildando)*

- [x] T1 — `app::volume` con sus tests; constantes en config.
- [x] T2 — `SoftMixer` en `Player`: `connect(…, volume)` y `set_volume`;
      la CLI pasa el volumen cargado (probar a mano que suena igual al
      100 %).
- [x] T3 — `shell`: `vol` / `v` / `mute` / `m`, ayuda y Tab, con tests.
- [x] T4 — Motor: `Volume` propio, `Input::VolumeUp/Down`, aplicar al
      conectar, `NowPlaying.volume`, guardado con espera, con tests.
- [x] T5 — Ventana: `Ctrl+↑`/`Ctrl+↓` y volumen en la barra.
- [x] T6 — CLI: `+` / `=` / `-`, volumen en la línea de estado y guardar
      al salir.
- [x] T7 — Pruebas a mano de todos los AC + medición (AC-10).
- [x] T8 — Docs: arquitectura, decisiones, README, changelog.

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

Automático: 105 tests (nuevos: `app::volume` — límites, mute, escala del
reproductor, archivo ausente/basura/fuera de rango, ida y vuelta;
`app::shell` — `vol`/`v`/`mute`/`m` y los errores de AC-8;
`app::engine` — volumen sin reproductor aplicado al conectar, cambios con
reproductor, atajo sin línea en la consola, se mantiene con `play` nuevo y
`stop`, cambio mientras conecta, guardado único con espera, aviso si no se
puede guardar, guardado al salir; `ui::playback` — `+`, `=`, `-`).
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y
`cargo test` sin avisos.

A mano (2026-09-27, `spotify-desktop.exe` y `spotify-terminal.exe`
release, Windows 10, con audio real): confirmado por vos ("todo ok")
sobre la lista de pruebas de T7 — `vol 30` baja enseguida sin cortes y el
mezclador de Windows no cambia (AC-1), `Ctrl+↑` sostenido sube de a pasos
y `↑` solo sigue siendo historial (AC-4), `mute` + `vol +` (AC-3), el
volumen se mantiene al reabrir ventana y CLI (AC-6, AC-9), barra con el
volumen (AC-5). AC-10: confirmado por vos junto con el resto; no quedó
registrado un número nuevo de RAM/CPU (la referencia sigue siendo la de
spec 004: 36,9 MB máx. reproduciendo).
