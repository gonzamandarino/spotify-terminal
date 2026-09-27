# 006 - Atajos de teclado globales

## Estado
Verificado

## Contexto
Los atajos de la app de escritorio (spec 004: `Ctrl+Espacio`, `Ctrl+→`,
`Ctrl+←`) solo andan con la ventana enfocada. Para pausar o pasar de tema
mientras se usa otra cosa hay que buscar la ventana, restaurarla y
volver. Se piden combinaciones de teclas propias que anden con la app
minimizada o en segundo plano: pausar, reanudar, siguiente y stop.

Decisión tomada al pedir el spec: **combinaciones propias globales**, no
las teclas multimedia del teclado (⏯ ⏭) ni el panel multimedia de
Windows.

## Preguntas / Supuestos

- **¿Qué combinaciones?** → Respondido por vos: todas con `Ctrl+Alt`, y
  stop en `Enter` (se propuso `Fin`). Se probó `Ctrl+Shift` y se descartó:
  tapaba la selección de texto de las demás apps.
  Pausa en `P`: `Ctrl+Alt+Espacio` ya estaba registrada por otra app en
  esta PC (`RegisterHotKey` → error 1409; según vos, Claude). El resto
  se probó igual y estaba libre.

  | Combinación | Acción | Igual que |
  |---|---|---|
  | `Ctrl+Alt+P` | pausa / reanudar | `pause` |
  | `Ctrl+Alt+→` | siguiente tema | `next` |
  | `Ctrl+Alt+←` | anterior (regla de 3 s) | `prev` |
  | `Ctrl+Alt+Enter` | stop (corta y vacía la cola) | `stop` |
  | `Ctrl+Alt+↑` / `Ctrl+Alt+↓` | sube / baja un paso de volumen | `vol +` / `vol -` |

  "Frenar" y "darle play de nuevo" son la misma combinación (alterna),
  como `pause`. Motivo: menos combinaciones que recordar. `anterior` se
  agrega aunque no se pidió, porque sin él `siguiente` no tiene vuelta.
- **¿Depende del spec de volumen?** → Sí: las combinaciones de volumen
  se implementan después de spec 005. Si 005 no está implementado, este
  spec se hace sin ellas y `AC-5` queda pendiente (`Reabierto (parcial)`).
- **¿Se pueden cambiar las combinaciones?** → Asumido: van en
  `src/config.rs` (regla del repo: nada hardcodeado en la lógica), pero
  cambiarlas requiere recompilar. Un archivo de configuración editable
  por el usuario queda fuera de alcance. Motivo: proyecto personal, y
  alcanza con editar la constante.
- **¿Qué pasa si otra app ya usa una combinación?** → Asumido: esa
  combinación no se registra, la consola lo avisa en amarillo al abrir
  (ej. "Ctrl+Alt+→ ya lo usa otra app: sin atajo global para
  siguiente"), y el resto funciona igual. La app no falla ni se cierra.
- **¿Solo en la app de escritorio?** → Asumido: sí. La CLI
  (`spotify-terminal.exe`) no registra atajos globales. Motivo: la CLI
  vive en una terminal que ya tiene sus teclas, y dos procesos no pueden
  registrar la misma combinación.
- **¿Dos ventanas abiertas?** → Asumido: la primera se queda con los
  atajos y la segunda avisa que están tomados (mismo caso que "otra app
  los usa"). Una sola instancia sigue fuera de alcance (spec 004).
- **¿Y con la ventana enfocada?** → Las combinaciones globales andan
  también, y hacen una sola acción (no se duplican con los atajos de
  spec 004, que no cambian).
- **¿Algún aviso al apretarlas?** → Asumido: no aparece notificación ni
  sonido de Windows. Solo se actualiza la barra de "sonando ahora" y no
  se escribe en la consola, para no llenarla. Motivo: el efecto se
  escucha.
- **¿Sin nada sonando?** → Pausa / siguiente / anterior / stop sin nada
  cargado no hacen nada (como los comandos hoy), sin error visible.
- **¿Dónde se ven?** → `help` suma una sección "Atajos globales" con las
  combinaciones activas.
- **Consumo** → Sin polling: Windows avisa a la app cuando se aprieta
  una combinación registrada. No se instala un hook de teclado global
  (que vería todas las teclas del sistema).

## Qué debe pasar (no cómo)
1. Con la app minimizada o detrás de otra ventana y música sonando,
   `Ctrl+Alt+P` pausa; otra vez, reanuda. `Ctrl+Alt+→` pasa al
   siguiente, `Ctrl+Alt+Enter` corta todo.
2. Si una combinación no se puede usar, la app lo dice al abrir y sigue
   andando con las demás.
3. Al cerrar la app, las combinaciones quedan libres para otras apps.

## Criterios de aceptación

- [x] **AC-1** — Con la ventana minimizada y música sonando,
      `Ctrl+Alt+P` pausa y otra vez reanuda, sin cortes ni demora
      perceptible (< ~0,5 s).
- [x] **AC-2** — `Ctrl+Alt+→` / `Ctrl+Alt+←` pasan al siguiente / anterior
      (regla de 3 s de spec 003) con la ventana minimizada o con otra
      app enfocada.
- [x] **AC-3** — `Ctrl+Alt+Enter` corta la reproducción y vacía la cola,
      igual que `stop`; la barra de "sonando ahora" queda vacía.
- [x] **AC-4** — Con la ventana enfocada, cada combinación hace su acción
      una sola vez, y los atajos de spec 004 (`Ctrl+Espacio`, `Ctrl+→`,
      `Ctrl+←`) siguen andando.
- [x] **AC-5** — `Ctrl+Alt+↑` / `Ctrl+Alt+↓` suben / bajan un paso de
      volumen (requiere spec 005).
- [x] **AC-6** — Si una combinación ya está tomada (probado abriendo una
      segunda ventana), la consola avisa cuál y la app sigue con las
      demás; ninguna falla la cierra.
- [x] **AC-7** — Al cerrar la app (`exit`, botón o cerrar desde la barra
      de tareas), las combinaciones quedan libres (una app nueva puede
      registrarlas).
- [x] **AC-8** — Sin nada sonando, las combinaciones no hacen nada y no
      muestran errores.
- [x] **AC-9** — `help` lista los atajos globales activos.
- [x] **AC-10** — Con la app en reposo, la CPU sigue en ~0 % (sin
      polling) y la RAM reproduciendo, dentro del tope de spec 004
      (≤ 100 MB). Medido con `scripts/medir-consumo.ps1`.
- [x] **AC-11** — La CLI (`spotify-terminal.exe`) no cambia de
      comportamiento.

## Riesgos / casos de falla
- **Tapar un atajo de otra app o de Windows:** una combinación registrada
  deja de llegar a las demás apps mientras la nuestra está abierta.
  `Ctrl+Alt+…` casi no se usa en otras apps (se probó `Ctrl+Shift`, pero
  tapaba "seleccionar palabra / línea" en todos los editores). La lista
  va en config para cambiarla si choca.
- **Rotar la pantalla:** en algunas PCs con gráficos Intel,
  `Ctrl+Alt+flechas` rota la pantalla, y el driver puede tomar la
  combinación antes que la app sin que falle el registro. Se verifica a
  mano en esta PC (AC-2, AC-5); si pasa, se desactiva el atajo del driver
  o se cambian las flechas en config.
- **Crash con los atajos registrados:** Windows los libera solo cuando
  el proceso termina, así que no quedan "tomados" después de un crash.
- **Stop por error:** `Ctrl+Alt+Enter` corta y vacía la cola, sin
  confirmar. Si molesta, se cambia o se saca de la lista en config.

## Plan técnico

*(Implementado con un desvío: en vez de `Input::Stop` y un campo `quiet`
en los controles, un solo `Input::Global(GlobalAction)` para todos los
atajos globales. El motor lo atiende sin escribir en la consola, y sin
nada sonando no hace nada salvo el volumen. La tecla de pausa quedó en
`Ctrl+Alt+P`: ver "Preguntas / Supuestos".)*

- **Mecanismo:** `RegisterHotKey` de Windows (user32). Cuando se aprieta
  una combinación registrada, Windows le manda un mensaje `WM_HOTKEY` a la
  app, que estaba dormida esperando mensajes: no hay polling ni hook de
  teclado. Se usa `windows-sys`, que ya es dependencia (0.52, la de
  winit). Solo se suman features de la misma crate:
  `Win32_UI_Input_KeyboardAndMouse` y `Win32_System_Threading`. No entra
  `global-hotkey`: sumaría otra crate para ~60 líneas de Win32.
- **Hilo propio "atajos"** (`src/desktop/hotkeys.rs`, nuevo, solo
  Windows): registra las combinaciones con `hWnd = NULL`, así los
  mensajes llegan a la cola de ese hilo, y espera con `GetMessageW`, que
  bloquea sin gastar CPU. Con cada `WM_HOTKEY` manda el `Input`
  correspondiente al motor por el mismo canal que usa la ventana. No
  pasa por el loop de winit, así que anda igual con la ventana minimizada
  y no depende de que se redibuje. El motor despierta a la ventana como
  siempre, para la barra.
  - `hotkeys::spawn(inputs) -> Result<Hotkeys, AppError>`: arranca el
    hilo, espera a que termine de registrar y devuelve cuáles quedaron
    activas y cuáles fallaron (con la combinación y la acción).
  - `Drop for Hotkeys`: `PostThreadMessageW(WM_QUIT)` al hilo, que
    desregistra todo (`UnregisterHotKey`) y termina; después, `join`.
  - Pausa, siguiente, anterior y stop se registran con `MOD_NOREPEAT`
    (mantener apretado = una vez). El volumen se registra sin esa opción:
    mantener `Ctrl+Alt+↑` sube de a pasos, como `Ctrl+↑` en la ventana.
- **Tipos y config:**
  - `hotkeys::Action` (`TogglePause`, `Next`, `Prev`, `Stop`, `VolumeUp`,
    `VolumeDown`) → `engine::Input`, y `hotkeys::Key` (`Space`, `Enter`,
    flechas) → virtual-key de Windows y nombre para mostrar (`→`,
    `Espacio`).
  - `config::GLOBAL_SHORTCUTS: &[(Action, Key)]` con la tabla del spec.
    El modificador (`Ctrl+Alt`) es un solo valor en config, igual para
    todas.
- **Motor** (`src/app/engine.rs`):
  - `Input::Stop`, igual que `stop` pero sin escribir en la consola (sin
    nada sonando no avisa: AC-8). Pausa, siguiente y anterior ya existen
    como `Input`, pero hoy avisan "No suena nada" sin nada sonando. Con
    un campo `quiet` en el control, los que vienen de un atajo global no
    escriben nada.
  - `Input::GlobalShortcuts(Vec<String>)`: la lista de combinaciones
    activas ("Ctrl+Alt+→  siguiente"), que `help` agrega al final en una
    sección "Atajos globales (con la app minimizada)". Sin ninguna activa,
    la sección no aparece.
- **Arranque** (`src/desktop/mod.rs`): después del motor,
  `hotkeys::spawn(inputs.clone())`. Si alguna combinación falla, la
  consola arranca con un aviso amarillo por cada una ("Ctrl+Alt+→ ya lo
  usa otra app: sin atajo global para siguiente"). Si falla el hilo
  entero, un aviso y la app sigue sin atajos globales. Al cerrar, los
  `Hotkeys` se sueltan antes de esperar al motor.
- **CLI:** sin cambios (AC-11).
- **Tests:**
  - `hotkeys`: acción → `Input`, nombre de cada combinación, que ninguna
    combinación de `config::GLOBAL_SHORTCUTS` esté repetida, y un test
    real en Windows. Registra una combinación que nadie usa
    (`Ctrl+Alt+Shift+F24`), comprueba que un segundo registro falla
    (AC-6), la suelta y comprueba que se puede registrar de nuevo (AC-7).
  - `engine`: `Input::Stop` con y sin nada sonando (sin línea en la
    consola), controles silenciosos sin nada sonando, `help` con y sin
    atajos globales.
  - A mano: AC-1 a AC-5 con la app minimizada y con otra app enfocada.
    AC-2 y AC-5 prueban también que el driver de gráficos no rote la
    pantalla con `Ctrl+Alt+flechas`. AC-6 abriendo una segunda ventana,
    y AC-10 con la medición.
- **Contratos / docs:** doc-comments de `hotkeys::spawn`, `Hotkeys`,
  `Action`, `Key`, `Input::Stop`, `Input::GlobalShortcuts`. En
  `docs/arquitectura.md`, el hilo "atajos" en el mapa y en "Hilos", y que
  manda `Input` al motor como la ventana. En `docs/decisiones.md`,
  `RegisterHotKey` en un hilo propio en vez de un hook de teclado, del
  hook de mensajes de winit o de la crate `global-hotkey`. README (tabla
  de atajos globales) y changelog.

## Tareas
*(desglose del plan, se van tildando)*

- [x] T1 — `hotkeys::Action`/`Key`, config y tests de nombres y mapeo.
- [x] T2 — Hilo "atajos": registrar, loop de `GetMessageW`, soltar al
      cerrar; test real de registro doble y liberación.
- [x] T3 — Motor: `Input::Stop`, controles silenciosos,
      `Input::GlobalShortcuts` y sección en `help`, con tests.
- [x] T4 — `desktop::run`: arrancar el hilo, avisos de las que fallaron y
      cierre ordenado.
- [x] T5 — Pruebas a mano de todos los AC + medición (AC-10).
- [x] T6 — Docs: arquitectura, decisiones, README, changelog.

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

Automático: 112 tests (nuevos: `desktop::hotkeys` con nombres,
virtual-keys, config sin repetidos y un test contra Windows de verdad
—combinación tomada por otro hilo falla y al soltarla queda libre—;
`app::engine` con atajos globales con y sin nada sonando, sin líneas en
la consola, y `help` con la lista). `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings` y `cargo test` sin avisos.

A mano (2026-09-27, `spotify-desktop.exe` release, Windows 10):
- Chequeo previo: `Ctrl+Alt+Espacio` ya estaba registrada por otra app
  (error 1409), por eso pausa va en `Ctrl+Alt+P`.
- AC-6/AC-7 en la app real: con la ventana abierta las 6 combinaciones
  figuran tomadas para otro proceso; al cerrarla, las 6 quedan libres.
- AC-1 a AC-6 y AC-8/AC-9 con audio real: confirmado por vos ("todo ok")
  sobre la lista de pruebas de T5, incluida la segunda ventana con sus
  avisos y que `Ctrl+Alt+flechas` no rota la pantalla.
- AC-10: confirmado por vos junto con el resto; no quedó registrado un
  número nuevo de RAM/CPU (referencia: spec 004, 36,9 MB máx.). El hilo
  de atajos duerme en `GetMessageW`, sin timers.
- AC-11: la CLI no cambió (sin cambios en `main`/`ui`; sus tests pasan).
