# 006 - Atajos de teclado globales

## Estado
En plan

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

  | Combinación | Acción | Igual que |
  |---|---|---|
  | `Ctrl+Alt+Espacio` | pausa / reanudar | `pause` |
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
   `Ctrl+Alt+Espacio` pausa; otra vez, reanuda. `Ctrl+Alt+→` pasa al
   siguiente, `Ctrl+Alt+Enter` corta todo.
2. Si una combinación no se puede usar, la app lo dice al abrir y sigue
   andando con las demás.
3. Al cerrar la app, las combinaciones quedan libres para otras apps.

## Criterios de aceptación

- [ ] **AC-1** — Con la ventana minimizada y música sonando,
      `Ctrl+Alt+Espacio` pausa y otra vez reanuda, sin cortes ni demora
      perceptible (< ~0,5 s).
- [ ] **AC-2** — `Ctrl+Alt+→` / `Ctrl+Alt+←` pasan al siguiente / anterior
      (regla de 3 s de spec 003) con la ventana minimizada o con otra
      app enfocada.
- [ ] **AC-3** — `Ctrl+Alt+Enter` corta la reproducción y vacía la cola,
      igual que `stop`; la barra de "sonando ahora" queda vacía.
- [ ] **AC-4** — Con la ventana enfocada, cada combinación hace su acción
      una sola vez, y los atajos de spec 004 (`Ctrl+Espacio`, `Ctrl+→`,
      `Ctrl+←`) siguen andando.
- [ ] **AC-5** — `Ctrl+Alt+↑` / `Ctrl+Alt+↓` suben / bajan un paso de
      volumen (requiere spec 005).
- [ ] **AC-6** — Si una combinación ya está tomada (probado abriendo una
      segunda ventana), la consola avisa cuál y la app sigue con las
      demás; ninguna falla la cierra.
- [ ] **AC-7** — Al cerrar la app (`exit`, botón o cerrar desde la barra
      de tareas), las combinaciones quedan libres (una app nueva puede
      registrarlas).
- [ ] **AC-8** — Sin nada sonando, las combinaciones no hacen nada y no
      muestran errores.
- [ ] **AC-9** — `help` lista los atajos globales activos.
- [ ] **AC-10** — Con la app en reposo, la CPU sigue en ~0 % (sin
      polling) y la RAM reproduciendo, dentro del tope de spec 004
      (≤ 100 MB). Medido con `scripts/medir-consumo.ps1`.
- [ ] **AC-11** — La CLI (`spotify-terminal.exe`) no cambia de
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
*(lo completa el agente antes de implementar, se revisa antes de seguir)*

## Tareas
*(desglose del plan, se van tildando)*

## Definition of Done

- [ ] Todos los AC tildados, o el estado es `Reabierto (parcial)` con el
      motivo explícito
- [ ] Tests corren y pasan
- [ ] Contratos de funciones públicas y doc de arquitectura actualizados si
      el spec cambió una firma, comportamiento o el mapa de módulos
- [ ] Decisiones de diseño relevantes documentadas
- [ ] Changelog actualizado
- [ ] Sin constantes/umbrales hardcodeados fuera de su lugar de config
- [ ] Sin secretos ni credenciales en el diff
- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test` pasan

## Notas de verificación
