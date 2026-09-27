# Arquitectura

Vista de conjunto del sistema: qué módulo hace qué, quién depende de quién,
y qué reglas estructurales no se pueden romper. Se actualiza en el mismo
commit que cualquier cambio que la invalide — ver la regla de sincronización
en `CLAUDE.md`.

## Mapa de módulos y flujo de datos

```
main ──> ui::cli (parseo de subcomandos)
  │
  ├──> spotify::auth ──> librespot-oauth ──> accounts.spotify.com
  │         │   TokenKind::Audio (Client ID librespot)
  │         │   TokenKind::Web   (Client ID propio, .env)
  │         └──> caches %APPDATA%\spotify-terminal\token-{audio,web}.json
  │
  ├──> spotify::web (token Web) ──> api.spotify.com/v1
  │
  └──> ui::playback ──> spotify::player (token Audio) ──> librespot
                           │   (cola de temas, precarga del siguiente)
                           ├──> librespot-metadata: temas de álbum/playlist
                           └──> salida de audio (rodio/WASAPI)

config  <── usado por todos (constantes, rutas)
error   <── usado por todos (AppError con mensajes para el usuario)
```

## Quién posee qué estado
- **Caches de token:** solo `spotify::auth` los lee y escribe (escritura
  atómica: temporal + rename). El resto pide un token con
  `auth::get_valid_token(config, kind)`.
- **Reproductor:** `spotify::player::Player` envuelve el de librespot y su
  sesión; solo `ui::playback` lo controla durante `play`. La cola (lista de
  temas y cuál suena) vive en `ui::playback::play_queue` mientras dura el
  comando.

## Reglas estructurales
- `spotify::web` solo recibe tokens `Web` y `spotify::player` solo `Audio`
  (con el token cruzado, la Web API da 429 y el audio no carga).
- Los temas de un álbum o playlist se piden por la sesión de audio
  (`Player::resolve_tracks`), no por la Web API (ver `docs/decisiones.md`).
- Los errores se traducen a `AppError` en el módulo que los recibe
  (`auth`, `web`, `player`); `main` solo los imprime y sale con código 1.

## Contratos de funciones/módulos públicos
El contrato completo (pre/postcondiciones, invariantes, qué no debe hacer)
vive como doc-comment junto a cada función pública, no acá. Esta sección
solo linkea (tabla función → archivo) para no duplicar — si hay diferencia
entre esta tabla y el código, el código es la fuente de verdad y hay que
corregir la tabla.

| Función/módulo | Archivo | Contrato en |
|---|---|---|
| `Config::load` | `src/config.rs` | doc-comment |
| `auth::get_valid_token`, `auth::logout`, `auth::Token`, `auth::TokenKind` | `src/spotify/auth.rs` | doc-comment |
| `web::current_user`, `web::User` | `src/spotify/web.rs` | doc-comment |
| `player::Player` | `src/spotify/player.rs` | doc-comment |
| `playback::play_queue` | `src/ui/playback.rs` | doc-comment |
| `cli::parse` | `src/ui/cli.rs` | doc-comment |
