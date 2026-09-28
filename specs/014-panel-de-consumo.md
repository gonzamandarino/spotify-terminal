# 014 - Panel de consumo

## Estado
En plan

## Contexto
Bajo consumo es un requisito del proyecto (CLAUDE.md), pero hoy la única
forma de ver cuánto gasta la app es el Administrador de tareas o medir
por PID a mano (como en los AC de consumo de specs 004 y 010). Se pide un
panel en la app de escritorio que muestre el consumo de la propia app,
como una opción más de Personalización. El panel va arriba de la
visualización (spec 010), y cuando se ven los dos la visualización se
achica para dejarle lugar.

## Preguntas / Supuestos

- **¿Qué muestra?** → Respondido por vos: CPU y RAM del proceso.
- **¿Cómo?** → Respondido por vos: solo texto, con los valores actuales
  (sin gráficos).
- **¿Y con la visualización en Ninguna?** → Respondido por vos: la
  columna derecha aparece igual, solo con el panel de consumo arriba.
- **¿CPU en qué unidad?** → Asumido: % de **un núcleo**, igual que en
  los AC de consumo de los specs 004 y 010 (puede pasar de 100 % en una
  PC de varios núcleos). Motivo: así el número del panel se compara
  directo con los topes de los specs ("CPU ≤ 10 % de un núcleo").
- **¿Qué RAM?** → Asumido: el working set del proceso (lo que se midió
  por PID en los specs 004 y 010), en MB.
- **¿Cada cuánto se actualiza?** → Asumido: cada 1 s (config), y el CPU
  es el promedio de ese segundo. Medir es barato (dos llamadas al
  sistema); lo que cuesta es redibujar la ventana, así que se redibuja
  solo para eso una vez por segundo, y solo con el panel visible y la
  ventana no minimizada. Con el panel apagado no se mide nada (reposo en
  ~0 % como hoy).
- **¿Qué se ve antes de la primera medición?** → Asumido: `CPU —` hasta
  tener dos mediciones (el % sale de la diferencia entre ambas); la RAM
  sale desde la primera.
- **¿Dónde se prende?** → Asumido: en el submenú **Visualización** de
  Personalización, abajo de los modos y tras un separador, una casilla
  **Mostrar consumo**. Motivo: comparte la columna derecha y su ancho con
  la visualización; un submenú nuevo con una sola casilla sería más ruido
  en el menú. Apagado por defecto (quien no lo quiere no paga nada).
- **¿Cómo se guarda?** → Asumido: en la sección de visualización de
  `ajustes.json`, `"visualizacion": {"consumo": true}`. "Restaurar
  visualización" y "Restaurar todo" lo apagan. Un valor que no es
  booleano abre apagado y avisa la clave (como spec 007 AC-9).
- **¿Qué tamaño tiene?** → Asumido: el ancho es el de la columna
  derecha (el mismo `visualizacion.ancho` arrastrable del spec 010, con
  las mismas reglas de ventana angosta). El alto es el justo para dos
  renglones (`CPU …` y `RAM …`) con la letra del contenido (spec 009), no
  se arrastra. La visualización ocupa el resto del alto.
- **¿Y si no queda alto para la visualización?** → Asumido: si abajo del
  panel de consumo quedan menos de `MIN_VIZ_HEIGHT` puntos (config), la
  visualización no se dibuja ni se anima (no gasta CPU) y el panel de
  consumo queda solo. Vuelve al agrandar la ventana.
- **¿Otras plataformas?** → Asumido: la medición es de Windows (la app
  de escritorio ya usa `windows-sys`). En otra plataforma el panel
  muestra `—` en los dos valores.
- **¿La CLI?** → Asumido: no cambia.
- **¿Versión?** → Spec nuevo → minor: `0.6.0`.

## Qué debe pasar (no cómo)
1. En Personalización → Visualización se prende o apaga **Mostrar
   consumo**; se aplica al instante y queda guardado.
2. Prendido, arriba en la columna derecha se ve el CPU y la RAM de la
   app, actualizados cada segundo.
3. Si además hay una visualización, esta queda abajo del panel de
   consumo, más baja, en vez de ocupar todo el alto.
4. Apagado, la app se ve y consume igual que antes de este spec.

## Criterios de aceptación

- [ ] **AC-1** — Personalización → Visualización tiene la casilla
      **Mostrar consumo**, tras un separador bajo los modos; se marca al
      prenderla, se aplica en el mismo frame y persiste al reabrir.
      Restaurar visualización y Restaurar todo la apagan.
- [ ] **AC-2** — Con consumo prendido y una visualización, la columna
      derecha tiene el panel de consumo arriba (alto de dos renglones) y
      la visualización abajo, con el alto que sobra; los dos comparten
      el ancho y el borde arrastrable del spec 010.
- [ ] **AC-3** — Con consumo prendido y visualización en Ninguna, la
      columna derecha existe solo con el panel de consumo arriba, del
      ancho guardado. Con los dos apagados no hay columna (como hoy).
- [ ] **AC-4** — Si abajo del panel de consumo quedan menos de
      `MIN_VIZ_HEIGHT` puntos, la visualización no se dibuja ni pide
      cuadros; al agrandar la ventana vuelve. Con la ventana angosta,
      la columna entera sigue las reglas de ancho del spec 010 AC-2.
- [ ] **AC-5** — El panel muestra `CPU X,Y %` (de un núcleo, promedio
      del último período) y `RAM N MB` (working set), actualizados cada
      `usage::PERIOD` (1 s). Antes de la segunda medición, `CPU —`. El
      cálculo del % a partir de dos mediciones es correcto (test con
      tiempos sintéticos, incluido 0 y más de 100 %).
- [ ] **AC-6** — Los valores coinciden con los medidos por PID desde
      afuera (Administrador de tareas / script de medición) con margen
      de ±1 punto de CPU y ±5 MB de RAM, en reposo y reproduciendo con
      Barras.
- [ ] **AC-7** — Consumo del propio panel: con consumo prendido, en
      reposo (sin música, visualización Ninguna) la app queda ≤ 1 % de
      un núcleo y la RAM no crece más de 1 MB respecto de apagado.
      Minimizada no se mide ni redibuja. Apagado, reposo en ~0 % como
      spec 007 AC-14.
- [ ] **AC-8** — `ajustes.json` con `visualizacion.consumo` no booleano
      abre con el panel apagado y avisa la clave.
- [ ] **AC-9** — La CLI (`spotify-terminal.exe`) no cambia.

## Plan técnico

Sin dependencias nuevas: se suma la feature
`Win32_System_ProcessStatus` a `windows-sys` (ya está) para
`K32GetProcessMemoryInfo`; `GetProcessTimes` ya viene con
`Win32_System_Threading`.

### Archivos que toca

| Archivo | Cambio |
|---|---|
| `Cargo.toml`, `Cargo.lock` | Feature `Win32_System_ProcessStatus`. Versión `0.6.0`. |
| `src/config.rs` | Módulo `usage`: `PERIOD = 1 s`. En `viz`: `MIN_VIZ_HEIGHT`. |
| `src/desktop/usage.rs` *(nuevo)* | `Sample { cpu: Duration, at: Instant, ram: u64 }`; `sample() -> Option<Sample>` (Windows: `GetProcessTimes` + `K32GetProcessMemoryInfo` sobre el propio proceso; otra plataforma: `None`); `cpu_percent(prev, next) -> f32` puro; `Usage` que guarda la última y la anterior y dice cuándo toca la próxima. |
| `src/desktop/settings.rs` | `show_usage: bool` (`visualizacion.consumo`), validación, restaurar con la sección Visualización. |
| `src/desktop/menu.rs` | Separador y casilla Mostrar consumo en `viz_menu`. |
| `src/desktop/viz/mod.rs` | `panel_width` recibe si hay consumo: la columna existe con modo ≠ Ninguna **o** consumo prendido. |
| `src/desktop/app.rs` | La columna derecha se arma si hay modo o consumo: arriba el panel de consumo (alto de dos renglones), abajo la visualización con el resto si llega a `MIN_VIZ_HEIGHT`. Mide y pide `request_repaint_after` al próximo período solo con consumo prendido y la ventana no minimizada. |
| `docs/arquitectura.md`, `docs/decisiones.md`, `docs/glosario.md`, `README.md`, `CHANGELOG.md` | Ver "Contratos y docs". |

### Tests

- `usage`: `cpu_percent` con tiempos sintéticos (0 %, 50 %, 250 % con
  varios núcleos, intervalo cero → 0 sin dividir por cero);
  `Usage` muestra `—` hasta la segunda medición; en Windows, `sample()`
  devuelve RAM > 0 (AC-5).
- `settings`: `consumo` válido, inválido → apagado + aviso, restaurar
  (AC-1, AC-8).
- `viz`: `panel_width` con Ninguna y consumo → columna; con los dos
  apagados → `None`; ventana angosta igual que antes (AC-3, AC-4).
- Layout: alto que queda para la visualización y el corte en
  `MIN_VIZ_HEIGHT`, como función pura (AC-2, AC-4).
- `menu`: la casilla cambia `show_usage`.
- Manual: capturas de AC-1–AC-4; AC-6 y AC-7 medidos por PID en release.

### Contratos y docs
- Contratos nuevos: `usage::sample`, `usage::cpu_percent`, `Usage`.
  Actualizar: `Visualizer::panel_width`, `Settings` (`show_usage`).
- `docs/arquitectura.md`: módulo `desktop::usage` y la columna derecha
  (consumo + visualización).
- `docs/decisiones.md`: CPU en % de un núcleo, working set, 1 s y solo
  con el panel visible; casilla dentro de Visualización.
- `docs/glosario.md`: "panel de consumo".
- `README.md`: la casilla en Personalización.

### Versión
`0.6.0` (minor: spec nuevo).

## Tareas

- [ ] **T1** — `usage`: medición en Windows, `cpu_percent`, `Usage`.
      Tests (AC-5).
- [ ] **T2** — Ajuste `visualizacion.consumo` y casilla en el menú
      (AC-1, AC-8).
- [ ] **T3** — Columna derecha con consumo arriba y visualización
      achicada; `panel_width` y corte por `MIN_VIZ_HEIGHT`. Tests (AC-2,
      AC-3, AC-4).
- [ ] **T4** — Repintado cada período solo con el panel visible y sin
      minimizar (AC-7).
- [ ] **T5** — Verificación: capturas, comparación por PID y consumo en
      reposo en release (AC-6, AC-7, AC-9).
- [ ] **T6** — Contratos, arquitectura, decisiones, glosario, README,
      changelog y versión `0.6.0`.

## Definition of Done

- [ ] Todos los AC tildados, o el estado es `Reabierto (parcial)` con el
      motivo explícito
- [ ] Tests corren y pasan
- [ ] Contratos de funciones públicas y doc de arquitectura actualizados si
      el spec cambió una firma, comportamiento o el mapa de módulos
- [ ] Decisiones de diseño relevantes documentadas
- [ ] Changelog actualizado: sección `## [0.6.0] - fecha` de la versión del
      spec
- [ ] Versión subida en `Cargo.toml` y `Cargo.lock` (si cambia lo que se
      distribuye); el Release lo publica CI al mergear
- [ ] Sin constantes/umbrales hardcodeados fuera de su lugar de config
- [ ] Sin secretos ni credenciales en el diff
- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test` pasan

## Notas de verificación
