# 009 - Menú Personalización y tamaño de letra real

## Estado
En verificación

## Contexto
Dos cambios sobre la barra de menús de spec 007, pedidos al usarla:

1. **Demasiados menús.** La barra tiene siete (Tema, Fuente, Atajos,
   Reproducción, Consola, Ventana, Ajustes). Se pide juntar Tema, Fuente,
   Atajos, Consola y Ventana en uno solo, **Personalización**.
2. **El tamaño de letra rompe la app.** Hoy el tamaño se aplica como zoom
   de toda la ventana (`ctx.set_zoom_factor(font_size / 14)`, decisión de
   spec 007). No agranda la letra: agranda la UI entera. Con 32 pt el zoom
   es ×2,3 y la ventana mínima (520×320) queda en 228×140 puntos: las
   cuatro barras fijas (título 36, menús 26, "sonando" 64, entrada 34 =
   160 pt → 365 px) no entran y la consola desaparece; con el tamaño por
   defecto de la ventana la consola queda en un tercio. Se pide que cambie
   **la letra**, no la UI.

## Preguntas / Supuestos

- **¿Cómo queda "Personalización" por dentro?** → Respondido por vos:
  un menú con cinco submenús — Tema ▸, Fuente ▸, Atajos ▸, Consola ▸,
  Ventana ▸ — cada uno con lo mismo que tiene hoy su menú (no un diálogo
  con pestañas).
- **¿Qué queda en la barra?** → Asumido: **Personalización**,
  **Reproducción** y **Ajustes**, en ese orden. Reproducción y Ajustes no
  se tocan (no estaban en el pedido).
- **¿Letras de `Alt`+letra?** → Asumido: `Alt+P` Personalización,
  `Alt+R` Reproducción, `Alt+A` Ajustes (antes `Alt+J`, porque la A la
  tenía Atajos). `Alt+T`/`F`/`C`/`V`/`J` quedan libres para atajos de
  ventana; `Alt+P` pasa a estar reservada.
- **¿Qué texto cambia con el tamaño de letra?** → Asumido: el del
  contenido — consola, línea de entrada (prompt incluido) y barra
  "sonando" (tema, artistas, tiempos, indicadores), manteniendo sus
  proporciones de hoy (ej. la hora de cada línea sigue 2 pt más chica).
  **No** cambian: la barra de título (♫, "spotify-terminal", botones de
  ventana, igual que la barra de título de Windows) ni los menús y
  diálogos (como `editor.fontSize` de VS Code, que no toca los menús).
  Menús y diálogos fijos: respondido por vos.
- **¿Qué pasa con las barras si la letra no entra?** → Asumido: la
  barra "sonando" y la línea de entrada crecen con la letra (alto =
  el de hoy o el que necesita el texto, el mayor), así nunca cortan el
  texto — ese era el motivo del zoom en spec 007. Con la letra por
  defecto (14 pt) todo queda igual que hoy, al píxel.
- **¿Y si con letra grande y ventana chica no entra nada?** → Asumido:
  la consola conserva al menos 3 renglones visibles; si la ventana es
  más chica que eso, la consola se achica primero pero las barras no se
  superponen ni se salen de la ventana. No se cambia el tamaño mínimo de
  la ventana.
- **¿`ajustes.json` cambia?** → Asumido: no. `fuente.tamaño` sigue igual
  (mismo rango 8–32, mismo default 14); solo cambia cómo se aplica. Un
  archivo de 0.1.0 se lee igual.
- **¿Un atajo de ventana guardado con `Alt+P`?** → Asumido: al cargar,
  queda inválido como cualquier combinación reservada (spec 007, AC-9):
  default para esa acción + aviso en la consola nombrándola.
- **¿Versión?** → Respondido por vos: `0.1.1` (patch: corrige y
  reorganiza lo de spec 007), **sin Release por ahora**: se publica junto
  con una versión más significativa. Como cualquier merge a `main` con
  versión nueva publica solo (`release.yml`), esta rama no se mergea
  hasta entonces.

## Qué debe pasar (no cómo)
1. La barra de menús tiene tres menús: Personalización, Reproducción y
   Ajustes. Todo lo que antes estaba en Tema, Fuente, Atajos, Consola y
   Ventana se encuentra dentro de Personalización, y anda igual.
2. Cambiar el tamaño de letra (menú, `Ctrl++`/`Ctrl+-`/`Ctrl+0`,
   `Ctrl+rueda`) agranda o achica el texto de la consola, la entrada y la
   barra "sonando". Las barras, los menús, los márgenes y la barra de
   título no se agrandan (salvo lo justo para que el texto entre).
3. Con cualquier tamaño de letra del rango y cualquier tamaño de ventana
   permitido, la app se sigue viendo y usando: nada cortado, nada
   superpuesto, la consola visible.

## Criterios de aceptación

- [x] **AC-1** — La barra muestra solo Personalización, Reproducción y
      Ajustes, con su letra subrayada. `Alt+P` / `Alt+R` / `Alt+A` abren
      cada uno; se navega con flechas (incluido entrar y salir de los
      submenús con → / ←), `Esc` y clic afuera cierran.
- [x] **AC-2** — Personalización tiene los submenús Tema, Fuente,
      Atajos, Consola y Ventana con todas las opciones que tenían en spec
      007 (incluidos "Editar colores…", "Editar atajos…" y cada
      "Restaurar …"); cada una sigue cumpliendo su AC de spec 007 (AC-2 a
      AC-5, AC-7, AC-11).
- [x] **AC-3** — `Alt+T`, `Alt+F`, `Alt+C`, `Alt+V` y `Alt+J` ya no abren
      nada y se aceptan como atajo de ventana; `Alt+P` se rechaza como
      reservada. Un `ajustes.json` con un atajo de ventana en `Alt+P` abre
      con el default de esa acción y un aviso que la nombra.
- [x] **AC-4** — El tamaño de letra cambia el tamaño del texto de la
      consola, la línea de entrada y la barra "sonando", al instante y en
      sus proporciones de hoy. La barra de título, la barra de menús, los
      menús desplegados y los diálogos no cambian de tamaño.
- [x] **AC-5** — Con 14 pt (default) la ventana se ve igual que en 0.1.0
      (mismos altos de barras y márgenes).
- [x] **AC-6** — Con 32 pt en la ventana mínima (520×320) y en la de
      por defecto (920×580): ningún texto sale cortado verticalmente en
      las barras, las barras no se superponen, la barra de título y los
      botones de ventana siguen andando y la consola se ve y hace scroll.
      Lo mismo con 8 pt.
- [x] **AC-7** — Cambiar el tamaño de letra no cambia el tamaño ni la
      posición de la ventana, ni lo que se guarda como geometría al
      cerrar.
- [x] **AC-8** — Un `ajustes.json` de 0.1.0 (con `fuente.tamaño`) se lee
      sin avisos y aplica el tamaño con el comportamiento nuevo.
- [ ] **AC-9** — Nada se corta (spec 007 AC-13) y el consumo sigue en el
      tope de spec 007 AC-14 (CPU ~0 % en reposo, ≤ 100 MB reproduciendo;
      `scripts/medir-consumo.ps1`). Cambiar el tamaño de letra 20 veces no
      acumula memoria.
- [x] **AC-10** — La CLI (`spotify-terminal.exe`) no cambia.

## Riesgos / casos de falla
- **Submenús de egui con teclado:** si en egui 0.34 los submenús no se
  navegan con → / ←, se avisa antes de seguir (sin dependencias nuevas);
  la alternativa es el diálogo con pestañas.
- **Atajo guardado en `Alt+P`:** cubierto por AC-3 (no impide abrir).
- **Un tamaño de letra que deja la app inusable:** el rango sigue topado
  en config y `Ctrl+Shift+F12` sigue restaurando todo.

## Plan técnico

Sin dependencias nuevas.

### Archivos que toca

| Archivo | Cambio |
|---|---|
| `src/config.rs` | `MENU_ACCESS_KEYS: [char; 3] = ['P', 'R', 'A']`. Altos base de las barras (hoy `const` sueltas en `app.rs`: `TITLE_HEIGHT`, `MENU_HEIGHT`, `INPUT_HEIGHT`, `NOW_HEIGHT`) pasan a `config::layout` junto con los tamaños de texto fijos de la barra de título y de los menús (`UI_FONT_SIZE`), y `CONSOLE_MIN_ROWS = 3`. |
| `src/desktop/menu.rs` | `TITLES` → Personalización, Reproducción, Ajustes. El menú Personalización llama a `theme_menu`, `font_menu`, `keys_menu`, `console_menu`, `window_menu` dentro de un `ui.menu_button` cada uno (submenú). Foco del primer ítem con `Alt`+letra igual que hoy. |
| `src/desktop/theme.rs` | `Theme::apply` deja de usar `set_zoom_factor` (queda en 1,0). Los `TextStyle` (Body, Button, Small, Monospace) quedan en el tamaño fijo de UI (menús, diálogos). Nuevo `Theme::content_size`/helpers `mono(size)` escalados: el tamaño del contenido sale de `appearance.font_size`. Contrato actualizado. |
| `src/desktop/app.rs` | Consola, entrada y barra "sonando" usan el tamaño de los ajustes en vez de `FONT_SIZE` fijo; sus tamaños relativos de hoy (13, 12, `FONT_SIZE - 2`...) pasan a ser proporciones del tamaño elegido. Alto de "sonando" y entrada = `max(alto base, alto que pide el texto)`; si no entran junto con `CONSOLE_MIN_ROWS`, se recortan a lo que queda sin superponerse. La barra de título y la de menús quedan con alto fijo. Ancho reservado para "⋯ ocupado" en la entrada, medido del texto en vez de 260 fijo. `window_geometry` ya no necesita compensar el zoom. |
| `src/desktop/window.rs` | Quita el comentario/compensación del zoom en `InnerSize` (con zoom 1 los puntos de egui = lógicos de Windows). |
| `src/desktop/settings.rs` | Sin cambios de formato; `check_combo` ya lee `MENU_ACCESS_KEYS`, solo cambian los tests que usaban `Alt+T`/`Alt+J`. |
| `docs/decisiones.md` | Nueva entrada: el tamaño de letra cambia solo el texto del contenido; la de spec 007 ("zoom de toda la ventana") pasa a *Reemplazada (spec 009)*. Otra: menú Personalización con submenús. |
| `docs/arquitectura.md`, `docs/glosario.md` | Menús de la barra; "tamaño de letra" = texto del contenido. |
| `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md` | `0.1.1` y su sección. |

### Funciones / estructuras

- `layout::bars(font_size, available_height) -> Bars` (función pura en
  `app.rs` o un `desktop/layout.rs` chico): altos de "sonando" y entrada
  para ese tamaño de letra y alto disponible. Invariantes: con 14 pt da
  los altos de hoy; nunca menor al alto base ni mayor a lo que deja libre
  el resto; la suma de barras nunca pasa `available_height`. Testeable
  sin ventana.
- `Theme::apply` — contrato nuevo: aplica fuente y colores; el tamaño
  de UI es fijo; no toca el zoom.

### Tests

- `menu`: cada menú con su letra (3); `Alt+P` reservada, `Alt+T`/`Alt+J`
  aceptadas (`settings::check_combo`); cada `Section` que se restauraba
  desde un menú sigue alcanzable (test sobre la tabla de submenús).
- `settings`: `ajustes.json` con atajo de ventana `Alt+P` → default +
  aviso con su nombre (AC-3); archivo de 0.1.0 con `fuente.tamaño` sin
  avisos (AC-8).
- `layout::bars`: 14 pt da los altos de 0.1.0 (AC-5); 32 pt en 320 de
  alto no se superpone y deja la consola ≥ 0 (AC-6); 8 pt no achica bajo
  el alto base.
- Manual (capturas, como en spec 007): AC-1, AC-2, AC-4, AC-6 (8 / 14 /
  32 pt × ventana mínima / por defecto), AC-7, AC-9 con audio real y
  `scripts/medir-consumo.ps1`, AC-10.

### Contratos y docs
- Actualizar: `Theme::apply`, `Menus::bar` (menús que dibuja),
  `config::MENU_ACCESS_KEYS`, `window_geometry`.
- Nuevo: `layout::bars`.
- `docs/decisiones.md`, `docs/arquitectura.md`, `docs/glosario.md`,
  `CHANGELOG.md` como en la tabla.

### Versión
`0.1.1` (patch: corrige y reorganiza lo de spec 007). Sin Release por
ahora (ver Preguntas / Supuestos): la rama no se mergea hasta sumar una
versión más significativa.

### Cambios respecto del plan (al implementar)

- `layout::bars` quedó en su propio módulo, `src/desktop/layout.rs`, con
  `layout::scale`. Los márgenes de arriba y abajo de la consola también
  pasaron a `config::layout` (entran en la cuenta de los renglones
  mínimos).
- egui olvida un submenú abierto a mano si no se dibujó el frame anterior
  (`MenuState`): al abrirlo con → se lo marca como visto
  (`MenuState::mark_shown`). Con ← egui movía el foco a lo que encontrara
  a la izquierda (ej. el menú Ajustes): se cancela ese salto
  (`move_focus(FocusDirection::None)`) y el foco vuelve al botón del
  submenú. Los sliders y el campo del prompt se marcan
  (`menu::keeps_arrows`) para que ← / → sigan siendo de ellos.
- Con 32 pt en la ventana mínima, "Nada sonando · escribí play <nombre>"
  se pisaba con el volumen: se corta con "…" antes del volumen.
- El aviso de "ocupado" en la entrada reservaba 260 pt fijos; ahora se
  mide su texto (que sigue a la letra).

## Tareas

- [x] **T1** — Barra con Personalización / Reproducción / Ajustes y
      submenús; `MENU_ACCESS_KEYS` nuevas; probar navegación con teclado
      de los submenús (si no anda, avisar antes de seguir). Tests (AC-1,
      AC-2, AC-3).
- [x] **T2** — Tamaño de letra sin zoom: `Theme::apply` con tamaño de UI
      fijo, contenido (consola, entrada, "sonando") con el tamaño de los
      ajustes y sus proporciones (AC-4, AC-5, AC-7, AC-8).
- [x] **T3** — `layout::bars`: barras que crecen con la letra sin
      superponerse ni tapar la consola. Tests (AC-6).
- [ ] **T4** — Verificación manual con capturas y audio real, consumo
      con `scripts/medir-consumo.ps1` (AC-6, AC-9, AC-10).
- [x] **T5** — Contratos, decisiones, arquitectura, glosario, changelog y
      versión `0.1.1`.

## Definition of Done

- [ ] Todos los AC tildados, o el estado es `Reabierto (parcial)` con el
      motivo explícito
- [x] Tests corren y pasan
- [x] Contratos de funciones públicas y doc de arquitectura actualizados si
      el spec cambió una firma, comportamiento o el mapa de módulos
- [x] Decisiones de diseño relevantes documentadas
- [x] Changelog actualizado: sección `## [0.1.1] - fecha` de la versión del
      spec
- [x] Versión subida en `Cargo.toml` y `Cargo.lock`; el Release lo publica
      CI al mergear
- [x] Sin constantes/umbrales hardcodeados fuera de su lugar de config
- [x] Sin secretos ni credenciales en el diff
- [x] `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test` pasan

## Notas de verificación

169 tests (`cargo test`), `cargo fmt --check` y `cargo clippy
--all-targets -- -D warnings` limpios. Verificación manual con
`spotify-desktop.exe` (debug para capturas, release para consumo), con
capturas de la ventana y teclas mandadas por SendKeys, sin reproducir
(tu instancia estaba abierta y sonando; una segunda no conecta audio
hasta un `play`) (2026-09-27):

- **AC-1:** la barra muestra Personalización, Reproducción y Ajustes con
  su letra subrayada. `Alt+P` abre Personalización con el foco en Tema;
  → abre el submenú con el foco en su primer ítem; ↓ baja; ← cierra el
  submenú y vuelve a Tema; ↓↓↓ → abre Consola; `Esc` cierra.
- **AC-2:** submenús Tema, Fuente y Consola abiertos con su contenido de
  spec 007 (mismas funciones de menú que antes, sin cambios en lo que
  hacen). En Consola, ← dentro del campo del prompt mueve el cursor y no
  cierra el submenú.
- **AC-3:** `settings::tests::chequeo_de_combinaciones` (`Alt+P`
  reservada; `Alt+T/F/C/V/J` libres) y
  `settings::tests::letras_de_los_menus_de_spec_009` (archivo con
  `Alt+P` → fábrica + aviso con `atajos_ventana.stop`).
- **AC-4:** capturas con 8, 14 y 32 pt: el texto de la consola, la
  entrada y "sonando" cambia; la barra de título, la de menús y el menú
  Fuente desplegado quedan igual.
- **AC-5:** `layout::tests::con_la_letra_por_defecto_quedan_los_altos_de_siempre`
  (64 / 34 en la ventana mínima, por defecto y grande); la captura con
  14 pt es igual a la de 0.1.0.
- **AC-6:** capturas con 32 pt en 920×580 y 520×320, y con 8 pt: nada
  cortado ni superpuesto, la consola visible (3 renglones a 32 pt en la
  mínima). Bug encontrado y corregido: el texto de "Nada sonando" contra
  el volumen. Tests `layout::tests::*`.
- **AC-7:** `Ctrl++` ×4 desde 29 pt: se queda en 32 y la ventana sigue
  en 920×580; `ajustes.json` guardó `tamaño: 32` y la misma geometría.
- **AC-8:** `letras_de_los_menus_de_spec_009` lee `fuente.tamaño` de un
  archivo de 0.1.0 sin avisos por él.
- **AC-9 (pendiente):** consumo en release, 60 s en reposo: CPU 0,02 %,
  35,7 MB. 20 cambios de letra: 35,9 → 36,7 MB (egui guarda los
  glifos de cada tamaño; con el rango 8–32 tiene tope, no confirmado con
  una corrida más larga). **Falta:** reproducir música y cambiar la letra
  y los menús mientras suena (sin cortes), y RAM reproduciendo ≤ 100 MB.
  No se probó para no abrir una segunda sesión de audio con tu instancia
  sonando.
- **AC-10:** no hay cambios en `src/ui`, `src/app`, `src/spotify` ni
  `src/bin`; `spotify-terminal --version` responde 0.1.1.
