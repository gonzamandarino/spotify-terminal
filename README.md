# spotify-terminal

Cliente de Spotify liviano para la terminal, en Rust. El audio lo reproduce
`librespot` dentro del mismo proceso: no hace falta la app oficial abierta.

Requiere **Spotify Premium**. Probado en Windows 10.

## Prerequisitos

1. [rustup](https://rustup.rs/) con el toolchain `stable-x86_64-pc-windows-msvc`.
2. Build Tools de Visual Studio con la carga "Desarrollo para el escritorio
   con C++" (el linker de MSVC).
3. Una app en el [Spotify Developer Dashboard](https://developer.spotify.com/dashboard):
   - Redirect URI: `http://127.0.0.1:8898/login`
   - API: Web API
   - Copiá el **Client ID** (no hace falta el client secret).

## Configuración

```powershell
Copy-Item .env.example .env
# editá .env y poné SPOTIFY_CLIENT_ID=<tu Client ID>
```

`.env` no se commitea. Los tokens se guardan en
`%APPDATA%\spotify-terminal\` (fuera del repo).

## Uso

```powershell
cargo build --release
.\target\release\spotify-terminal.exe login    # la primera vez: autoriza dos veces en el navegador
.\target\release\spotify-terminal.exe whoami
.\target\release\spotify-terminal.exe play never gonna give you up
.\target\release\spotify-terminal.exe play list rock nacional
.\target\release\spotify-terminal.exe play https://open.spotify.com/album/<ID>
.\target\release\spotify-terminal.exe logout
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
| `q` | salir |

La cola vive mientras dura `play` y no se ve desde el celular ni desde la
app oficial: el reproductor no es un dispositivo Spotify Connect (ver
`docs/decisiones.md`).

¿Por qué dos autorizaciones? El audio usa el Client ID de librespot y la Web
API el tuyo; ver `docs/decisiones.md`.

## Medir consumo

```powershell
# en otra terminal, con play corriendo:
.\scripts\medir-consumo.ps1 -Minutos 30
```

## Desarrollo

El proyecto sigue Spec-Driven Development: ninguna feature sin spec en
`specs/` (ver `CLAUDE.md`). Todo commit pasa:

```powershell
cargo fmt --check; cargo clippy --all-targets -- -D warnings; cargo test
```

Mapa de módulos: `docs/arquitectura.md`. Decisiones: `docs/decisiones.md`.
