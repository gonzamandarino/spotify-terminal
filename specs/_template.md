# NNN - Nombre de la feature

## Estado
Uno de: `Draft` | `En plan` | `En implementación` | `En verificación` |
`Verificado` | `Reabierto (parcial)`

Reglas:
- `Verificado` solo si **todos** los criterios de aceptación están tildados.
- Si al cerrar queda algún criterio sin cumplir (bloqueado, no verificable en
  esta fase, etc.), el estado es `Reabierto (parcial)`, no `Verificado`. Se
  anota en "Notas de verificación" qué falta y, si el trabajo sigue en un
  spec nuevo, cuál (`ver spec NNN`).
- `Reabierto (parcial)` no bloquea mergear si vos lo acepta así
  a propósito — pero queda explícito en el spec, no implícito.

## Contexto
Por qué hace falta esta feature. Qué problema resuelve o qué comportamiento
actual cambia.

## Preguntas / Supuestos
*(antes de escribir el Plan técnico: ambigüedades del pedido original y cómo
se resolvieron — respondidas por vos, o asumidas explícitamente
con el motivo. No se avanza al Plan con una ambigüedad relevante sin resolver
de alguna de las dos formas — cuanto más caro es deshacer una decisión
equivocada, más vale resolver la ambigüedad acá en vez de en el código.)*

- Pregunta / ambigüedad 1 → respuesta de vos, o "Asumido: ... (motivo)"

## Qué debe pasar (no cómo)
Descripción del comportamiento esperado, desde afuera del código/sistema.

## Criterios de aceptación
Cada uno con ID (`AC-1`, `AC-2`, ...) para poder referenciarlo desde tests,
commits y "Notas de verificación" sin ambigüedad.

- [ ] **AC-1** — Criterio verificable 1
- [ ] **AC-2** — Criterio verificable 2
- [ ] **AC-3** — Caso límite cubierto

## Riesgos / casos de falla
*(obligatoria si el spec toca algo físico, irreversible o con efecto fuera
del propio sistema: hardware, dinero, envío de datos a terceros, borrado de
datos, notificaciones a personas reales. Se borra esta sección si no aplica.)*

- ¿Qué pasa si el proceso se interrumpe a mitad de la operación (crash,
  reinicio, timeout)? ¿Queda en un estado seguro?
- ¿Hay un tope/limite duro independiente de la lógica, para que un bug no
  deje algo en un estado peligroso o costoso sostenido?
- ¿Qué pasa si una dependencia externa (sensor, API, red) falla de forma
  sostenida, no solo puntual?

## Plan técnico
*(lo completa el agente antes de implementar, se revisa antes de seguir)*

- Archivos que toca:
- Funciones/estructuras nuevas:
- Tests necesarios:
- Contratos a crear/actualizar (pre/postcondiciones de funciones públicas) y
  si hace falta tocar el doc de arquitectura del proyecto:

## Tareas
*(desglose del plan, se van tildando)*

- [ ] Tarea 1
- [ ] Tarea 2

## Definition of Done
*(checklist fijo, además de los criterios de aceptación de arriba — ajustar
las rutas de docs si el proyecto las nombra distinto)*

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
*(al cerrar: qué se probó y resultado — por AC cuando no sea obvio)*
