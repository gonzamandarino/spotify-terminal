# Protección de ramas y PRs

## Reglas
- `main` no reciben commits ni pushes directos.
- Todo entra por PR desde `feature/*` a `main`.
- Solo vos aprueba y mergea. Los agentes abren el PR y piden revisión, nunca mergean.

## Local (ya activo tras `git config core.hooksPath .githooks`)
- `pre-commit`: bloquea commits en ramas protegidas y de archivos de secretos.
- `pre-push`: bloquea push a ramas protegidas.
- Cada clon nuevo debe correr `git config core.hooksPath .githooks`.

## GitHub (a configurar cuando exista el remoto)
Los hooks locales se pueden saltear; la protección real está en GitHub.
Settings → Branches → Add rule, para cada rama protegida:
- Require a pull request before merging
- Require approvals: 1 (o 0 si el único dueño no puede autoaprobar su propio PR — ver nota abajo)
- Require review from Code Owners (usa `.github/CODEOWNERS`)
- Dismiss stale approvals when new commits are pushed
- Do not allow bypassing the above settings
- Restrict force pushes y deletions

Con un solo dueño, GitHub no deja aprobar tu propio PR si el autor sos vos.
Si los PRs los abre el agente con tu cuenta, esto es un problema: opciones
(a) usar un token/cuenta distinta para el agente, o (b) dejar approvals en 0
y confiar en "Require a pull request" + que mergees vos a mano.

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
publicada y su sección `## [X.Y.Z]` en `CHANGELOG.md`. Conviene marcarlo
como check requerido (Settings → Branches → Require status checks →
"Chequeo de versión / version").

Al mergear, `.github/workflows/release.yml` publica el Release `vX.Y.Z` si
esa versión todavía no tiene tag. El tag lo crea el workflow con
`GITHUB_TOKEN` (job `release`, único con `contents: write`); no hace falta
permiso extra ni push de tags a mano. Ver "Versiones y releases" en
`CLAUDE.md`.
