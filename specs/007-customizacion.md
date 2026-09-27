# 007 - Customización desde una barra de menús

## Estado
En implementación

## Contexto
Hoy todo el look y el manejo de la app de escritorio (spec 004) está fijo
en `src/config.rs`: colores (`config::theme`), fuente (Consolas) y su
tamaño, atajos de la ventana (`Ctrl+Espacio`, `Ctrl+→`...) y atajos
globales (spec 006, `Ctrl+Alt+…`). Para cambiar cualquier cosa hay que
recompilar.

Se pide que **todo sea configurable** (fuentes, tamaño, colores, atajos y
lo que tenga sentido) desde la propia app, con una **barra de menús arriba
estilo Visual Studio Code** ("File  Edit  Selection  View…"), pero con
menús de customización.

## Preguntas / Supuestos

- **¿En qué app?** → Asumido: solo la app de escritorio
  (`spotify-desktop.exe`). En la CLI (`spotify-terminal.exe`) la fuente,
  el tamaño y los colores los pone la terminal (PowerShell / conhost), no
  la app; no hay dónde poner una barra de menús. La CLI no cambia de
  comportamiento.
- **¿Dónde va la barra?** → Asumido: dentro de la barra de título propia,
  a la derecha del ícono ♫, como hace VS Code (una sola franja: ícono,
  menús, título, botones de ventana). Motivo: no le saca otra franja de
  alto a la consola. Si el ancho no alcanza, el título se oculta antes
  que los menús.
- **¿Qué menús?** → Asumido (ajustable al revisar):

  | Menú | Qué tiene |
  |---|---|
  | **Tema** | temas predefinidos (Spotify oscuro — el actual —, Claro, Alto contraste); "Editar colores…" (cada color de `config::theme` con selector de color); "Restaurar colores" |
  | **Fuente** | familia (lista de monoespaciadas instaladas en la PC: Consolas, Cascadia Mono, Courier New, Lucida Console, y la de egui); tamaño (lista + "Agrandar" / "Achicar" / "Tamaño por defecto"); negrita en título y tema que suena sí / no |
  | **Atajos** | tabla con cada acción, su atajo de ventana y su atajo global; "Cambiar…" graba la próxima combinación que se aprieta; "Quitar"; "Restaurar atajos" |
  | **Reproducción** | paso de volumen; calidad de audio (96 / 160 / 320 kbps); umbral de "anterior reinicia el tema" (s) |
  | **Consola** | símbolo del prompt; líneas de scrollback; largo del historial; mostrar hora en cada línea sí / no; limpiar consola |
  | **Ventana** | siempre visible (encima de las demás); recordar tamaño y posición al cerrar; restaurar tamaño |
  | **Ajustes** | abrir el archivo de ajustes; recargar desde el archivo; restaurar **todo** por defecto |

  "Lo que se te ocurra" se acota a esta lista: cualquier constante de
  `config.rs` que el usuario pueda querer tocar sin saber Rust. Quedan
  **fuera**: Client ID, scopes, redirect URI, URLs, timeouts de red y
  límites de la API (cambiarlos rompe la app, no la personaliza).
- **¿Se aplica al instante o al reiniciar?** → Asumido: al instante,
  sin reiniciar, salvo la calidad de audio, que se aplica desde el
  próximo tema (librespot la fija al cargar cada tema). El menú lo aclara.
  **Ajustado en el plan:** librespot fija la calidad al crear el
  reproductor, no por tema, y hoy el motor reusa el mismo reproductor
  entre `play`s. Cambiarla a mitad de una cola obligaría a recrearlo y
  cortaría el tema. Se aplica desde el **próximo `play`** (o `stop` +
  `play`); la cola que ya suena sigue con la calidad anterior.
- **¿Dónde se guarda?** → Asumido: `ajustes.json` en la carpeta de datos
  (`%APPDATA%\spotify-terminal`), junto a los caches de token y
  `volumen.txt`. JSON porque `serde_json` ya está (sin dependencia nueva
  por TOML). Se guarda solo lo que difiere del valor por defecto, así un
  cambio de defaults en una versión nueva le llega al usuario. Se puede
  editar a mano; "Recargar desde el archivo" lo aplica sin reiniciar.
- **¿Qué pasa con un archivo roto o un valor inválido?** → Asumido: nunca
  impide abrir la app. Archivo ilegible → todos los defaults y un aviso en
  la consola; valor inválido o fuera de rango en una clave → default solo
  para esa clave y aviso nombrándola. El archivo no se pisa hasta que el
  usuario cambie algo (para no perder lo que escribió a mano).
- **¿Qué pasa si un atajo nuevo choca?** → Asumido: si ya lo usa otra
  acción de la app, se avisa y se pide confirmar el cambio (la otra queda
  sin atajo). Si es global y Windows no lo deja registrar (spec 006,
  error 1409), se avisa en la consola y se vuelve al atajo anterior.
  Teclas reservadas para editar (`Enter`, `Esc`, `↑`, `↓`, `Tab` sin
  modificadores, letras sin `Ctrl`/`Alt`) no se aceptan como atajo de
  ventana: taparían la escritura en la consola.
- **¿Fuentes que no son monoespaciadas?** → Asumido: no se ofrecen. La
  consola alinea columnas (lista numerada de búsqueda, `help`) y una
  proporcional las desarma. Motivo extra: leer todas las fuentes del
  sistema para saber su nombre cuesta RAM y arranque.
- **¿Un tema que deja el texto ilegible?** → Asumido: se permite (es
  decisión del usuario), pero "Restaurar todo" también está en un atajo
  fijo, no configurable (`Ctrl+Shift+F12`), por si el menú mismo queda
  invisible.
- **¿Comandos de consola para ajustes (ej. `set font 16`)?** → Asumido:
  no en este spec. Todo por menú o archivo. Motivo: acotar; se puede
  pedir aparte.
- **¿Zoom con `Ctrl+rueda` / `Ctrl++` como VS Code?** → Asumido: sí,
  `Ctrl++` / `Ctrl+-` / `Ctrl+0` son los atajos por defecto de
  Agrandar / Achicar / Tamaño por defecto (configurables como el resto).
  `Ctrl+rueda` también cambia el tamaño.

## Qué debe pasar (no cómo)
1. Arriba de la ventana hay una barra de menús (Tema, Fuente, Atajos,
   Reproducción, Consola, Ventana, Ajustes). Clic en uno despliega sus
   opciones; se navega también con teclado (`Alt` + letra subrayada,
   flechas, `Esc` cierra).
2. Cambiar un color, la fuente, su tamaño o un atajo se ve / anda en el
   momento, sin reiniciar y sin cortar la música.
3. Al cerrar y volver a abrir, todo sigue como se dejó.
4. Un archivo de ajustes roto o un valor raro nunca impide abrir la app:
   se usan los defaults donde haga falta y se avisa qué se ignoró.
5. Siempre se puede volver a como vino la app, por menú o por atajo fijo.

## Criterios de aceptación

- [ ] **AC-1** — La barra de menús aparece en la barra de título, con los
      siete menús; cada uno se abre con clic y con `Alt`+letra, se navega
      con flechas y se cierra con `Esc` o clic afuera. Arrastrar la barra
      (fuera de los menús), doble clic para maximizar y los botones de
      ventana siguen andando como en spec 004.
- [ ] **AC-2** — **Tema:** elegir un tema predefinido o cambiar un color
      en "Editar colores…" cambia la ventana en el mismo frame (consola,
      barras, prompt, colores de éxito / aviso / error).
- [ ] **AC-3** — **Fuente:** cambiar familia y tamaño se aplica al
      instante en toda la ventana. Solo se listan fuentes presentes en la
      PC. `Ctrl++`, `Ctrl+-`, `Ctrl+0` y `Ctrl+rueda` cambian el tamaño
      dentro de un rango en config (ej. 8–32 pt), sin pasarse.
- [ ] **AC-4** — **Atajos de ventana:** se puede cambiar o quitar el
      atajo de cada acción; el nuevo anda enseguida y el viejo deja de
      hacer algo. Una combinación reservada para escribir se rechaza con
      aviso; una usada por otra acción pide confirmación.
- [ ] **AC-5** — **Atajos globales:** cambiar uno libera el anterior (otra
      app puede registrarlo) y registra el nuevo sin reiniciar. Si Windows
      lo rechaza (tomado por otra app), se avisa en la consola y queda el
      anterior.
- [ ] **AC-6** — **Reproducción:** el paso de volumen y el umbral de
      "anterior" cambian el comportamiento al instante; la calidad de
      audio se aplica desde el próximo `play`, sin cortar lo que suena.
- [ ] **AC-7** — **Consola y Ventana:** prompt, scrollback, historial,
      hora por línea, siempre visible y recordar tamaño / posición andan
      como dice su menú. Achicar el scrollback descarta las líneas más
      viejas que sobran.
- [ ] **AC-8** — Los ajustes cambiados persisten: cerrar y abrir la app
      los muestra igual. `ajustes.json` guarda solo lo que difiere del
      default y no se escribe a disco en cada paso de un cambio continuo
      (arrastrar el selector de color, `Ctrl+rueda`).
- [ ] **AC-9** — Con `ajustes.json` inválido (JSON roto, clave
      desconocida, color mal escrito, tamaño fuera de rango, atajo
      inexistente) la app abre, usa el default en lo inválido, avisa en la
      consola qué clave se ignoró y no pisa el archivo hasta que el
      usuario cambie algo.
- [ ] **AC-10** — "Recargar desde el archivo" aplica un `ajustes.json`
      editado a mano sin reiniciar.
- [ ] **AC-11** — "Restaurar todo" y el atajo fijo `Ctrl+Shift+F12`
      vuelven todo al default (incluidos los atajos globales) y lo dejan
      guardado. Los "Restaurar …" de cada menú solo afectan a lo suyo.
- [ ] **AC-12** — `help` lista los atajos vigentes (los configurados, no
      los de fábrica).
- [ ] **AC-13** — Nada se corta: cambiar cualquier ajuste mientras suena
      música no produce silencios ni saltos.
- [ ] **AC-14** — Consumo: en reposo con la barra visible, CPU ~0 % (los
      menús no fuerzan redibujos continuos) y RAM reproduciendo dentro del
      tope de spec 004 (≤ 100 MB), medido con
      `scripts/medir-consumo.ps1`. Cambiar de fuente varias veces no
      acumula memoria (se descarta la anterior).
- [ ] **AC-15** — La CLI (`spotify-terminal.exe`) no cambia de
      comportamiento.

## Riesgos / casos de falla
- **Quedarse sin poder usar la app:** un tema ilegible o atajos borrados.
  Mitigación: atajo fijo `Ctrl+Shift+F12` (restaurar todo) que no se
  puede reasignar ni quitar, y borrar `ajustes.json` a mano siempre
  vuelve a fábrica.
- **Interrupción al guardar:** un crash a mitad de escritura podría dejar
  `ajustes.json` cortado. Se escribe a un archivo temporal y se renombra
  (reemplazo atómico); si igual queda roto, aplica AC-9.
- **Atajo global que tapa uno de Windows u otra app:** igual que spec 006.
  Mientras la app está abierta, la combinación deja de llegar a las
  demás; al cerrarla se libera.
- **Fuente que desaparece** (desinstalada entre sesiones): se usa la de
  egui y se avisa, igual que hoy si falta Consolas.

## Plan técnico

Sin dependencias nuevas: egui ya trae menús (`egui::containers::menu`) y
selector de color (`color_edit_button_srgb`); `serde`/`serde_json` ya
están.

### Archivos que toca

| Archivo | Cambio |
|---|---|
| `src/config.rs` | Los valores de hoy pasan a ser **defaults** de los ajustes. Nuevo: rangos (tamaño de letra, paso de volumen, umbral, scrollback, historial), temas predefinidos (`THEME_PRESETS`), catálogo de fuentes (`FONT_CATALOG`: nombre → archivo normal / negrita en `%WINDIR%\Fonts`), atajos de ventana por defecto, teclas reservadas para escribir, atajo fijo de restaurar (`Ctrl+Shift+F12`), `SETTINGS_FILE = "ajustes.json"`, `SETTINGS_SAVE_DELAY`. `theme::*` queda como el preset "Spotify oscuro". |
| `src/desktop/settings.rs` *(nuevo)* | Modelo de ajustes, carga / validación / guardado. Sin egui ni Windows: todo testeable. |
| `src/desktop/combo.rs` *(nuevo)* | `Combo` (modificadores + tecla), común a atajos de ventana y globales: parseo / texto (`"Ctrl+Alt+P"`), a `egui::Key` y a virtual-key de Windows. Reemplaza `hotkeys::Key` / `hotkeys::Modifiers`. |
| `src/desktop/menu.rs` *(nuevo)* | Barra de menús y sus diálogos (editar colores, tabla de atajos con "grabar combinación"). Devuelve cambios pedidos; no aplica nada por su cuenta. |
| `src/desktop/theme.rs` | `install` pasa a `apply(ctx, &Appearance)`: fuente y colores desde los ajustes, se puede llamar en cualquier frame. Colores leídos de los ajustes, no de `config::theme`. |
| `src/desktop/app.rs` | Dueña de los `Settings` vivos. `title_bar` dibuja la barra de menús entre el ícono y el título; `shortcuts()` usa la tabla configurada en vez del `match` fijo; `Ctrl+rueda`; scrollback / historial / prompt / hora desde ajustes; guardado diferido. |
| `src/desktop/hotkeys.rs` | Usa `Combo`. Nuevo `Hotkeys::replace`: suelta el hilo y registra la lista nueva; si la combinación cambiada falla, vuelve a la anterior. |
| `src/desktop/window.rs` | Tamaño / posición inicial desde ajustes; `ViewportCommand::WindowLevel` (siempre visible); informa tamaño / posición al cerrar. Desactiva el zoom propio de egui (`zoom_with_keyboard`) para que `Ctrl++`/`Ctrl+-` sean los nuestros. |
| `src/desktop/mod.rs` | Carga los ajustes antes del primer frame y pasa los avisos a la consola (como hoy los de atajos). |
| `src/app/engine.rs` | `Input::Playback(PlaybackSettings)` (paso de volumen, umbral de anterior, calidad). `Input::GlobalShortcuts` pasa a `Input::Shortcuts { window, global }` para `help`. Al preparar un `play`, si la calidad pedida difiere de la del reproductor actual, crea uno nuevo en vez de reusarlo. |
| `src/app/backend.rs`, `src/spotify/player.rs` | `connect(volume, bitrate)`: la calidad deja de salir de `config::AUDIO_BITRATE` fijo. |
| `src/app/volume.rs` | `up(step)` / `down(step)` en vez de leer `config::VOLUME_STEP`. La CLI le pasa el default. |
| `src/app/queue.rs` | `previous(elapsed, threshold)`. La CLI le pasa el default. |
| `src/ui/playback.rs` | Solo adapta las llamadas a `up/down/previous` con los defaults de config (AC-15). |
| `docs/arquitectura.md`, `docs/decisiones.md`, `docs/glosario.md`, `CHANGELOG.md` | Ver "Contratos y docs". |

### Estructuras nuevas

- `settings::Settings` — lo que se guarda: `appearance` (`colors:
  Palette`, `font: FontChoice`, `font_size`, `bold_titles`), `window_keys:
  BTreeMap<WindowAction, Option<Combo>>`, `global_keys:
  BTreeMap<GlobalAction, Option<Combo>>`, `playback: PlaybackSettings`,
  `console` (`prompt`, `scrollback`, `history`, `timestamps`), `window`
  (`always_on_top`, `remember_geometry`, `geometry: Option<Geometry>`).
  Invariante: todo valor está dentro de su rango de config.
- `settings::load(dir) -> (Settings, Vec<String>)` — nunca falla: archivo
  ausente → defaults sin aviso; ilegible → defaults + aviso; clave a clave
  sobre `serde_json::Value`, una clave inválida o desconocida → default +
  aviso con su nombre (AC-9).
- `settings::save(dir, &Settings)` — solo las claves distintas del
  default; escribe `ajustes.json.tmp` y renombra (reemplazo atómico).
- `settings::Palette` — un `[u8; 3]` por cada color de hoy (fondo, panel,
  borde, acento, texto, texto fuerte, secundario, aviso, error, hover de
  cerrar). En JSON como `"#1DB954"`.
- `WindowAction` — pausa, siguiente, anterior, subir / bajar volumen,
  stop, shuffle, limpiar consola, agrandar / achicar / tamaño por defecto
  de letra. `Esc`, `↑`, `↓`, `Tab`, `Enter` siguen fijos (son de edición).
- `settings::check_combo(&Settings, target, combo) -> ComboCheck` —
  `Ok` / `Reserved` / `UsedBy(acción)`; lo usa el menú antes de aplicar
  (AC-4). El atajo fijo de restaurar cuenta como usado.
- `menu::MenuBar::show(ui, &Settings) -> Vec<Change>` — `Change` es un
  enum (tema, color, fuente, tamaño, atajo, reproducción, consola,
  ventana, recargar, restaurar sección / todo, abrir archivo). `app.rs`
  los aplica en un solo lugar: actualiza `Settings`, reaplica tema /
  fuente si hace falta, manda `Input::Playback` o `Hotkeys::replace`,
  y agenda el guardado.
- Guardado diferido: cada cambio fija `save_at = ahora +
  SETTINGS_SAVE_DELAY` y pide **un** repintado para esa hora (no
  continuo); al cerrar la ventana se guarda lo pendiente (AC-8, AC-14).
- `Alt`+letra: los menús de egui no lo traen; se abre el menú por su id
  y se navega con el foco de egui (flechas / `Enter` / `Esc`). Si en T3
  el foco de egui no alcanza para navegar con flechas, se avisa antes de
  seguir (no se agrega una dependencia).

### Tests (unitarios, sin ventana)

- `combo`: parseo y texto ida y vuelta (`"Ctrl+Alt+P"`, `"Ctrl+Shift+F12"`,
  flechas, `F1-F12`), texto inválido → error; virtual-keys; a `egui::Key`.
- `settings`: defaults cumplen la invariante; guardar solo diffs; ida y
  vuelta load/save; JSON roto, clave desconocida, color `"#12345"`, tamaño
  fuera de rango, atajo inexistente → default de esa clave + aviso con su
  nombre y el resto se respeta; archivo no se escribe al cargar.
- `check_combo`: reservada, usada por otra acción, usada por el atajo fijo,
  libre.
- Restaurar sección vs. todo.
- `volume` / `queue`: paso y umbral pasados por parámetro.
- `engine`: `Input::Playback` cambia el paso de `vol +` y el umbral de
  `prev`; con otra calidad, el próximo `play` conecta un reproductor
  nuevo y lo que suena no se toca (con el backend falso de los tests).
- `app`: achicar el scrollback descarta las más viejas; `help` muestra
  los atajos configurados (AC-12).
- `hotkeys`: `replace` libera el anterior y registra el nuevo; con el
  nuevo tomado vuelve al anterior (mismo método que el test de spec 006).
- Manual (con audio real): AC-1, AC-2, AC-3, AC-7, AC-13, AC-14 (con
  `scripts/medir-consumo.ps1`), AC-15.

### Contratos y docs

- Doc-comments de contrato nuevos: `settings::load`, `settings::save`,
  `settings::check_combo`, `Combo::parse`, `MenuBar::show`,
  `Hotkeys::replace`, `theme::apply`.
- Actualizar contratos que cambian de firma: `Volume::up/down`,
  `Queue::previous`, `Player::connect`, `Backend::connect`,
  `hotkeys::spawn`/`configured`, `theme::install` (→ `apply`).
- `docs/arquitectura.md`: nuevos módulos `desktop::settings`, `combo`,
  `menu`; `DesktopApp` pasa a ser dueña de los ajustes; flujo
  menú → `Change` → motor / hotkeys / tema.
- `docs/decisiones.md`: ajustes en JSON (sin TOML), solo diffs; menú en la
  barra de título; calidad desde el próximo `play`; atajo fijo de
  restaurar; fuentes solo monoespaciadas de un catálogo.
- `docs/glosario.md`: "ajustes", "atajo de ventana" vs. "atajo global".

## Tareas

- [x] **T1** — `combo.rs`: `Combo` con parseo / texto / virtual-key /
      `egui::Key`, y `hotkeys` migrado a `Combo` sin cambiar
      comportamiento. Tests.
- [ ] **T2** — `settings.rs` + defaults / rangos / presets / catálogo en
      `config.rs`: modelo, `load` tolerante, `save` atómico solo con
      diffs, `check_combo`, restaurar. Tests (AC-8, AC-9, AC-11 lógica).
- [ ] **T3** — Barra de menús en la barra de título, vacía de acciones:
      abrir con clic y `Alt`+letra, flechas, `Esc`; arrastre, doble clic
      y botones de ventana intactos (AC-1). Medir CPU en reposo.
- [ ] **T4** — Tema y fuente vivos: `theme::apply`, menús Tema y Fuente,
      "Editar colores…", `Ctrl++`/`Ctrl+-`/`Ctrl+0`/`Ctrl+rueda`
      (AC-2, AC-3).
- [ ] **T5** — Atajos de ventana configurables: tabla, grabar
      combinación, reservadas, conflictos (AC-4).
- [ ] **T6** — Atajos globales configurables: `Hotkeys::replace` con
      vuelta atrás; `help` con los atajos vigentes (AC-5, AC-12).
- [ ] **T7** — Reproducción: `Input::Playback`, paso / umbral por
      parámetro, calidad al próximo `play`; CLI con defaults (AC-6,
      AC-15).
- [ ] **T8** — Menús Consola y Ventana: prompt, scrollback, historial,
      hora, siempre visible, recordar tamaño / posición (AC-7).
- [ ] **T9** — Persistencia viva: guardado diferido, guardar al cerrar,
      Ajustes → abrir archivo / recargar / restaurar todo, atajo fijo
      `Ctrl+Shift+F12` (AC-8, AC-10, AC-11).
- [ ] **T10** — Verificación manual con audio real (AC-13) y consumo con
      `scripts/medir-consumo.ps1` (AC-14); docs, contratos y changelog.

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
