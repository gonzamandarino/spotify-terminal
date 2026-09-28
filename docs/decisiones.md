# Decisiones

Una entrada por decisión: fecha, decisión, alternativas, motivo, y
**Estado** (`Vigente` | `Reemplazada por <fecha/spec>`). No se borran
entradas viejas — se marcan reemplazadas.

## 2026-09-26 — Lenguaje: Rust
- **Decisión:** el cliente se escribe en Rust.
- **Alternativas:** Go (`go-librespot`), Python (con reproductor externo).
- **Motivo:** `librespot` es Rust y se embebe sin puente; binario único y el
  menor consumo de memoria.
- **Estado:** Vigente (spec 001)

## 2026-09-26 — Audio: `librespot` embebido en el mismo proceso
- **Decisión:** el cliente es a la vez dispositivo de reproducción, usando
  las crates de `librespot`, sin depender de otra app de Spotify abierta.
- **Alternativas:** controlar un dispositivo Spotify Connect existente vía
  Web API; `spotifyd` como proceso aparte.
- **Motivo:** un solo proceso liviano que reemplaza a la app oficial.
  Requiere Premium (confirmado). Respaldo si Spotify rompe `librespot`:
  controlar un dispositivo Connect vía Web API.
- **Estado:** Vigente (spec 001)

## 2026-09-26 — Consumo: RAM baja, pero la reproducción manda
- **Decisión:** objetivo ≤ 60 MB de RAM reproduciendo. Si reducir memoria
  (buffers de audio, caché) provoca cortes o demoras audibles, se prioriza
  la reproducción y se documenta el consumo real acá.
- **Motivo:** pedido explícito — "bajo, pero que no afecte la reproducción".
- **Estado:** Vigente (spec 001)

## 2026-09-26 — Sin agente mentor de Rust
- **Decisión:** se elimina `rust-mentor` y su registro de aprendizaje.
- **Motivo:** costo (tokens/tiempo) sin beneficio suficiente — pedido de vos.
  Se mantienen las reglas de calidad de Rust (`fmt`, `clippy`, sin `unwrap`).
- **Estado:** Vigente

## 2026-09-26 — La sesión de audio usa el Client ID de librespot (Web API: ver abajo)
- **Decisión:** la sesión de `librespot` se autentica con un token emitido
  para el Client ID de librespot (`65b708073fc0480ea92a077233ca87bd`).
- **Evidencia:** spike T2 del spec 001 — con nuestro Client ID, `login5`
  rechaza el token (`INVALID_CREDENTIALS`) y ningún tema carga.
- **Web API:** se eligió primero usar ese mismo token (un solo login).
  **Reemplazado** por la entrada "Dos tokens" de abajo.
- **Estado:** Vigente

## 2026-09-26 — Dos tokens: audio (librespot) y Web API (Client ID propio)
- **Decisión:** el token de audio usa el Client ID de librespot; el de la Web
  API usa el Client ID propio (`SPOTIFY_CLIENT_ID` en `.env`).
- **Evidencia:** T4 del spec 001 — con el Client ID de librespot, `GET /me`
  dio 429 permanente (6 intentos en 5 min respetando `Retry-After`); con el
  propio, 200. El Client ID de librespot lo comparten todos sus usuarios y su
  cuota de Web API está agotada.
- **Alternativa descartada:** usar solo APIs internas de librespot (sin Web
  API): no oficial, sin búsqueda documentada, likes/edición inciertos.
- **Costo:** la primera vez se autoriza dos veces; después, nunca.
- **Estado:** Vigente (reemplaza el "un solo login" de la entrada anterior)

## 2026-09-26 — Temas de álbum/playlist por la sesión de audio
- **Decisión:** `play` de un álbum o playlist obtiene la lista de temas con
  `librespot-metadata` sobre la sesión de audio ya abierta, no con la Web
  API.
- **Motivo:** no gasta cuota de la Web API ni pide scopes nuevos, y la
  sesión ya está abierta para reproducir. La crate ya era dependencia.
- **Alternativa:** `GET /albums/{id}/tracks` y `/playlists/{id}/items` con el
  token Web; queda para cuando haga falta paginar o mostrar más datos
  (spec de playlists).
- **Estado:** Vigente (spec 001, T7)

## 2026-09-26 — La sesión de audio corre en un hilo propio
- **Decisión:** la `Session` de librespot se crea y atiende en un hilo con
  su propio runtime tokio de un solo hilo (`player::AudioRuntime`), no en
  el runtime de `main`.
- **Motivo:** librespot lanza las tareas de la sesión en el runtime de quien
  la crea, y los pedidos de clave de audio tienen timeout de 1,5 s.
  Cualquier bloqueo de `main` (login, disco, render de la futura TUI)
  podía hacer fallar la carga de un tema. Gana la reproducción.
- **Costo:** un hilo más (stack chico, sin trabajo cuando no hay I/O).
- **Estado:** Vigente (spec 001, revisión de código)

## 2026-09-26 — Dependencias chicas agregadas en la revisión de código
- **`crossterm` feature `event-stream`** + **`futures-util`** (sin
  features): las teclas se leen como `Stream` dentro del `select!`. Reemplaza
  un hilo lector propio que quedaba vivo y se comía la próxima tecla.
- **`cpal`**: ver si hay dispositivo de salida antes de abrir la sesión
  (si no hay, rodio hace panic dentro de librespot).
- **`oauth2`** (solo dev): test que fija el texto de error del que depende
  la detección de "refresh token rechazado".
- Las tres ya venían como dependencias de librespot: no se compila nada
  nuevo.
- Se quitó `env_logger` (solo lo usaba el spike, que se borró).
- **Estado:** Vigente (spec 001)


## 2026-09-26 — Búsqueda: un pedido a `/search`, playlists con `limit` 10
- **Decisión:** `play <texto>` y `play list <texto>` hacen un solo
  `GET /search` con `market=from_token`. Temas: `limit=5`. Playlists:
  `limit=10` (el máximo en modo desarrollo) y se muestran las 5 primeras
  que no vienen `null`.
- **Motivo:** el spike (spec 002, T1) mostró que Spotify manda `null` en
  lugar de las playlists editoriales/algorítmicas (las apps en modo
  desarrollo no las ven): con `limit=5` a veces quedaban 1 o 2 resultados.
  Pedir 10 sigue siendo un solo pedido.
- **Limitación aceptada:** las playlists de Spotify ("Today's Top Hits",
  "This Is…", "Discover Weekly") no aparecen en la búsqueda. Se pueden
  reproducir igual con su link, porque la lista de temas va por la sesión
  de audio.
- **Estado:** Vigente (spec 002)

## 2026-09-26 — Cola de reproducción propia, no la de Spotify Connect
- **Decisión:** shuffle, siguiente/anterior y encolar se resuelven en
  `ui::playback::Queue`, que decide qué tema pedirle a librespot. No se usa
  `POST /me/player/queue` ni los endpoints de `/me/player/*`.
- **Motivo:** esos endpoints controlan un dispositivo Spotify Connect, y
  nuestro reproductor es librespot embebido sin Spotify Connect (spec 001):
  Spotify no conoce su cola. Además no gastan cuota de la Web API.
- **Costo:** la cola no se ve ni se maneja desde el celular ni desde la app
  oficial, y se pierde al salir.
- **Alternativa:** activar Spotify Connect en librespot (`Spirc`) y usar la
  cola de Spotify; más memoria y otro spec.
- **Estado:** Vigente (spec 003)

## 2026-09-26 — `rand` como dependencia directa
- **Decisión:** `rand` 0.9 para mezclar la cola (`SliceRandom::shuffle`).
- **Motivo:** ya la compila librespot con la misma versión y las mismas
  features: no suma código al binario.
- **Estado:** Vigente (spec 003)


## 2026-09-27 — App de escritorio: ventana propia en Rust
- **Decisión:** la app de escritorio es una ventana propia hecha en Rust
  (egui), que dibuja su propia consola.
- **Alternativas:** Windows Terminal con un perfil propio (no está
  instalado en la máquina de prueba y el look queda limitado al perfil);
  Tauri + xterm.js (WebView2: el más pesado, ~100 MB o más).
- **Motivo:** elegido por vos al pedir el spec 004: un solo `.exe` y
  control total del look.
- **Estado:** Vigente (spec 004)

## 2026-09-27 — La ventana se dibuja por CPU, sin GPU
- **Decisión:** egui 0.34 + `egui_software_backend` (rasteriza por CPU) +
  `softbuffer` (muestra la imagen con GDI) + `winit` (ventana). El loop de
  la ventana es nuestro (`desktop::window`); del crate solo se usa el
  rasterizador.
- **Evidencia (spike T5):** la misma ventana mínima con `eframe`/`glow`
  (OpenGL) ocupaba **120 MB** en reposo: el driver de NVIDIA carga
  `nvgpucomp64.dll` (~106 MB) y `nvoglv64.dll` (~47 MB). Por CPU:
  **19 MB**. Con la consola completa: 26 MB en reposo y 0 ms de CPU en
  10 s; reproduciendo 10 min, máximo 36,9 MB y 0,07 % de CPU promedio
  (AC-12).
- **Costos:** `egui_software_backend` es joven (0.0.3) y fija egui en 0.34.
  Cada redibujo cuesta CPU (con caché por zonas: solo se rehace lo que
  cambió). Por eso el cursor de texto no titila (titilar redibujaba todo
  cada medio segundo: ~6 % de un núcleo en reposo), y la barra de progreso
  se redibuja una vez por segundo solo mientras suena algo.
- **Sin bordes redondeados:** GDI no maneja transparencia por pixel; la
  ventana es rectangular con un borde de 1 px. Cambio respecto del supuesto
  del spec.
- **Respaldo:** si el crate se abandona o rompe, volver a `eframe`/`glow`
  cuesta ~100 MB más pero cambia solo `desktop::window`.
- **Estado:** Vigente (spec 004)

## 2026-09-27 — Dos ejecutables del mismo crate
- **Decisión:** `spotify-terminal.exe` (CLI, sin cambios de uso) y
  `spotify-desktop.exe` (ventana, subsistema "windows": sin consola negra
  detrás). La lógica está en la librería (`src/lib.rs`).
- **Motivo:** un `.exe` con subsistema "windows" pierde la salida de
  consola; no puede ser CLI y ventana a la vez.
- **Estado:** Vigente (spec 004)

## 2026-09-27 — Dependencias de la app de escritorio
- `egui` (con `default_fonts` como respaldo de símbolos), `egui-winit`
  (con portapapeles para copiar/pegar en la línea de entrada),
  `egui_software_backend` (solo el rasterizador), `winit`, `softbuffer`,
  `bytemuck` (ver el buffer de softbuffer como pixeles, sin `unsafe`).
- `windows-sys` 0.52 (la misma que ya trae winit) solo para `MessageBoxW`:
  mostrar un error fatal sin consola.
- `winresource` (solo al compilar, en Windows): ícono de los `.exe`.
- Fuente: ver "La consola usa Consolas" (reemplazó a JetBrains Mono
  embebida).
- El `.exe` de la ventana pesa ~8 MB (la CLI, ~4,5 MB).
- **Estado:** Vigente (spec 004)

## 2026-09-27 — El motor de la app de escritorio corre en su propio hilo
- **Decisión:** `app::engine` atiende comandos, eventos del reproductor y
  tareas de red en un hilo "motor" con runtime tokio de un hilo. La ventana
  (hilo principal) y el motor se hablan solo por canales.
- **Motivo:** la ventana tiene que seguir respondiendo mientras se busca o
  se espera un login, y el reproductor tiene que seguir recibiendo sus
  eventos (el siguiente tema no puede esperar a un redibujo).
- **Hallazgo:** `librespot-oauth` espera el callback del navegador con una
  llamada bloqueante dentro de una función `async`. En la CLI no importa;
  en el motor habría frenado la cola. Los tokens se piden con
  `spawn_blocking`. Si se cancela un login con Esc, ese hilo sigue
  esperando el callback hasta que se completa o se cierra la app (un
  segundo `login` avisaría que el puerto está ocupado).
- **Estado:** Vigente (spec 004)

## 2026-09-27 — Atajos globales con RegisterHotKey en un hilo propio
- **Decisión:** los atajos globales se registran con `RegisterHotKey` de
  Windows, sin ventana, desde un hilo "atajos" que espera en
  `GetMessageW` y manda cada atajo al motor como un `Input`.
- **Alternativas descartadas:**
  - Hook de teclado global (`WH_KEYBOARD_LL`): vería todas las teclas del
    sistema y se despierta con cada una.
  - El hook de mensajes de winit (`with_msg_hook`): ata los atajos al
    loop de la ventana.
  - La crate `global-hotkey`: sumaría una dependencia (con su propia
    ventana oculta) para ~60 líneas de Win32. `windows-sys` ya estaba (la
    trae winit); solo se sumaron features.
- **Combinaciones:** `Ctrl+Alt+…`. `Ctrl+Shift` tapaba la selección de
  texto en las demás apps. `Ctrl+Alt+Espacio` ya la registraba otra app
  en la PC de desarrollo (error 1409), así que pausa va en `Ctrl+Alt+P`.
  La lista está en `config::GLOBAL_SHORTCUTS` (desde spec 007, los
  defaults: se cambian desde el menú Atajos).
- **Estado:** Vigente (spec 006)

## 2026-09-27 — Volumen por software de librespot, no el de Windows por app
- **Decisión:** el volumen de la app se aplica con el `SoftMixer` de
  librespot (escala cada muestra antes de la salida), con su curva
  logarítmica por defecto.
- **Alternativa descartada:** el volumen por app de Windows (WASAPI
  `ISimpleAudioVolume`, el que muestra el mezclador). Necesita llegar a la
  sesión de audio que abre rodio por dentro, más código específico de
  Windows y más features de `windows-sys`.
- **Consecuencias:** sin dependencias nuevas. El mezclador de Windows
  sigue mostrando la app al 100 %: el volumen de la app se suma por
  debajo. Con la curva lineal, de 50 % a 100 % casi no se nota el cambio;
  con la logarítmica cada paso de 5 % suena parecido.
- **Estado:** Vigente (spec 005)

## 2026-09-27 — La consola usa Consolas, la fuente de la consola de Windows
- **Decisión:** la app de escritorio usa Consolas (normal y negrita),
  leída de `%WINDIR%\Fonts` al arrancar. Si no está, la monoespaciada de
  egui.
- **Alternativa anterior:** JetBrains Mono embebida en el `.exe` (OFL,
  ~540 KB). Reemplazada a pedido de vos: querías la fuente de la consola.
- **Por qué no se embebe:** la licencia de Consolas no permite
  redistribuirla; viene con Windows desde Vista.
- **Estado:** Vigente (spec 004; reemplaza la fuente de la entrada
  "Dependencias de la app de escritorio")

## 2026-09-27 — Ajustes en JSON, solo lo que difiere de fábrica
- **Decisión:** los ajustes de la app de escritorio (spec 007) se guardan
  en `%APPDATA%\spotify-terminal\ajustes.json`, con claves en castellano
  por sección (`tema`, `fuente`, `atajos_ventana`...), y solo las que
  difieren del default. Cada clave se valida por separado: una inválida
  vuelve a fábrica con un aviso y no arrastra a las demás.
- **Alternativa descartada:** TOML (más cómodo a mano, pero una
  dependencia nueva; `serde_json` ya estaba).
- **Consecuencias:** un default nuevo le llega al usuario que no lo
  cambió. Un archivo con avisos no se pisa hasta que el usuario cambie
  algo, para no perder lo que escribió. Se acepta UTF-8 con BOM (lo
  escriben el Bloc de notas y PowerShell 5.1).
- **Estado:** Vigente (spec 007)

## 2026-09-27 — Barra de menús en un renglón bajo la barra de título
- **Decisión:** los menús de customización (Tema, Fuente, Atajos,
  Reproducción, Consola, Ventana, Ajustes) van en un renglón propio
  debajo de la barra de título; arriba sigue ♫ + "spotify-terminal" +
  botones de ventana. Se abren con clic o `Alt`+letra subrayada;
  `Alt`+esas letras no se aceptan como atajo de ventana. Usa los menús
  que ya trae egui (sin dependencias).
- **Alternativa descartada:** los menús dentro de la barra de título,
  entre el ícono y el título (como VS Code). Fue la primera versión;
  se cambió a pedido tuyo al verla.
- **Consecuencias:** la consola tiene un renglón menos de alto.
- **Estado:** Vigente (spec 007); los siete menús se juntaron en tres en
  spec 009 (ver "Menú Personalización con submenús").

## 2026-09-27 — El tamaño de letra es un zoom de toda la ventana
- **Decisión:** el tamaño de letra de los ajustes se aplica como zoom de
  egui (`set_zoom_factor`): texto, barras y márgenes crecen juntos, como
  `Ctrl++` en VS Code. El zoom propio de egui con el teclado se apaga
  para que `Ctrl++`/`Ctrl+-`/`Ctrl+0` sean atajos configurables.
- **Alternativa descartada:** cambiar solo el tamaño del texto (las
  barras de alto fijo lo cortaban con letra grande).
- **Estado:** Reemplazada por "El tamaño de letra cambia el texto, no la
  UI" (2026-09-27, spec 009).

## 2026-09-27 — La calidad de audio cambia desde el próximo `play`
- **Decisión:** librespot fija la calidad al crear el reproductor. Al
  cambiarla, el motor sigue con el reproductor actual y abre uno nuevo en
  el próximo `play` (lo que suena sigue hasta que el nuevo está listo).
- **Alternativa descartada:** recrear el reproductor en el momento (corta
  el tema que suena; la reproducción tiene prioridad).
- **Estado:** Vigente (spec 007)

## 2026-09-27 — Atajo fijo para restaurar todo
- **Decisión:** `Ctrl+Shift+F12` vuelve todos los ajustes a fábrica y no
  se puede reasignar ni quitar (`config::RESTORE_ALL_SHORTCUT`).
- **Motivo:** un tema ilegible o atajos borrados podrían dejar el menú
  inusable. Borrar `ajustes.json` a mano también vuelve a fábrica.
- **Estado:** Vigente (spec 007)

## 2026-09-27 — Solo fuentes monoespaciadas de un catálogo
- **Decisión:** el menú Fuente ofrece las de `config::FONT_CATALOG`
  (Consolas, Cascadia Mono, Courier New, Lucida Console) que estén
  instaladas, más la de egui. Se mira solo si el archivo existe; se lee
  entera solo la elegida.
- **Alternativa descartada:** listar todas las fuentes del sistema (leer
  cada archivo para saber su nombre cuesta RAM y arranque; las
  proporcionales desarman las columnas de la consola).
- **Estado:** Vigente (spec 007)

## 2026-09-27 — Cada usuario usa su propio Client ID (camino A)
- **Decisión:** la app no trae un Client ID; la primera vez guía para
  crear una app en el Developer Dashboard y pide el suyo (`setup`).
- **Motivo:** desde feb 2026 una app en modo desarrollo admite 5 cuentas
  cargadas a mano y se cae para todos si su dueño pierde Premium; el
  *Extended Quota Mode* pide empresa y 250.000 usuarios activos por mes.
  Como reproducir ya exige Premium, crear la app no agrega requisitos.
- **Alternativa descartada:** Client ID compartido (camino B): 5 personas
  como máximo y un único punto de falla.
- **Estado:** Vigente (spec 008)

## 2026-09-27 — Client ID en `client-id.txt`, no en `ajustes.json`
- **Decisión:** `setup` lo guarda en `%APPDATA%\spotify-terminal\client-id.txt`
  (texto plano, escritura atómica). Prioridad: variable de entorno
  `SPOTIFY_CLIENT_ID`, `.env`, `client-id.txt`; así el flujo de
  desarrollo con `.env` no cambia.
- **Motivo:** cambiarlo no personaliza la app, cambia de cuenta de
  desarrollador (y borra el token de la Web API); `ajustes.json` solo
  guarda lo que difiere de fábrica y la CLI no lo lee. No es secreto: con
  PKCE no hay client secret.
- **Estado:** Vigente (spec 008)

## 2026-09-27 — Chequeo del Client ID antes de abrir el navegador
- **Decisión:** antes del login interactivo de la Web API se canjea un
  código falso en `/api/token`: `invalid_client` → el Client ID no
  existe (`InvalidClientId`); cualquier otra respuesta sigue al login.
- **Motivo:** con un Client ID inexistente Spotify muestra el error en el
  navegador y nunca llama al callback: la app se quedaba esperando. Es un
  pedido más, solo en el login interactivo (nunca en el refresh).
- **Alternativa descartada:** `GET /authorize` (sin sesión en el
  navegador responde 303 al login para cualquier Client ID). La Redirect
  URI mal registrada no se puede detectar así; la guía insiste en
  copiarla exacta.
- **Estado:** Vigente (spec 008)

## 2026-09-27 — Distribución: zip portable, sin instalador ni firma
- **Decisión:** un `.zip` con los dos `.exe`, `LEEME.txt` y el script de
  accesos directos (más un `.cmd` que lo llama con `-ExecutionPolicy
  Bypass`), publicado en GitHub Releases por `release.yml` al pushear un
  tag `vX.Y.Z` igual a la versión de `Cargo.toml`. El zip se arma desde
  una lista explícita de archivos (`scripts/armar-zip.ps1`).
- **Motivo:** proyecto chico: un instalador (MSI/MSIX) o autoupdate es
  más mantenimiento que valor. Los `.exe` no se firman (el certificado se
  paga todos los años); el LEEME explica cómo pasar SmartScreen.
- **Estado:** Vigente (spec 008). El disparo por tag a mano quedó
  reemplazado como camino normal por "Un Release por spec, publicado al
  mergear" (el tag a mano sigue andando).

## 2026-09-27 — Runtime de C estático (`+crt-static`)
- **Decisión:** `.cargo/config.toml` enlaza el CRT dentro del `.exe` para
  `x86_64-pc-windows-msvc`, también al compilar en local.
- **Motivo:** sin eso los `.exe` piden `vcruntime140.dll` y las
  `api-ms-win-crt-*`, que una PC sin Build Tools ni Visual C++
  Redistributable puede no tener. Cuesta ~200 KB por `.exe`.
  `scripts/verificar-dependencias.ps1` lo chequea en CI.
- **Estado:** Vigente (spec 008)

## 2026-09-27 — Un Release por spec, publicado al mergear
- **Decisión:** cada spec que cambia lo que se distribuye cierra con una
  versión nueva (semver en `0.x`: spec nuevo → minor, corrección →
  patch). El PR sube `Cargo.toml` / `Cargo.lock` y trae su sección
  `## [X.Y.Z]` en el changelog (lo exige `version-check.yml`); al
  mergearlo, `release.yml` crea el tag y el Release solo. El changelog ya
  no tiene sección "Sin publicar".
- **Motivo:** que publicar sea parte del flujo spec → docs → código → PR
  y no un paso que se olvida. La versión se decide en el plan y se revisa
  en el PR, como el resto del spec; nadie crea tags a mano.
- **Alternativa descartada:** tag a mano después del merge (se olvida, y
  el tag puede no coincidir con `Cargo.toml`); versionar con cada merge
  aunque sea de docs (Releases vacíos).
- **Estado:** Vigente

## 2026-09-27 — Menú Personalización con submenús
- **Decisión:** la barra de menús queda con Personalización,
  Reproducción y Ajustes (`Alt+P` / `Alt+R` / `Alt+A`). Tema, Fuente,
  Atajos, Consola y Ventana pasan a ser submenús de Personalización, con
  el mismo contenido. egui abre un submenú con el mouse o Enter; se suma
  → para abrirlo y ← para cerrarlo y volver a su botón (en un slider o en
  el campo del prompt, las flechas siguen siendo de ellos).
- **Motivo:** menos cosas en la barra, pedido tuyo (spec 009).
- **Alternativa descartada:** un diálogo "Personalización" con pestañas
  (más código nuevo; los submenús reusan los menús de spec 007).
- **Consecuencias:** `Alt+T`/`F`/`C`/`V`/`J` quedan libres como atajo
  de ventana y `Alt+P` pasa a estar reservada; un `ajustes.json` con un
  atajo de ventana en `Alt+P` vuelve a fábrica en ese atajo, con aviso.
- **Estado:** Vigente (spec 009)

## 2026-09-27 — El tamaño de letra cambia el texto, no la UI
- **Decisión:** el tamaño de letra de los ajustes cambia solo el texto
  de la consola, la línea de entrada y la barra "sonando", en las mismas
  proporciones de antes. La barra de título, la de menús, los menús
  desplegados y los diálogos tienen letra fija
  (`config::layout::UI_FONT_SIZE`). "Sonando" y la entrada crecen con la
  letra para que no se corte (`desktop::layout::bars`); si la ventana no
  da, bajan hasta lo justo para el texto y la consola se achica, sin que
  nada se superponga.
- **Motivo:** el zoom de toda la ventana (spec 007) agrandaba la UI
  entera: con 32 pt las barras ocupaban más que la ventana mínima y la
  consola desaparecía (spec 009).
- **Alternativa descartada:** seguir con el zoom y agrandar la ventana
  mínima (la UI seguiría creciendo en vez de la letra).
- **Estado:** Vigente (spec 009)

## 2026-09-27 — Visualización: 15 fps y solo mientras suena
- **Decisión:** la visualización (onda, barras, vinilo) se redibuja a
  `config::viz::FPS` = 15 cuadros por segundo y solo con música sonando y
  la ventana visible. En pausa, stop o sin visualización no pide cuadros.
  Ninguna es el default. Las muestras se copian solo con Onda o Barras.
- **Motivo:** el costo es redibujar la ventana por CPU. Medido con un
  prototipo en 920×580 (spec 010): 30 fps = 16–18 % de un núcleo; 15 fps
  = 7–9 %; quieto = 0 %.
- **Alternativa descartada:** 30 fps (el doble de CPU sin verse mucho
  mejor); fps elegibles desde el menú (pedido tuyo: fijo en config).
- **Estado:** Vigente (spec 010). Reconfirmada el 2026-09-28: tras la
  prueba con audio real se probó 30 fps (más fluido) y se volvió a 15
  para mantener el tope de 10 % de un núcleo (AC-8); la fluidez se busca
  con el suavizado de la onda y la caída de las barras.

## 2026-09-27 — Muestras para dibujar: copia que nunca frena el audio
- **Decisión:** la salida de audio de librespot se envuelve en `TapSink`,
  que pasa cada paquete tal cual y copia sus muestras (mono, sin el
  volumen) a `AudioTap` con `try_lock`: si la ventana lo está leyendo, se
  saltea esa copia. El volumen se saca dividiendo por el factor que aplicó
  librespot, así la forma no cambia con el volumen.
- **Motivo:** la reproducción gana siempre; perder un paquete del dibujo
  no se nota, esperar en el hilo de audio sí.
- **Alternativa descartada:** un canal con cada paquete (reserva memoria
  por paquete en el hilo de audio y se acumula si la ventana no lee).
- **Estado:** Vigente (spec 010)

## 2026-09-27 — FFT propia y JPEG de `image` para las tapas
- **Decisión:** la FFT de las barras es radix-2 escrita a mano (~60
  líneas, con test contra una DFT). Las tapas (JPEG de `i.scdn.co`) se
  decodifican con `image`, que ya estaba en el árbol por el portapapeles
  de `egui-winit`, sumándole el formato `jpeg` (`zune-jpeg`, Rust puro).
  Se bajan una vez por tema, de ~300 px, en el motor.
- **Motivo:** una FFT de 1024 puntos 15 veces por segundo no justifica
  `rustfft`; para JPEG no hay alternativa sin dependencia y esta es la
  más chica (no suma otra crate de imágenes).
- **Estado:** Vigente (spec 010)

## 2026-09-27 — La tapa se recorta en círculo en el vinilo
- **Decisión:** en Vinilo la tapa del disco es la etiqueta del centro,
  recortada en círculo y girando (~10 rpm, `config::viz::VINYL_RPM`).
- **Consecuencias:** las guías de marca de Spotify piden no recortar ni
  alterar las tapas. Para un cliente personal se acepta; si el proyecto
  se distribuyera más allá, revisar.
- **Estado:** Vigente (spec 010)

## 2026-09-28 — Barras relativas a lo más fuerte reciente, onda con disparo
- **Decisión:** las barras miden cada banda en dB con una pendiente de
  +3 dB/octava desde 1 kHz y la altura es relativa a una referencia que
  sube de golpe a la banda más fuerte y baja 4 dB/s (rango visible 36 dB,
  `config::viz::BAR_*`). La onda arranca en un cruce por cero hacia arriba
  (de la señal pasada por un pasabajos), promedia las muestras de cada
  punto y mezcla 40 % del cuadro anterior.
- **Motivo:** con un piso fijo de −60 dB la música llenaba casi todas las
  barras todo el tiempo y los agudos no se movían ("casi quietas"); la
  onda saltaba de fase en cada cuadro y se veía como ruido.
- **Estado:** Vigente (spec 010)

## 2026-09-28 — Salida de audio propia que sigue al dispositivo por defecto
- **Decisión:** en vez de la salida rodio de librespot
  (`audio_backend::find`), `spotify::output::DeviceSink`: también rodio
  (0.21, misma versión y features que ya trae librespot, ahora como
  dependencia directa), pero reabre el dispositivo por defecto si el
  abierto avisa un error, si cambió el de por defecto (se mira cada 1 s
  mientras suena) o si su cola no baja en 1 s. Sin dispositivo, devuelve
  error y librespot pausa.
- **Motivo:** la de librespot se queda con el dispositivo que había al
  abrirla; al cambiar la salida de audio de Windows o desconectarla, deja
  de pedir audio y librespot espera a que se vacíe su cola sin límite: no
  suena más y el hilo del reproductor queda colgado (pausa y cerrar la app
  también). No tiene forma de configurarlo desde afuera.
- **Consecuencias:** al pasar de dispositivo se pierde lo que estaba en
  cola en el viejo (≤ ~340 ms). No se suma ninguna crate al árbol.
- **Estado:** Vigente (spec 010)
