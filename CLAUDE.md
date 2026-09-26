# spotify-terminal

Cliente de Spotify liviano con UI de terminal, pensado para reemplazar la app
oficial consumiendo pocos recursos (CPU/RAM). Proyecto personal chico.

## Funcionalidades objetivo

- Playlists de Spotify (del usuario) y playlists propias/locales del cliente.
- Likear / deslikear canciones (biblioteca "Tus me gusta").
- Shuffle.
- Elegir y reproducir una canción específica (búsqueda / selección).
- Generar una playlist random a partir de requerimientos del usuario
  (ej. género, artistas, duración total, cantidad de temas).

Cada una entra por su propio spec en `specs/` — nada de esto se implementa
sin spec aprobado (ver "Sistema SDD" abajo).

## Restricciones conocidas (verificar en el spec correspondiente)

- El control de reproducción vía Spotify Web API requiere cuenta **Premium**.
- La Web API controla un dispositivo Spotify Connect; el audio lo emite otro
  proceso (un dispositivo existente, o un player liviano tipo librespot /
  spotifyd). Cuál se usa es una decisión a registrar en `docs/decisiones.md`.
- Autenticación con OAuth (Authorization Code + PKCE). El refresh token se
  cachea localmente y **nunca** se commitea (`.gitignore` + hook pre-commit).
- Endpoints como `/recommendations` y `/audio-features` fueron restringidos
  para apps nuevas (nov 2024): el generador de playlists no debe asumirlos
  disponibles. Confirmar en el spec del generador.
- Stack: **Rust** + `librespot` embebido para el audio (ver
  `docs/decisiones.md`). Bajo consumo de RAM, pero nunca a costa de cortes
  en la reproducción: si hay que elegir, gana la reproducción.

## Agentes

- `pr-agent` / `docs-agent` — PRs y documentación (genéricos del sistema SDD).
- `spotify-api-agent` — cliente Web API, OAuth, playback, likes, playlists (`src/spotify/`).
- `tui-agent` — UI de terminal (`src/ui/`).
- `playlist-gen-agent` — generador de playlists y playlists locales (`src/generator/`, `src/library/`).
- `testing-agent` — tests y verificación de AC, sin tocar código de producción (`tests/`).
- `rust-mentor` — enseña Rust sobre el código real del proyecto; no escribe
  código de producción (`docs/rust-aprendizaje.md`, `ejercicios/`).

## Aprender Rust es parte del proyecto

Vos está aprendiendo Rust con este proyecto. Eso cambia cómo trabajan los
agentes de dominio: código idiomático y legible antes que ingenioso, sin
macros ni abstracciones que el spec no necesite, y comentarios que expliquen
el *por qué* cuando algo es específico de Rust (ownership, lifetimes,
`Arc<Mutex<_>>`). `rust-mentor` interviene en los pasos 3, 5 y 6 del flujo.

## Sistema SDD (Spec-Driven Development)

Ninguna feature nueva o modificación de comportamiento existente se implementa
sin un spec en `specs/` primero. El spec es el contrato entre vos
y el agente: evita que el agente asuma cosas que después hay que deshacer.

### Flujo

1. **Specify** — vos (o el agente a partir de un pedido en
   lenguaje natural) escribe `specs/NNN-nombre-feature.md` usando el template
   de `specs/_template.md`. Se define qué debe pasar, no cómo se implementa.
   Cada criterio de aceptación lleva un ID (`AC-1`, `AC-2`, ...).
2. **Clarify** — antes de planear, el agente lista en "Preguntas / Supuestos"
   las ambigüedades del pedido y cómo se resolvieron (respondidas por
   vos, o asumidas con motivo explícito). No se avanza al plan
   con una ambigüedad relevante sin resolver de alguna de las dos formas.
3. **Plan** — el agente lee el spec y escribe la sección "Plan técnico": qué
   archivos toca, qué funciones/estructuras nuevas, qué tests hacen falta.
   Vos revisás el plan antes de que se escriba código.
   Con el plan escrito, `rust-mentor` completa la sección "Conceptos de
   Rust" del spec: qué vas a necesitar entender y en qué orden.
4. **Tasks** — el plan se descompone en una lista de tareas chicas y
   verificables (checklist al final del mismo archivo de spec).
5. **Implement** — el agente ejecuta las tareas una por una, marcando cada
   checkbox cuando está hecha y testeada. Después de cada tarea que
   introduce un concepto nuevo de Rust, `rust-mentor` recorre el diff con vos
   antes de pasar a la siguiente.
6. **Verify** — al cerrar la feature, cada `AC-N` se marca cumplido, o el
   spec queda `Reabierto (parcial)` con el motivo explícito — nunca
   `Verificado` con algún `AC-N` sin tildar. El checklist de "Definition of
   Done" del spec se revisa entero, no solo los AC. Antes de `pr-agent`,
   `rust-mentor` registra lo aprendido en `docs/rust-aprendizaje.md`.

`specs/[0-9]*.md` se valida automáticamente en cada PR que toca `specs/**`
con `.github/workflows/spec-lint.yml`: estructura de secciones y que
"Verificado" no tenga AC sin tildar (ver `docs/proteccion-de-ramas.md`).

Los specs implementados quedan en `specs/` como historial — no se borran.

### Convención de nombres

`specs/001-nombre-feature.md`, `specs/002-otra-feature.md`, etc.
Numeración secuencial.

### Contratos de funciones/módulos públicos

Cada función/módulo público documentado tiene su contrato (precondiciones,
postcondiciones, invariantes, qué no debe hacer) como doc-comment junto a su
firma/definición. `docs/arquitectura.md` tiene la vista de conjunto (mapa de
módulos, quién depende de quién) y linkea a cada contrato — no lo duplica.

**Regla de sincronización, obligatoria para cualquier agente que edite una
firma o cambie el comportamiento documentado:** actualizar el doc-comment de
contrato en el mismo commit que el cambio de código, y si el cambio afecta
el mapa de módulos, dependencias o quién posee qué estado, actualizar
`docs/arquitectura.md` también en ese commit. Un PR que cambia una firma o
un comportamiento documentado sin tocar su contrato está incompleto —
`pr-agent` lo señala como falta antes de pedir revisión.

## Reglas para cualquier agente en este repo

- No modificar código fuera del alcance del spec activo
- No commitear secretos ni credenciales (`.env`, cache de tokens OAuth)
- Todo umbral o constante de configuración va en su lugar de config, nunca
  hardcodeado en la lógica
- Un spec sin criterios de aceptación no se implementa — se devuelve para
  completar primero
- Todo cambio de firma o comportamiento de una función/módulo documentado
  actualiza su contrato y, si aplica, `docs/arquitectura.md`, en el mismo
  commit
- Todo commit de Rust pasa `cargo fmt --check`, `cargo clippy -- -D warnings`
  y `cargo test`
- Nada de `unwrap()`/`expect()` en lógica de producción salvo invariantes
  documentadas; los errores se propagan con `Result` y `?`
- Bajo consumo es un requisito, no un extra: nada de polling agresivo, ni
  dependencias pesadas sin justificarlas en `docs/decisiones.md`
