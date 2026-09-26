# Arquitectura

Vista de conjunto del sistema: qué módulo hace qué, quién depende de quién,
y qué reglas estructurales no se pueden romper. Se actualiza en el mismo
commit que cualquier cambio que la invalide — ver la regla de sincronización
en `CLAUDE.md`.

*(Completar al escribir el primer spec real. Lo mínimo que tiene que tener
un proyecto nuevo antes de que el primer agente toque código:)*

## Mapa de módulos y flujo de datos

```
main ──> ui::cli (parseo de subcomandos)
  │
  └──> spotify::auth ──> librespot-oauth ──> accounts.spotify.com
            │
            └──> cache de token (%APPDATA%\spotify-terminal	oken.json)

config  <── usado por todos (constantes, rutas)
error   <── usado por todos (AppError con mensajes para el usuario)
```

## Quién posee qué estado
- **Cache de token:** solo `spotify::auth` lo lee y escribe (escritura
  atómica: temporal + rename). El resto pide un token con
  `auth::get_valid_token`.

## Reglas estructurales
<!-- Ej: qué capas pueden depender de I/O externo y cuáles deben quedar puras
y testeables sin él. Nace de problemas reales encontrados, no se inventa
de antemano — documentar cuando aparezca uno. -->

## Contratos de funciones/módulos públicos
El contrato completo (pre/postcondiciones, invariantes, qué no debe hacer)
vive como doc-comment junto a cada función pública, no acá. Esta sección
solo linkea (tabla función → archivo) para no duplicar — si hay diferencia
entre esta tabla y el código, el código es la fuente de verdad y hay que
corregir la tabla.

| Función/módulo | Archivo | Contrato en |
|---|---|---|
| `Config::load`, `Config::token_cache_path` | `src/config.rs` | doc-comment |
| `auth::get_valid_token`, `auth::logout`, `auth::Token` | `src/spotify/auth.rs` | doc-comment |
| `cli::parse` | `src/ui/cli.rs` | doc-comment |
