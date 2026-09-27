# Protección de ramas y PRs

## Reglas
- `main` no reciben commits ni pushes directos.
- Todo entra por PR desde `feature/*` a `main`.
- Solo vos aprueba y mergea. Los agentes abren el PR y piden revisión, nunca mergean.

## Local (ya activo tras `git config core.hooksPath .githooks`)
- `pre-commit`: bloquea commits en ramas protegidas y de archivos de secretos.
- `pre-push`: bloquea push a ramas protegidas.
- Cada clon nuevo debe correr `git config core.hooksPath .githooks`.

## GitHub: ruleset `main` (activo desde 2026-09-27)
Los hooks locales se pueden saltear; la protección real está en GitHub.
Settings → Rules → Rulesets → ruleset `main` (Active), target: la rama
por defecto. Reglas:
- Restrict deletions
- Block force pushes
- Require a pull request before merging, con **0 approvals** (ver nota)
- Require status checks to pass: `version` (GitHub Actions), el de
  `version-check.yml`. Como solo corre en PRs, también bloquea cualquier
  push directo a `main`.

Para crearlo de nuevo (otro repo o si se borra): Settings → Rules →
Rulesets → New ruleset → New branch ruleset → Enforcement: Active →
Target branches: Add target → Include default branch → tildar las reglas
de arriba → Create. El check `version` solo aparece en "Add checks"
después de haber corrido una vez en algún PR.

`lint` (spec-lint) **no** es requerido a propósito: solo corre en PRs que
tocan `specs/**` y en los demás el PR quedaría esperándolo.

Con un solo dueño, GitHub no deja aprobar tu propio PR si el autor sos vos,
y los PRs los abre el agente con tu cuenta: por eso approvals en 0, y la
revisión es que vos mergeás a mano.

## Notificaciones de PRs por Telegram
El workflow `.github/workflows/notificar-pr.yml` avisa por Telegram cuando se
abre o reabre un PR, o pasa de borrador a listo. El botón del mensaje abre la
pestaña de archivos del PR, donde se aprueba y mergea a mano.

Configuración (una vez):
1. Crear un bot con @BotFather (`/newbot`) y guardar el token.
2. Escribirle un mensaje al bot y leer el `chat.id` en
   `https://api.telegram.org/bot<TOKEN>/getUpdates`.
3. Settings → Secrets and variables → Actions → **Repository secrets**:
   `TELEGRAM_BOT_TOKEN` y `TELEGRAM_CHAT_ID`.

Prueba manual: pestaña Actions → "Notificar PR por Telegram" → Run workflow.
Si los secrets faltan, el workflow avisa con un warning y no falla el PR.
No es un check requerido, así que si Telegram falla no bloquea el merge.

## Lint de specs
El workflow `.github/workflows/spec-lint.yml` corre en cada PR que toca
`specs/**`. Valida que todo spec tenga las secciones que exige
`specs/_template.md`, y que un spec en estado `Verificado` no tenga ningún
criterio de aceptación sin tildar. No es un check requerido por defecto.

## Versión y Release por spec
`.github/workflows/version-check.yml` corre en cada PR a `main`: si el PR
cambia lo que se distribuye (`src/`, `Cargo.toml`, `Cargo.lock`, `.cargo/`,
`dist/`), exige versión nueva en `Cargo.toml` y `Cargo.lock`, que no esté
publicada y su sección `## [X.Y.Z]` en `CHANGELOG.md`. Es check requerido
en el ruleset `main` (ver arriba).

Al mergear, `.github/workflows/release.yml` publica el Release `vX.Y.Z` si
esa versión todavía no tiene tag. El tag lo crea el workflow con
`GITHUB_TOKEN` (job `release`, único con `contents: write`); no hace falta
permiso extra ni push de tags a mano. Ver "Versiones y releases" en
`CLAUDE.md`.
