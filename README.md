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
.\target\release\spotify-terminal.exe play https://open.spotify.com/album/<ID>
.\target\release\spotify-terminal.exe logout
```

`play` acepta un tema, un álbum o una playlist (URI `spotify:…`, link de
`open.spotify.com` o ID de tema). Durante la reproducción: **espacio** pausa
y reanuda, **q** sale.

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
