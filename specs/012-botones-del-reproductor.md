# 012 - Botones en el reproductor

## Estado
En verificación

Falta la prueba manual con algo sonando (T6: AC-1, AC-8, AC-10).

## Contexto
La barra "sonando" de la app de escritorio (debajo de la consola) solo
muestra información: estado, tema, artistas, progreso, 🔀 y volumen. Para
controlar la reproducción hay que escribir comandos o usar atajos. Se piden
botones clickeables en esa barra para:

1. Anterior / siguiente tema.
2. Pausar / reanudar.
3. Shuffle.
4. Like.

**Depende del spec 011**: el botón de like usa el guardado en Tus me gusta
y el estado ♥ que agrega ese spec. Se implementa después de 011.

## Preguntas / Supuestos

- **¿Dónde van en la barra?** → Asumido, estilo app oficial:
  `⏮ ⏯ ⏭` centrados en la fila de arriba; ♥ pegado a la derecha del
  título/artistas; el 🔀 que hoy es solo indicador pasa a ser el botón de
  shuffle (mismo lugar, entre la cola y el volumen). La fila de abajo
  (tiempo y progreso) no cambia. Si el ancho no alcanza, se corta primero
  el texto del título/artistas con "…"; los botones siempre se ven.
- **¿El botón de like alterna?** → Asumido: sí, como el corazón de la app
  oficial: lleno → click lo quita, apagado → click lo agrega. Motivo: acá
  el estado está a la vista (a diferencia del comando `like`, spec 011).
- **¿Qué hace cada botón por dentro?** → Asumido: lo mismo que su comando
  (`prev`, `pause`, `next`, `shuffle`, `like`/`unlike`), con los mismos
  mensajes en la consola. No hay un camino paralelo.
- **¿Sin nada sonando?** → Asumido: los botones se ven deshabilitados
  (atenuados) y no hacen nada; la barra sigue mostrando "Nada sonando…".
- **¿Foco del teclado?** → Asumido: clickear un botón no le saca el foco a
  la línea de entrada; se puede seguir escribiendo sin volver a clickear
  la consola.
- **¿Tooltip?** → Asumido: al pasar el mouse, el nombre de la acción y su
  atajo de ventana si tiene (según los ajustes del spec 007).
- **¿Color del ícono de estado en pausa?** → Respondido por vos (en la
  prueba manual): el ícono al lado del nombre del tema (▶ / ⏸) usa el
  mismo color sonando y en pausa (el de acento); antes, en pausa era el de
  advertencia.
- **¿Tamaño?** → Asumido: escalan con el tamaño de letra (spec 009), como
  el resto de la barra; área clickeable mínima de config.

## Qué debe pasar (no cómo)

Con algo sonando, la barra de abajo tiene botones para anterior, pausa /
reanudar, siguiente, shuffle y like, que hacen lo mismo que sus comandos
y muestran el estado actual (⏸/▶, shuffle encendido/apagado, ♥ lleno o
apagado).

## Criterios de aceptación

- [ ] **AC-1** — Con un tema sonando, la barra "sonando" muestra ⏮, ⏯ y ⏭
      centrados en la fila de arriba, ♥ junto al título y 🔀 a la derecha,
      sin superponerse con el texto ni con la barra de progreso.
- [x] **AC-2** — Click en ⏭ / ⏮ hace lo mismo que `next` / `prev`
      (incluido reiniciar el tema si pasaron más de unos segundos, como el
      comando) y deja la misma línea en la consola.
- [x] **AC-3** — Click en ⏯ pausa si suena y reanuda si está en pausa; el
      ícono muestra la acción disponible (⏸ sonando, ▶ en pausa).
- [x] **AC-4** — Click en 🔀 alterna shuffle como `shuffle`; se ve
      encendido (color de acento) o apagado.
- [x] **AC-5** — Click en ♥ apagado agrega el tema a Tus me gusta; en ♥
      lleno lo quita. El ícono cambia al confirmarse y la consola lo informa
      como `like` / `unlike` (spec 011). Si el pedido falla, el ícono vuelve
      a su estado anterior y se ve el error.
- [x] **AC-6** — Sin nada sonando, los botones se ven atenuados y el click
      no hace nada ni escribe en la consola.
- [x] **AC-7** — Después de clickear cualquier botón, lo que se escribe va
      a la línea de entrada sin tener que clickearla.
- [ ] **AC-8** — Al pasar el mouse, cada botón se resalta y muestra un
      tooltip con su acción y su atajo de ventana, si tiene.
- [x] **AC-9** — Con letra máxima y con la ventana en su tamaño mínimo, los
      botones siguen visibles y clickeables (el título se corta con "…").
- [ ] **AC-10** — Con música sonando y el mouse quieto, el consumo de CPU
      no sube respecto de 0.3.0 (los botones no agregan redibujos
      continuos).
- [x] **AC-11** — El ícono de estado al lado del nombre del tema (▶ sonando,
      ⏸ en pausa) tiene el mismo color en los dos estados.

## Plan técnico

- Archivos que toca: `src/app/engine.rs`, `src/desktop/app.rs`,
  `src/config.rs`, `docs/arquitectura.md`, `CHANGELOG.md`,
  `Cargo.toml`/`Cargo.lock`.
- Funciones/estructuras nuevas:
  - `engine::Input::Shuffle` y `Input::ToggleLike` (♥ lleno → `unlike`,
    si no → `like`, mismo camino y mensajes que los comandos del spec
    011). ⏮ ⏯ ⏭ usan los `Input::Prev/TogglePause/Next` que ya existen.
  - `desktop::app`: `now_bar` pasa a `&mut self` y dibuja los botones con
    `ui.interact(rect, id, Sense::click())`, como `title_button`. Una
    función `player_button` (ícono, activo/atenuado, resaltado al pasar
    el mouse, tooltip con la acción y el atajo de ventana vigente).
    Layout: primero se ubican los botones (centro y ♥), después el
    título/artistas se cortan en el espacio que queda.
  - Sin nada sonando los botones se dibujan atenuados y no se mandan
    inputs.
  - El foco: la línea de entrada ya pide el foco en cada cuadro
    (`request_focus`), así que el click no se lo saca (AC-7); se verifica
    con un test.
  - `config::layout`: tamaño de botón y separación (escalan con la letra).
  - `desktop::layout::player_row` (pura, sin egui): dónde va cada botón y
    qué chips entran; sin lugar se ocultan posición, cola y volumen, en
    ese orden, antes que cualquier botón.
  - Íconos dibujados con formas (⏮ ⏯ ⏭ ♥), sin depender de la fuente;
    🔀 sigue siendo texto.
- Tests necesarios:
  - `engine`: `Input::ToggleLike` alterna según el ♥; `Input::Shuffle`
    igual que `shuffle`; sin nada sonando no escribe nada.
  - `desktop` (egui sin ventana, como los tests actuales de `app.rs`):
    click en cada botón manda el `Input` correcto; sin nada sonando no
    manda nada; tras el click, la entrada sigue con foco.
  - AC-1, AC-8, AC-9, AC-10: prueba manual (mirar la barra en tamaño
    mínimo y letra máxima; CPU con el mouse quieto contra 0.3.0).
- Contratos a crear/actualizar: doc de `Input::Shuffle`/`ToggleLike`;
  `docs/arquitectura.md` si cambia qué manda la ventana al motor.
- Versión que publica: `0.4.0` (spec nuevo)

## Tareas

- [x] T1 — `engine`: `Input::Shuffle` e `Input::ToggleLike` + tests.
- [x] T2 — `config`: medidas de los botones.
- [x] T3 — `desktop`: botones en `now_bar`, layout y tooltips.
- [x] T4 — Tests de la ventana (click → input, deshabilitados, foco).
- [x] T5 — Docs, changelog 0.4.0, versión; fmt/clippy/test.
- [ ] T6 — Prueba manual (AC-1, AC-8, AC-9, AC-10).

## Definition of Done

- [ ] Todos los AC tildados, o el estado es `Reabierto (parcial)` con el
      motivo explícito
- [ ] Tests corren y pasan
- [ ] Contratos de funciones públicas y `docs/arquitectura.md` actualizados
      si el spec cambió una firma, comportamiento o el mapa de módulos
- [ ] Decisiones de diseño relevantes documentadas en `docs/decisiones.md`
- [ ] Changelog actualizado: sección `## [0.4.0] - fecha`
- [ ] Versión subida en `Cargo.toml` y `Cargo.lock`; el Release lo publica
      CI al mergear
- [ ] Sin constantes/umbrales hardcodeados fuera de `src/config.rs`
- [ ] Sin secretos ni credenciales en el diff
- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test` pasan

## Notas de verificación

- Tests: `engine` (`Input::ToggleLike` alterna, `Input::Shuffle` como el
  comando: AC-4, AC-5), ventana sin ventana real con clicks de egui (cada
  botón manda su `Input`, sin nada sonando no manda nada, la entrada
  conserva el foco: AC-2, AC-3, AC-6, AC-7), letra máxima en la ventana
  mínima sin superposiciones (AC-9), `player_row`.
- AC-5 "si falla vuelve a su estado anterior": el ♥ solo cambia cuando
  Spotify confirma, así que si falla nunca cambió (test de spec 011).
- Captura de la app en release sin nada sonando: ⏮ ▶ ⏭, ♥ y 🔀 atenuados
  y alineados.
- AC-11: test `el_icono_de_estado_no_cambia_de_color_en_pausa`.
- Prueba manual 1: los íconos no quedaban a la misma altura (capturas en
  `screens/`). egui centra el texto por la caja del renglón, así que las
  letras se veían más arriba que los botones dibujados, y ⏸ / 🔀 (glyphs
  de otra fuente) cada uno a su altura. Corregido: el texto se centra por
  sus mayúsculas (`caps_shift`), los glyphs por su dibujo (`paint_glyph`)
  y el ♥ por su forma. Medido en una captura: todos los elementos de la
  fila con el centro a la misma altura (±1 px).
- Pendiente, prueba manual con algo sonando: AC-1, AC-8 (resaltado y
  tooltip), AC-10 (CPU con el mouse quieto contra 0.3.0).
