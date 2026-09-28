# 014 - Panel de consumo

## Estado
En verificación

## Contexto
Bajo consumo es un requisito del proyecto (CLAUDE.md), pero hoy la única
forma de ver cuánto gasta la app es el Administrador de tareas o medir
por PID a mano (como en los AC de consumo de specs 004 y 010). Se pide un
panel en la app de escritorio que muestre el consumo de la propia app,
como una opción más de Personalización. El panel va arriba de la
visualización (spec 010), y cuando se ven los dos la visualización se
achica para dejarle lugar.

El panel es personalizable: qué valores se ven (CPU, RAM o ambos), un
gráfico (de CPU, de RAM o uno para cada uno) y un máximo de uso por
recurso que marca cuándo la app se pasa.

## Preguntas / Supuestos

- **¿Qué muestra?** → Respondido por vos: CPU y RAM del proceso, en
  texto, y se elige cuáles: **Ambos**, **Solo CPU** o **Solo RAM**.
- **¿Gráfico?** → Respondido por vos: sí, elegible entre **CPU**,
  **RAM** o **uno para cada uno**; de línea, con los últimos 60 s (un
  punto por segundo, ventana en config). Asumido: también **Ninguno**
  (de fábrica, el panel más bajo y liviano), y el gráfico se elige
  aparte de los valores en texto (se puede ver RAM en texto y el gráfico
  de CPU). Motivo: son dos opciones distintas del pedido y atarlas quita
  combinaciones sin ahorrar nada.
- **¿Qué es el "máximo de uso de recursos"?** → Respondido por vos: un
  **umbral de alerta**, uno de CPU (%) y uno de RAM (MB). Si el valor lo
  pasa, se pinta en el color de error del tema; el gráfico dibuja el
  umbral como línea horizontal y el tramo que lo pasa en ese color. La
  app no cambia su comportamiento por pasarlo.
- **¿Umbrales de fábrica y rango?** → Asumido: CPU 1 % de la PC, RAM
  100 MB (tope de spec 004). Rango: CPU 0,1–100 %, RAM 20–2000 MB
  (config). *(Primero era CPU 10 % en 1–400 %, en % de un núcleo; se
  cambió con las unidades, ver abajo.)* Se eligen con un slider en el menú (como el tamaño
  de letra, spec 009).
- **¿Escala del gráfico?** → Asumido: de 0 al mayor entre el umbral y
  el máximo de la ventana visible, más un 10 %: el umbral siempre se ve y
  un pico no se sale del gráfico.
- **¿Y con la visualización en Ninguna?** → Respondido por vos: la
  columna derecha aparece igual, solo con el panel de consumo arriba.
- **¿Dónde se configura?** → Asumido: un submenú nuevo **Consumo ▸** en
  Personalización, después de Visualización, con: casilla **Mostrar
  consumo**; **Valores** (Ambos / Solo CPU / Solo RAM); **Gráfico**
  (Ninguno / CPU / RAM / Uno para cada uno); sliders **Máximo de CPU** y
  **Máximo de RAM**; **Restaurar consumo**. Motivo: con cinco opciones
  propias ya no entra como una casilla dentro de Visualización.
- **¿Cómo se guarda?** → Asumido: sección propia en `ajustes.json`:
  `"consumo": {"visible": false, "valores": "ambos", "grafico":
  "ninguno", "max_cpu": 1, "max_ram": 100}`. "Restaurar consumo" y
  "Restaurar todo" vuelven a esos valores. Una clave inválida abre con su
  valor de fábrica y avisa la clave (como spec 007 AC-9).
- **¿CPU y RAM en qué unidad?** → Respondido por vos (2026-09-28, al
  probarlo): **las del Administrador de tareas**. CPU en % de toda la PC
  (dividido por los núcleos lógicos) y RAM en working set **privado**.
  Primero se había asumido % de un núcleo y working set (las unidades de
  los topes de los specs 004 y 010) y no coincidían con el Administrador:
  un pico de 40 % de un núcleo son 2,5 % en una PC de 16, y el working
  set suma las páginas compartidas (~30 MB contra ~16 MB privados).
- **¿Cada cuánto se actualiza?** → Asumido: cada 1 s (config), y el CPU
  es el promedio de ese segundo. Medir es barato (dos llamadas al
  sistema); lo que cuesta es redibujar la ventana, así que se redibuja
  para esto una vez por segundo, solo con el panel visible y la ventana
  no minimizada. Con el panel apagado no se mide nada ni se guarda
  historia (reposo en ~0 % como hoy).
- **¿Qué pasa con la historia al minimizar o apagar?** → Asumido: se
  borra. Al volver, el gráfico arranca de cero. Motivo: minimizada no se
  mide; unir los extremos inventaría una línea que no pasó.
- **¿Qué se ve antes de la primera medición?** → Asumido: `CPU —` hasta
  tener dos mediciones (el % sale de la diferencia entre ambas); la RAM
  sale desde la primera.
- **¿Qué tamaño tiene?** → Asumido: el ancho es el de la columna
  derecha (el mismo `visualizacion.ancho` arrastrable del spec 010, con
  las mismas reglas de ventana angosta). El alto no se arrastra: un
  renglón por valor en texto (con la letra del contenido, spec 009) más
  `GRAPH_HEIGHT` (config) por gráfico. La visualización ocupa el resto.
- **¿Y si no queda alto para la visualización?** → Asumido: si abajo del
  panel de consumo quedan menos de `MIN_VIZ_HEIGHT` puntos (config), la
  visualización no se dibuja ni se anima (no gasta CPU) y el panel de
  consumo queda solo. Vuelve al agrandar la ventana.
- **¿Otras plataformas?** → Asumido: la medición es de Windows (la app
  de escritorio ya usa `windows-sys`). En otra plataforma el panel
  muestra `—` y los gráficos quedan vacíos.
- **¿La CLI?** → Asumido: no cambia.
- **¿Versión?** → Spec nuevo → minor: `0.6.0`.

## Qué debe pasar (no cómo)
1. En Personalización → Consumo se prende o apaga el panel, se elige qué
   valores y qué gráficos se ven y se fija el máximo de CPU y de RAM; se
   aplica al instante y queda guardado.
2. Prendido, arriba en la columna derecha se ven los valores elegidos
   del consumo de la app y sus gráficos de los últimos 60 s,
   actualizados cada segundo.
3. Si un valor pasa su máximo, se ve en el color de error, en el texto y
   en el gráfico.
4. Si además hay una visualización, esta queda abajo del panel de
   consumo, más baja, en vez de ocupar todo el alto.
5. Apagado, la app se ve y consume igual que antes de este spec.

## Criterios de aceptación

- [x] **AC-1** — Personalización tiene el submenú **Consumo**, después
      de Visualización, con: casilla Mostrar consumo; Valores (Ambos /
      Solo CPU / Solo RAM); Gráfico (Ninguno / CPU / RAM / Uno para cada
      uno); sliders Máximo de CPU y Máximo de RAM dentro del rango de
      config; Restaurar consumo. Lo elegido se marca, se aplica en el
      mismo frame y persiste al reabrir. Restaurar consumo y Restaurar
      todo vuelven a los valores de fábrica (apagado, Ambos, Ninguno,
      1 %, 100 MB).
- [x] **AC-2** — Con consumo prendido y una visualización, la columna
      derecha tiene el panel de consumo arriba y la visualización abajo,
      con el alto que sobra; los dos comparten el ancho y el borde
      arrastrable del spec 010. El alto del panel es un renglón por valor
      elegido más `GRAPH_HEIGHT` por gráfico elegido.
- [x] **AC-3** — Con consumo prendido y visualización en Ninguna, la
      columna derecha existe solo con el panel de consumo arriba, del
      ancho guardado. Con los dos apagados no hay columna (como hoy).
- [x] **AC-4** — Si abajo del panel de consumo quedan menos de
      `MIN_VIZ_HEIGHT` puntos, la visualización no se dibuja ni pide
      cuadros; al agrandar la ventana vuelve. Con la ventana angosta,
      la columna entera sigue las reglas de ancho del spec 010 AC-2.
- [x] **AC-5** — Los valores son `CPU X,Y %` (de toda la PC, promedio
      del último período) y `RAM N MB` (working set privado), según Valores,
      actualizados cada `usage::PERIOD` (1 s). Antes de la segunda
      medición, `CPU —`. El % a partir de dos mediciones es correcto
      (test con tiempos sintéticos, incluido 0, más de 100 % e intervalo
      cero).
- [x] **AC-6** — Gráficos: uno de línea por recurso elegido en Gráfico,
      con un punto por medición y como mucho `usage::HISTORY` (60 s) de
      historia; el punto más nuevo a la derecha. La escala va de 0 al
      mayor entre el umbral y el máximo visible, +10 %. La historia se
      borra al apagar el panel o minimizar la ventana (tests de la
      historia y la escala).
- [x] **AC-7** — Umbral: un valor mayor a su máximo se pinta en el color
      de error del tema, en el texto y en el tramo del gráfico que lo
      pasa; el gráfico dibuja el máximo como línea horizontal. Igual o
      menor, colores normales (test del corte; captura con un máximo bajo).
- [ ] **AC-8** — Los valores coinciden con los medidos por PID desde
      afuera (Administrador de tareas / script de medición) con margen de
      ±1 punto de CPU y ±5 MB de RAM, en reposo y reproduciendo con
      Barras. Mismas unidades que el Administrador de tareas.
- [ ] **AC-9** — Consumo del propio panel: prendido con los dos valores
      y los dos gráficos, en reposo (sin música, visualización Ninguna),
      la app queda ≤ 1 % de un núcleo y la RAM no crece más de 1 MB
      respecto de apagado. Minimizada no se mide ni redibuja. Apagado,
      reposo en ~0 % como spec 007 AC-14.
- [x] **AC-10** — `ajustes.json` con una clave de `consumo` inválida
      (tipo equivocado, opción desconocida o máximo fuera de rango) abre
      con el valor de fábrica de esa clave y avisa cuál.
- [x] **AC-11** — La CLI (`spotify-terminal.exe`) no cambia.

## Plan técnico

Sin dependencias nuevas: se suma la feature
`Win32_System_ProcessStatus` a `windows-sys` (ya está) para
`K32GetProcessMemoryInfo`; `GetProcessTimes` ya viene con
`Win32_System_Threading`. El gráfico se dibuja con el `Painter` de egui
(`line_segment`), sin librerías de gráficos.

### Archivos que toca

| Archivo | Cambio |
|---|---|
| `Cargo.toml`, `Cargo.lock` | Feature `Win32_System_ProcessStatus`. Versión `0.6.0`. |
| `src/config.rs` | Módulo `usage`: `PERIOD = 1 s`, `HISTORY = 60`, `GRAPH_HEIGHT`, `MAX_CPU = 1.0` y rango `MAX_CPU_MIN..MAX_CPU_MAX` (0,1–100), `MAX_RAM_MB = 100` y rango (20–2000), `SCALE_HEADROOM = 1.1`. En `viz`: `MIN_VIZ_HEIGHT`. |
| `src/desktop/usage.rs` *(nuevo)* | `Sample { cpu: Duration, at: Instant, ram: u64 }`; `sample() -> Option<Sample>` (Windows: `GetProcessTimes` + `K32GetProcessMemoryInfo` del propio proceso; otra plataforma: `None`); `cpu_percent(prev, next) -> f32` puro; `Usage`: última medición, historia circular de `HISTORY` puntos por recurso, `due(now)`, `clear()`; `scale(history, max) -> f32` puro. Dibujo: `show(ui, rect, &UsageSettings, &Palette)` con texto y gráficos, y `height(&UsageSettings, row_height) -> f32`. |
| `src/desktop/settings.rs` | `UsageSettings { visible, values: UsageValues, graph: UsageGraph, max_cpu, max_ram_mb }`, `Section::Usage` (`"consumo"`), validación por clave y restaurar. |
| `src/desktop/menu.rs` | `SUBMENUS` suma "Consumo"; `usage_menu` con casilla, dos grupos de opciones, dos sliders y Restaurar consumo. |
| `src/desktop/viz/mod.rs` | `panel_width` recibe si hay consumo: la columna existe con modo ≠ Ninguna **o** consumo prendido. |
| `src/desktop/app.rs` | Columna derecha si hay modo o consumo: arriba el panel de consumo (`usage::height`), abajo la visualización con el resto si llega a `MIN_VIZ_HEIGHT` (`viz_height`, puro). Mide con `Usage::due` y pide `request_repaint_after` al próximo período solo con consumo prendido y sin minimizar; `clear()` al apagar o minimizar. |
| `docs/arquitectura.md`, `docs/decisiones.md`, `docs/glosario.md`, `README.md`, `CHANGELOG.md` | Ver "Contratos y docs". |

### Tests

- `usage`: `cpu_percent` (0 %, 50 %, 250 % con varios núcleos,
  intervalo cero → 0); `—` hasta la segunda medición; la historia guarda
  como mucho `HISTORY` y descarta la más vieja; `clear` la vacía;
  `scale` con valores bajo y sobre el umbral; corte del umbral (igual =
  normal, mayor = error); `height` según valores y gráficos; en Windows,
  `sample()` devuelve RAM > 0 (AC-5, AC-6, AC-7).
- `settings`: cada clave de `consumo` válida, inválida → fábrica +
  aviso, máximos fuera de rango, restaurar (AC-1, AC-10).
- `menu`: "Consumo" en `SUBMENUS` después de "Visualización".
- `viz` / `app`: `panel_width` con Ninguna y consumo → columna; los dos
  apagados → `None`; `viz_height` y el corte en `MIN_VIZ_HEIGHT` (AC-2,
  AC-3, AC-4).
- Manual: capturas de AC-1–AC-4 y AC-7; AC-8 y AC-9 medidos por PID en
  release.

### Contratos y docs
- Contratos nuevos: `usage::sample`, `usage::cpu_percent`,
  `usage::scale`, `usage::height`, `usage::show`, `Usage`,
  `UsageSettings`. Actualizar: `Visualizer::panel_width`, `Settings`,
  `Section`.
- `docs/arquitectura.md`: módulo `desktop::usage` y la columna derecha
  (consumo + visualización).
- `docs/decisiones.md`: unidades del Administrador de tareas, 1 s y solo
  con el panel visible, historia que se borra al minimizar, umbral como
  alerta (no como tope que la app aplica), gráfico con `Painter` sin
  librería.
- `docs/glosario.md`: "panel de consumo", "máximo de consumo".
- `README.md`: el submenú Consumo.

### Versión
`0.6.0` (minor: spec nuevo).

### Cambios respecto del plan (al implementar)

- La ventana minimizada no llama a `ui()` (mide 0×0), así que no se
  detecta el minimizado: `Usage::record` borra la historia si la
  medición llega más de `config::usage::GAP_RESET` (3 s) después de la
  anterior. Mismo efecto que AC-6 pide.
- `Usage::tick` mide si pasó `PERIOD` menos `EARLY` (50 ms, config): el
  redibujo pedido a 1 s puede llegar unos ms antes y sin esto habría un
  segundo redibujo solo para esperar lo que falta.
- `Usage::clear` / `reset` vacían en el lugar (sin reservar memoria): con
  el panel oculto se llaman en cada cuadro.
- El corte de alto quedó en `viz::below(area, used)` (puro, testeado) en
  vez de una `viz_height` en `app`. `usage::over` es el corte del máximo.
- La línea que separa consumo y visualización solo se dibuja si abajo hay
  visualización; el rótulo del gráfico lleva fondo propio para no pisarse
  con la línea punteada del máximo (visto en las capturas).
- **Unidades (2026-09-28, pedido tuyo al probarlo):** `cpu_percent`
  recibe los núcleos lógicos (`available_parallelism`, una vez en
  `Usage::new`) y divide por ellos; `sample` lee
  `PROCESS_MEMORY_COUNTERS_EX2.PrivateWorkingSetSize` con una estructura
  propia (`windows-sys` 0.52 no la trae). `MAX_CPU` pasa a 1 % en
  0,1–100 %.

## Tareas

- [x] **T1** — `usage`: medición en Windows, `cpu_percent`, `Usage` con
      historia, `scale`. Tests (AC-5, AC-6).
- [x] **T2** — `UsageSettings`, sección `consumo` en `ajustes.json`,
      validación y restaurar. Tests (AC-1, AC-10).
- [x] **T3** — Submenú Consumo (AC-1).
- [x] **T4** — Dibujo del panel: valores, gráficos, umbral en color de
      error. Tests del corte y la altura (AC-5–AC-7).
- [x] **T5** — Columna derecha con consumo arriba y visualización
      achicada; `panel_width` y corte por `MIN_VIZ_HEIGHT`. Tests (AC-2–
      AC-4).
- [x] **T6** — Repintado cada período solo con el panel visible y sin
      minimizar; historia borrada al apagar/minimizar (AC-6, AC-9).
- [ ] **T7** — Verificación: capturas, comparación por PID y consumo en
      reposo en release (AC-8, AC-9, AC-11).
- [x] **T8** — Contratos, arquitectura, decisiones, glosario, README,
      changelog y versión `0.6.0`.

## Definition of Done

- [ ] Todos los AC tildados, o el estado es `Reabierto (parcial)` con el
      motivo explícito
- [x] Tests corren y pasan
- [x] Contratos de funciones públicas y doc de arquitectura actualizados si
      el spec cambió una firma, comportamiento o el mapa de módulos
- [x] Decisiones de diseño relevantes documentadas
- [x] Changelog actualizado: sección `## [0.6.0] - fecha` de la versión del
      spec
- [x] Versión subida en `Cargo.toml` y `Cargo.lock` (si cambia lo que se
      distribuye); el Release lo publica CI al mergear
- [x] Sin constantes/umbrales hardcodeados fuera de su lugar de config
- [x] Sin secretos ni credenciales en el diff
- [x] `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test` pasan

## Notas de verificación

2026-09-28. 244 tests (`cargo test`), `cargo fmt --check` y `cargo clippy
--all-targets -- -D warnings` limpios. Capturas y mediciones con
`spotify-desktop.exe` en release, 920×580, sin música, con un
`ajustes.json` de prueba (el del usuario se guardó antes y se restauró
después). Las capturas se sacaron con `PrintWindow`; el menú se manejó
mandando teclas a la ventana.

- **AC-1:** captura de Personalización → Consumo abierto con `Alt+P`,
  ↓×6, →: casilla, Valores, Gráfico, los dos sliders (entonces 10 % y
  100 MB de fábrica; hoy 1 % y 100 MB) y Restaurar consumo. Con la casilla marcada con `Espacio`, el
  panel apareció enseguida y `ajustes.json` quedó con
  `"consumo": {"visible": true}`. Restaurar:
  `settings::tests::consumo_valido_o_de_fabrica`; "Restaurar todo" vuelve
  a `Settings::default()`. Orden: `menu::tests::personalizacion_junta_los_menus_de_antes`.
- **AC-2:** captura con Barras + consumo (dos valores, dos gráficos): el
  panel arriba y las barras debajo, más bajas, del mismo ancho.
- **AC-3:** captura con Ninguna + consumo (Solo CPU, gráfico de RAM): la
  columna aparece solo con el panel. Los dos apagados:
  `viz::tests::el_panel_solo_con_modo_y_lugar` (`None`);
  `la_columna_tambien_con_consumo_solo`.
- **AC-4:** captura en 920×400 con Barras + dos gráficos: el panel se ve y
  las barras no se dibujan (quedaban menos de `MIN_VIZ_HEIGHT`).
  `viz::tests::la_visualizacion_se_achica_bajo_el_consumo`; ancho, mismas
  reglas: `la_columna_tambien_con_consumo_solo`.
- **AC-5:** `usage::tests::cpu_en_porcentaje_de_un_nucleo` (0, 50, 250 %,
  intervalo cero), `cpu_desde_la_segunda_medicion`,
  `mide_una_vez_por_periodo`, `mide_el_proceso_en_windows`. Formato
  `CPU 1,6 %` / `RAM 28 MB` en las capturas.
- **AC-6:** `la_historia_guarda_las_ultimas`, `un_hueco_largo_borra_la_historia`,
  `escala_con_el_maximo_siempre_visible`; en las capturas el gráfico crece
  desde la derecha.
- **AC-7:** con `max_ram` 20 MB, `RAM 28 MB` y su línea en rojo (captura);
  en la captura del menú, el pico de CPU al abrirlo pasó el 10 % y ese
  tramo se ve rojo. `pasar_el_maximo_es_mayor_estricto`.
- **AC-8 (parcial):** con las unidades del Administrador (release, en
  reposo, tres capturas cada ~2,5 s): el panel mostró `CPU 0,0 %` /
  `RAM 15 MB`; afuera, CPU 0,00 % de la PC y working set privado
  (`Win32_PerfRawData_PerfProc_Process.WorkingSetPrivate`, el del
  Administrador) 15,0–15,1 MB. Antes del cambio, tu app abierta mostraba
  ~30 MB con 16,1 MB en el Administrador, y picos de 17,7 % de un núcleo
  = 1,1 % de la PC: la diferencia que viste era de unidades. **Falta** reproduciendo con Barras (hace
  falta música; no se pudo mandar `play` sin que la ventana tuviera foco
  estable).
- **AC-9 (no cumple la RAM):** reposo, visualización Ninguna, por PID.
  Primero en working set, tres corridas de 20–30 s cada una:

  | Panel | CPU (% de un núcleo) | RAM (MB) |
  |---|---|---|
  | Apagado | 0,00 / 0,00 / 0,00 | 27,3 / 27,3 / 27,2 |
  | Prendido, dos valores y dos gráficos | 0,15 / 0,15 / 0,00 | 28,5–28,7 / 28,5 / 28,4–28,6 |
  | Prendido, solo texto | — / 0,08 / 0,31 | — / 27,8–28,0 / 28,0 |

  Rehecho con las unidades nuevas, working set privado, dos corridas de
  20 s: apagado 13,8 / 13,8 MB; dos gráficos 14,9–15,1 / 14,9–15,0 MB;
  solo texto 14,4–14,5 / 14,4 MB. CPU 0,07–0,30 % de un núcleo.

  CPU dentro del tope (≤ 1 %). La RAM sube **+1,1–1,3 MB** con los dos
  gráficos (+0,7 MB solo con texto), estable (no crece con el tiempo),
  por encima del tope de 1 MB que se asumió en este spec. Lo que el panel
  guarda son 2×60 `f32`; la diferencia es de redibujar una vez por
  segundo (buffers de egui y de la superficie que Windows mantiene en el
  working set). Pendiente decisión tuya: aceptar el tope en ~2 MB o
  buscar reducirlo.
- **AC-10:** `settings::tests::consumo_valido_o_de_fabrica` (tipo
  equivocado, opción desconocida, máximos fuera de rango y RAM no entera:
  valor de fábrica + aviso con la clave).
- **AC-11:** el diff no toca `src/main.rs`, `src/ui/` ni `src/app/`.
