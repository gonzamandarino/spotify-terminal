# spotify-terminal

Cliente de Spotify liviano para la terminal, en Rust. El audio lo reproduce
`librespot` dentro del mismo proceso: no hace falta la app oficial abierta.

Requiere **Spotify Premium** y Windows 10/11 de 64 bits.

## Usar

1. Bajá `spotify-terminal-vX.Y.Z-windows-x64.zip` de
   [Releases](https://github.com/gonzamandarino/spotify-terminal/releases)
   y descomprimilo donde quieras. No hace falta instalar nada.
2. Abrí `spotify-desktop.exe` (si aparece "Windows protegió su PC":
   "Más información" → "Ejecutar de todas formas"; los `.exe` no están
   firmados).
3. La primera vez te guía para crear tu app en el
   [Spotify Developer Dashboard](https://developer.spotify.com/dashboard)
   y te pide su **Client ID** (cada uno usa el suyo: Spotify limita cada
   app a 5 cuentas). Después autorizás dos veces en el navegador y listo.

`crear-accesos-directos.cmd` crea accesos directos en el escritorio y el
menú Inicio. `setup` cambia el Client ID; `version` muestra la versión.
Todo lo que guarda la app está en `%APPDATA%\spotify-terminal\`. El
`LEEME.txt` del zip tiene los detalles.

### CLI

```powershell
spotify-terminal.exe setup      # configura o cambia el Client ID y hace login (también se pide solo si falta)
spotify-terminal.exe login      # la primera vez: autoriza dos veces en el navegador
spotify-terminal.exe whoami
spotify-terminal.exe play never gonna give you up
spotify-terminal.exe play list rock nacional
spotify-terminal.exe play https://open.spotify.com/album/<ID>
spotify-terminal.exe logout
spotify-terminal.exe --version
```

`play <nombre>` busca temas y `play list <nombre>` (o `play playlist`)
busca playlists: muestra los 5 mejores resultados y se elige con **1-5**
(**Enter** = el primero, **q** = cancelar). Para buscar un tema que empieza
con "list", ponelo entre comillas: `play "list of demands"`. Las playlists
editoriales de Spotify no aparecen en la búsqueda (la API no se las muestra
a apps en modo desarrollo), pero se pueden reproducir con su link.

`play` también acepta un tema, un álbum o una playlist por URI
(`spotify:…`), link de `open.spotify.com` o ID de tema. Con `-s` /
`--shuffle` arranca mezclado.

Durante la reproducción:

| Tecla | Qué hace |
|---|---|
| espacio | pausa / reanudar |
| `n` o → | siguiente tema |
| `p` o ← | reinicia el tema; si recién empezó (< 3 s), vuelve al anterior |
| `s` | shuffle sí / no (el tema actual sigue sonando) |
| `a` | busca un tema y lo agrega a la cola: suena después del actual |
| `+` (o `=`) / `-` | sube / baja el volumen de la app un 5 % |
| `q` | salir |

La cola vive mientras dura `play` y no se ve desde el celular ni desde la
app oficial: el reproductor no es un dispositivo Spotify Connect (ver
`docs/decisiones.md`).

¿Por qué dos autorizaciones? El audio usa el Client ID de librespot y la Web
API el tuyo; ver `docs/decisiones.md`.

## App de escritorio

`spotify-desktop.exe` abre una ventana con una consola propia: se escriben
los mismos comandos, sin el nombre del programa, y la música sigue mientras
se escribe.

| Comando | Qué hace |
|---|---|
| `play …` | igual que en la CLI (`play <nombre>`, `play list <nombre>`, links, `-s`) |
| `pause` | pausa / reanudar (Ctrl+Espacio) |
| `next`, `n` | siguiente tema (Ctrl+→) |
| `prev`, `p` | anterior o reinicia el tema (Ctrl+←) |
| `shuffle`, `s` | shuffle sí / no |
| `queue <tema>`, `a <tema>` | busca un tema y lo encola |
| `like` / `unlike` | agrega / quita el tema que suena de Tus me gusta (♥ en la barra de abajo) |
| `playlists`, `pl` | lista tus playlists (propias y seguidas) y reproduce la que elijas; `-s` mezclado |
| `stop` | corta y vacía la cola |
| `vol`, `v` | muestra el volumen de la app |
| `vol <0-100>`, `vol +`, `vol -` | fija / sube / baja el volumen (Ctrl+↑ / Ctrl+↓) |
| `mute`, `m` | silencia / vuelve al volumen de antes |
| `login`, `logout`, `whoami`, `help` | como en la CLI |
| `setup` | muestra la guía, pide un Client ID nuevo y vuelve a iniciar sesión (Esc cancela) |
| `version` | versión de la app |
| `clear` / `exit` | limpia la consola / cierra |

La barra de abajo tiene botones: ⏮ ⏯ ⏭ en el centro, ♥ al lado del tema
(agrega o quita de Tus me gusta) y 🔀 shuffle a la derecha. Hacen lo mismo
que sus comandos; al pasar el mouse muestran el atajo.

Después de una búsqueda, se elige con el número y Enter (Enter solo = el
primero). ↑/↓ recorren el historial, Tab completa el comando y Esc cancela
una búsqueda o elección. Un `play` nuevo reemplaza lo que suena.

El volumen es solo el de la app (no toca el de Windows) y se recuerda al
cerrar, tanto en la ventana como en la CLI
(`%APPDATA%\spotify-terminal\volumen.txt`).

Atajos globales, que andan con la ventana minimizada o con otra app
enfocada:

| Atajo | Qué hace |
|---|---|
| Ctrl+Alt+P | pausa / reanudar |
| Ctrl+Alt+→ / Ctrl+Alt+← | siguiente / anterior |
| Ctrl+Alt+Enter | stop (corta y vacía la cola) |
| Ctrl+Alt+↑ / Ctrl+Alt+↓ | sube / baja el volumen |

Si otra app ya usa alguno, la consola lo avisa al abrir y ese atajo queda
sin efecto. Se cambian desde Personalización → Atajos.

### Personalizar

Arriba, debajo de la barra de título, están los menús **Personalización**,
**Reproducción** y **Ajustes** (se abren con clic o con `Alt`+la letra
subrayada; en Personalización, → abre un submenú y ← vuelve):

| Menú | Qué se cambia |
|---|---|
| Personalización → Tema | temas predefinidos (Spotify oscuro, Claro, Alto contraste) y cada color |
| Personalización → Fuente | Consolas, Cascadia Mono, Courier New, Lucida Console (las instaladas) o la de la app; tamaño de la letra de la consola, la entrada y la barra "sonando" (también Ctrl++ / Ctrl+- / Ctrl+0 y Ctrl+rueda); negrita |
| Personalización → Atajos | los de la ventana y los globales: clic y apretá la combinación nueva |
| Personalización → Consola | símbolo del prompt, líneas guardadas, historial, hora en cada línea |
| Personalización → Ventana | siempre visible, recordar tamaño y posición |
| Personalización → Visualización | panel a la derecha de la consola con la onda, las barras (espectro) o un vinilo girando con la tapa del disco; se anima solo mientras suena. El ancho se cambia arrastrando el borde izquierdo del panel |
| Reproducción | paso de volumen, cuándo "anterior" reinicia el tema, calidad de audio (desde el próximo `play`) |
| Ajustes | abrir / recargar `ajustes.json`, restaurar todo |

Todo se aplica al instante y se guarda en
`%APPDATA%\spotify-terminal\ajustes.json` (solo lo que difiere de
fábrica; se puede editar a mano y recargar desde el menú). **Ctrl+Shift+F12**
vuelve todo a fábrica y no se puede cambiar.

Si algo falla al abrir la ventana, se muestra un cuadro de error; un panic
queda anotado en `%APPDATA%\spotify-terminal\desktop-panic.log`.

## Desarrollar

### Prerequisitos

1. [rustup](https://rustup.rs/) con el toolchain `stable-x86_64-pc-windows-msvc`.
2. Build Tools de Visual Studio con la carga "Desarrollo para el escritorio
   con C++" (el linker de MSVC).
3. Una app en el [Spotify Developer Dashboard](https://developer.spotify.com/dashboard):
   - Redirect URI: `http://127.0.0.1:8898/login`
   - API: Web API
   - Copiá el **Client ID** (no hace falta el client secret).

### Configuración

```powershell
Copy-Item .env.example .env
# editá .env y poné SPOTIFY_CLIENT_ID=<tu Client ID>
```

`.env` no se commitea. El Client ID se toma de la variable de entorno
`SPOTIFY_CLIENT_ID`, si no del `.env` de la carpeta desde donde arranca el
programa, y si no de `%APPDATA%\spotify-terminal\client-id.txt` (el que
guarda `setup`). Los tokens se guardan en `%APPDATA%\spotify-terminal\`
(fuera del repo).

```powershell
cargo build --release
.\scripts\instalar-acceso-directo.ps1   # accesos directos (arrancan en el repo, para el .env)
.\target\release\spotify-desktop.exe
```

Los `.exe` se enlazan con el runtime de C estático (`.cargo/config.toml`):
no dependen de `vcruntime140.dll`.

### Publicar una versión

Cada spec publica su versión (ver "Versiones y releases" en `CLAUDE.md`):
el PR sube `version` en `Cargo.toml` (y `Cargo.lock`, con `cargo build`) y
agrega la sección `## [X.Y.Z] - fecha` al `CHANGELOG.md`; el check
`version-check` hace fallar el PR si falta algo. Al mergearlo,
`.github/workflows/release.yml` compila, corre los chequeos, arma el zip y
publica el Release `vX.Y.Z` con las notas de esa sección. No hace falta
crear tags a mano. Para revisar el zip sin publicar: "Run workflow" en la
pestaña Actions.

### Medir consumo

```powershell
# en otra terminal, con play corriendo:
.\scripts\medir-consumo.ps1 -Minutos 30
# la app de escritorio:
.\scripts\medir-consumo.ps1 -Minutos 10 -Proceso spotify-desktop
```

### Spec-Driven Development

El proyecto sigue Spec-Driven Development: ninguna feature sin spec en
`specs/` (ver `CLAUDE.md`). Todo commit pasa:

```powershell
cargo fmt --check; cargo clippy --all-targets -- -D warnings; cargo test
```

Mapa de módulos: `docs/arquitectura.md`. Decisiones: `docs/decisiones.md`.
