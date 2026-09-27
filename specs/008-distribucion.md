# 008 - Distribución: que cualquiera con Premium lo pueda usar

## Estado
En implementación

## Contexto
Hoy la app solo la puede usar alguien que clone el repo, instale rustup y
las Build Tools de Visual Studio, compile, cree una app en el Spotify
Developer Dashboard y escriba su Client ID en un `.env` dentro de la carpeta
del repo (`Config::load` lo busca en la carpeta desde donde arranca el
programa; el acceso directo de spec 004 arranca ahí a propósito).

Se quiere que funcione en cualquier PC y con cualquier persona que tenga
una cuenta de Spotify.

**Límite externo que define el alcance:** desde febrero de 2026, una app
del Dashboard en modo desarrollo admite como máximo 5 usuarios, cargados a
mano en *User Management*, y deja de funcionar para todos si su dueño pierde
Premium
([guía de migración](https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide),
[cuotas jul 2026](https://developer.spotify.com/blog/2026-07-23-web-api-quota-updates)).
El *Extended Quota Mode* pide empresa registrada y 250.000 usuarios activos
por mes. Por eso **no se distribuye un Client ID compartido**: cada usuario
usa el suyo (camino A). Como ya hace falta Premium para reproducir, esto
no le agrega ningún requisito de cuenta.

## Preguntas / Supuestos

- **¿Client ID compartido o uno por usuario?** → Respondido por vos:
  uno por usuario (camino A). El Client ID compartido (camino B) queda
  fuera de este spec.
- **¿Qué sistemas operativos?** → Asumido: solo Windows 10/11 x64. Es lo
  único probado, y la app de escritorio usa APIs de Windows
  (`RegisterHotKey`, cuadro de error nativo, ícono por `winresource`).
  macOS y Linux necesitarían un spec aparte.
- **¿Cómo se distribuye?** → Asumido: un `.zip` portable con los dos
  `.exe`, un `LEEME.txt` y el script de accesos directos, publicado en
  GitHub Releases por un workflow que se dispara al pushear un tag `v*`.
  Sin instalador (MSI/MSIX) ni autoupdate: más mantenimiento que valor
  para un proyecto chico.
- **El repo era privado: los Releases no los podía bajar nadie más.** →
  Respondido por vos: el repo se hizo público (2026-09-27) y el zip se
  publica en sus GitHub Releases. Antes se revisó el historial completo:
  no hay `.env`, tokens ni secretos commiteados. Los secretos de Telegram
  viven en GitHub Secrets, y `notificar-pr.yml` usa `pull_request` (no
  `pull_request_target`), así que un PR desde un fork no los ve.
- **¿Dónde se guarda el Client ID?** → Asumido: en
  `%APPDATA%\spotify-terminal\client-id.txt`, junto a los tokens y
  `ajustes.json`. En un archivo propio y no en `ajustes.json`, porque spec 007
  lo dejó fuera de los ajustes a propósito (cambiarlo no personaliza la app,
  cambia de cuenta de desarrollador) y porque `ajustes.json` solo guarda lo
  que difiere de fábrica. El Client ID no es secreto (con PKCE no hay client
  secret), así que va en texto plano.
- **¿Se sigue aceptando `.env` / variable de entorno?** → Asumido: sí, y
  tienen prioridad sobre el archivo. Orden: variable de entorno, `.env`,
  `client-id.txt`. Así el flujo de desarrollo actual no cambia.
- **¿Cómo se pide el Client ID?** → Asumido: la primera vez que haga falta
  (cualquier comando que use la Web API, o al abrir la app de escritorio), en
  vez del error `MissingClientId` se muestra una guía paso a paso y se pide
  pegar el Client ID. En la CLI se pide por la terminal y en la app de
  escritorio por su consola. Un comando nuevo `setup` repite la guía y deja
  cambiarlo.
- **¿Cómo se valida?** → Asumido: el formato se valida al pegarlo (32
  caracteres hexadecimales, sin espacios alrededor), antes de guardar. Si
  el ID tiene buen formato pero no existe, se detecta recién en el login
  (Spotify responde `INVALID_CLIENT`). En ese caso el mensaje dice que el
  Client ID es incorrecto y sugiere `setup`, no el mensaje genérico
  actual de `AppError::Login`.
- **¿Qué pasa si alguien usa el Client ID de otra persona?** → Asumido: no
  se impide (con camino B funcionaría para 5 personas), pero si la Web API
  responde 403 porque el usuario no está en *User Management*, el mensaje lo
  explica y sugiere crear su propia app con `setup`.
- **¿El dueño de la app tiene que agregarse a sí mismo en User
  Management?** → Asumido: no (el dueño tiene acceso de entrada). **Verificar
  en T1** con una cuenta que nunca haya usado el Dashboard. Si hace falta, se
  agrega como paso a la guía.
- **¿Runtime de C++?** → Asumido: los `.exe` se enlazan con el CRT
  estático (`-C target-feature=+crt-static`), para no depender de
  `vcruntime140.dll`. Se configura en `.cargo/config.toml` para el target
  MSVC, así también aplica al compilar en local.
- **Aviso de SmartScreen ("Windows protegió su PC")** → Asumido: los `.exe`
  no se firman (un certificado de firma cuesta plata todos los años). El
  `LEEME.txt` explica cómo pasar el aviso ("Más información" → "Ejecutar de
  todas formas").
- **¿Accesos directos desde el `.zip`?** → Asumido: el script se adapta
  para que, si está junto al `.exe` (carpeta del zip), use esa carpeta en
  vez de la del repo. Con el Client ID en `%APPDATA%`, el acceso directo ya
  no necesita arrancar en ninguna carpeta en particular.
- **¿Versión visible?** → Asumido: `spotify-terminal --version` y el
  comando `version` en la app de escritorio muestran la versión de
  `Cargo.toml`. El tag del Release tiene que coincidir con ella (el workflow
  falla si no). Sirve para saber qué versión tiene alguien que reporta un
  problema.
- **¿Puerto 8898 ocupado?** → Ya resuelto: `AppError::LoginPortBusy` existe
  y lo explica. El puerto no puede ser dinámico porque la Redirect URI tiene
  que coincidir exacta con la registrada en el Dashboard. No cambia.
- **¿Dónde se verifica "PC limpia"?** → **Pendiente, lo decidís antes de
  verificar AC-9** (no bloquea el plan ni la implementación): tu PC
  tiene Windows 10 Home, que no trae Windows Sandbox. Opciones: otra PC, una
  VM (Hyper-V no está en Home; VirtualBox sí), o probar en tu PC desde una
  cuenta de usuario de Windows nueva. Esto último no prueba la ausencia de
  Rust ni del runtime de C++.

Agregados al escribir el plan (a confirmar al revisarlo):

- **Un Client ID inexistente, ¿cómo se detecta sin colgar el login?** →
  Asumido: con un Client ID que Spotify no conoce (o con la Redirect URI mal
  registrada), la página de autorización muestra `INVALID_CLIENT` en el
  navegador y **nunca redirige** al callback, así que hoy la app se queda
  esperando para siempre. Antes de abrir el navegador en un login
  interactivo de la Web API, se hace un chequeo previo contra Spotify y, si
  el Client ID no existe, se corta con el error de AC-6. Un pedido más solo
  en el login interactivo, nunca en el refresh.
  **Corregido en T1:** `GET /authorize` sin sesión en el navegador responde
  303 al login para cualquier Client ID, así que no sirve. Sí sirve
  `POST /api/token` con un código falso: Client ID inexistente →
  `invalid_client`, válido → `invalid_grant` (evidencia en Notas de
  verificación). La Redirect URI mal registrada **no** se puede detectar
  antes del login (el canje responde `invalid_grant` igual); ese caso sigue
  como hoy y la guía insiste en copiar la URI exacta.
- **`setup` con `SPOTIFY_CLIENT_ID` en la variable de entorno o `.env`** →
  Asumido: guarda igual en `client-id.txt`, pero avisa que la variable /
  `.env` tiene prioridad y que el nuevo no se va a usar mientras exista.
  Los tokens se borran solo si cambia el Client ID *efectivo* (así un
  `setup` que no cambia nada no obliga a volver a loguearse).
- **`logout` sin Client ID configurado** → Asumido: funciona igual (solo
  borra los caches de `%APPDATA%`), sin mostrar la guía. Hoy falla con
  `MissingClientId` porque carga toda la configuración.
- **Política de ejecución de PowerShell** → Asumido: en Windows 10/11
  un `.ps1` bajado de internet no corre con doble clic (política
  `Restricted` / Mark of the Web). El zip lleva además
  `crear-accesos-directos.cmd`, que llama al `.ps1` con
  `-ExecutionPolicy Bypass` solo para ese proceso. El `.ps1` sigue siendo
  el mismo que usa el repo.
- **Probar el workflow sin publicar** → Asumido: además del disparo por
  tag, el workflow acepta `workflow_dispatch`, que arma el zip y lo deja
  como artifact de la corrida, sin crear Release. Sirve para revisar el
  contenido del zip y AC-10 antes del primer tag.

## Qué debe pasar (no cómo)

Una persona con Windows y Spotify Premium, sin herramientas de desarrollo:

1. Baja un `.zip`, lo descomprime donde quiera y abre `spotify-desktop.exe`
   (o `spotify-terminal.exe` desde una terminal).
2. Si todavía no configuró un Client ID, la app le explica en pocos pasos
   cómo crear su app en el Developer Dashboard: el link, qué nombre y
   descripción poner (cualquiera), la Redirect URI exacta para copiar,
   que marque "Web API" y dónde está el Client ID. Después le pide que lo
   pegue.
3. La app lo valida, lo guarda y sigue directo con el login de siempre (dos
   autorizaciones en el navegador). Desde ahí se usa igual que hoy.
4. No tiene que tocar ningún archivo a mano ni abrir la app desde una
   carpeta en particular.
5. Si se equivocó de Client ID, o usó el de otra persona sin estar
   habilitado, el mensaje dice exactamente eso y cómo arreglarlo (`setup`).

Para quien desarrolla, nada cambia: `.env` y `cargo build` siguen
funcionando igual.

## Criterios de aceptación

- [ ] **AC-1** — Sin variable de entorno, sin `.env` y sin `client-id.txt`,
      cualquier comando de la CLI que use la Web API y la apertura de la app
      de escritorio muestran la guía de configuración (link al Dashboard,
      Redirect URI `http://127.0.0.1:8898/login` y pasos) y piden el Client
      ID, en vez de terminar con el error `MissingClientId`.
- [x] **AC-2** — Un Client ID con formato inválido (largo distinto de 32,
      caracteres no hexadecimales, vacío) se rechaza con un mensaje que dice
      por qué, y se vuelve a pedir. No se guarda nada.
- [ ] **AC-3** — Un Client ID válido se guarda en
      `%APPDATA%\spotify-terminal\client-id.txt` (sin espacios alrededor) y
      la app sigue con el login sin reiniciar.
- [x] **AC-4** — Prioridad: variable de entorno, después `.env`, después
      `client-id.txt`. Con `.env` presente, el comportamiento es idéntico al
      de hoy (tests de resolución con las tres fuentes).
- [ ] **AC-5** — El comando `setup` (CLI y app de escritorio) muestra la
      guía, pide un Client ID nuevo y lo guarda. Si cambió, borra los tokens
      cacheados de la Web API (eran de otro Client ID) y avisa que hay que
      hacer `login`. En la CLI, cancelar con Ctrl+C / `q`, o en la app de
      escritorio con Esc, deja el Client ID anterior intacto.
- [ ] **AC-6** — Un Client ID con buen formato que Spotify no reconoce
      termina el login con un mensaje que dice que el Client ID es
      incorrecto y sugiere `setup`.
- [ ] **AC-7** — Si la Web API responde 403 porque el usuario no está
      habilitado en la app del Dashboard, el mensaje lo explica y sugiere
      crear una app propia con `setup`.
- [ ] **AC-8** — Pushear un tag `vX.Y.Z` que coincide con la versión de
      `Cargo.toml` genera en CI un `spotify-terminal-vX.Y.Z-windows-x64.zip`
      con `spotify-terminal.exe`, `spotify-desktop.exe`, `LEEME.txt` y el
      script de accesos directos, y lo publica en el GitHub Release de ese
      tag. Con tag y versión distintos, el workflow falla.
- [ ] **AC-9** — En una PC con Windows sin Rust, sin Build Tools de Visual
      Studio, sin el repo y sin configuración previa: se descomprime el zip,
      se abre `spotify-desktop.exe`, se sigue la guía con una cuenta Premium
      que nunca usó el Dashboard, se hace login y `play <tema>` suena.
- [x] **AC-10** — Los `.exe` de release no dependen de `vcruntime140.dll`
      ni de otras DLL del runtime de C++ (`dumpbin /dependents`, o el
      equivalente).
- [ ] **AC-11** — El script de accesos directos, corrido desde la carpeta
      del zip, crea accesos directos que abren la app, y la app encuentra el
      Client ID sin importar la carpeta de inicio. Corrido desde el repo,
      funciona como hoy.
- [x] **AC-12** — `spotify-terminal --version` y `version` en la app de
      escritorio muestran la versión de `Cargo.toml`.
- [x] **AC-13** — `LEEME.txt` cubre: requisitos (Windows 10/11 x64,
      Premium), aviso de SmartScreen, configuración del Dashboard paso a
      paso, dónde quedan los datos (`%APPDATA%\spotify-terminal`) y cómo
      borrarlos. El README separa "Usar" (el zip) de "Desarrollar"
      (compilar).

## Riesgos / casos de falla

- **Spotify cambia de nuevo las reglas del modo desarrollo** (ya pasó en
  nov 2024 y en feb y jul 2026): la guía de configuración y el `LEEME.txt`
  pueden quedar desactualizados. Los pasos del Dashboard van en un único
  lugar de config/texto, para corregirlos sin buscarlos por todo el código.
- **El audio depende del Client ID de librespot** (`docs/decisiones.md`), que
  es no oficial: si Spotify lo bloquea, deja de sonar para todos y este spec
  no puede evitarlo. Se menciona en `LEEME.txt` como limitación conocida.
- **Se corta a mitad de guardar `client-id.txt`**: el guardado es atómico
  (archivo temporal + rename, como `ajustes.json` en spec 007). Nunca queda
  un archivo a medio escribir que después se lea como Client ID.
- **Un `client-id.txt` corrupto o editado a mano con basura**: se trata como
  si no hubiera Client ID. Se muestra la guía con un aviso de que el
  guardado era inválido, sin cerrar la app.
- **Datos que salen de la PC**: ninguno nuevo. El Client ID solo viaja a
  Spotify en el login, igual que hoy.
- **Binario publicado con algo que no debería**: el workflow arma el zip
  desde una lista explícita de archivos, nunca desde una carpeta entera.
  Así no puede meterse un `.env` ni un token.

## Plan técnico

### Archivos que toca

| Archivo | Cambio | Agente |
|---|---|---|
| `src/config.rs` | Resolución del Client ID en 3 fuentes, constantes nuevas (archivo, largo, URL del Dashboard, texto de la guía, versión) | spotify-api-agent |
| `src/setup.rs` (nuevo) | Validar, leer, guardar (atómico) y aplicar un Client ID; lógica compartida por CLI y escritorio | spotify-api-agent |
| `src/error.rs` | `MissingClientId` sugiere `setup`; variantes nuevas `InvalidClientId`, `UserNotAllowed` | spotify-api-agent |
| `src/spotify/auth.rs` | Chequeo previo del Client ID contra `/api/token`; `invalid_client` en el canje → `InvalidClientId`; `logout` recibe la carpeta de datos | spotify-api-agent |
| `src/spotify/web.rs` | 403 de usuario no habilitado → `UserNotAllowed` (lee el cuerpo del error) | spotify-api-agent |
| `src/ui/cli.rs`, `src/main.rs` | Comandos `setup` y `version` / `--version` / `-V`; guía interactiva por terminal; `Config::load` recién en los comandos que usan la Web API | tui-agent |
| `src/app/shell.rs`, `src/app/engine.rs`, `src/app/backend.rs` | Comandos `setup` / `version`; modo "pidiendo Client ID" del motor; guía al arrancar sin Client ID | tui-agent |
| `src/desktop/app.rs` | Prompt nuevo (`Client ID ›`, Esc cancela) | tui-agent |
| `.cargo/config.toml` (nuevo) | `+crt-static` para `x86_64-pc-windows-msvc` | spotify-api-agent |
| `scripts/instalar-acceso-directo.ps1` | Modo zip: si hay un `spotify-desktop.exe` al lado, usa esa carpeta | tui-agent |
| `dist/crear-accesos-directos.cmd` (nuevo) | Llama al `.ps1` con `-ExecutionPolicy Bypass` | tui-agent |
| `dist/LEEME.txt` (nuevo) | Guía de uso para el zip (AC-13) | docs-agent |
| `.github/workflows/release.yml` (nuevo) | Build, chequeos, zip y Release | pr-agent |
| `scripts/armar-zip.ps1`, `scripts/verificar-dependencias.ps1` (nuevos, agregados al implementar) | Zip desde lista explícita y chequeo de DLLs; los usa el workflow y se pueden correr en local | pr-agent |
| `.gitattributes` | `*.cmd` y `dist/LEEME.txt` en CRLF | pr-agent |
| `README.md`, `CHANGELOG.md`, `docs/decisiones.md`, `docs/arquitectura.md`, `docs/glosario.md` | Ver "Contratos y docs" | docs-agent |
| `tests/` | Ver "Tests necesarios" | testing-agent |

### Diseño

**Resolución del Client ID (`config.rs`).** `Config` suma
`client_id_source: ClientIdSource` (`Env` | `SavedFile`). `Config::load`:

1. `dotenvy::dotenv()` como hoy (no pisa variables ya definidas, así que la
   variable de entorno ya gana sobre `.env`).
2. Si `SPOTIFY_CLIENT_ID` está y no es vacío → se usa con la validación de
   hoy (recortado, no vacío; **sin** exigir formato, para que el flujo de
   desarrollo no cambie: AC-4).
3. Si no, `setup::read_saved(data_dir)` → `Valid(id)`, `Missing` o
   `Invalid`.
4. `Missing` → `Err(MissingClientId { saved_invalid: false })`;
   `Invalid` → `Err(MissingClientId { saved_invalid: true })`.

La lógica de prioridad va en una función pura
`resolve_client_id(env: Option<String>, saved: SavedClientId)` para
testear las tres fuentes sin tocar el entorno real.

Constantes nuevas en `config.rs` (único lugar, ver Riesgos):
`CLIENT_ID_FILE = "client-id.txt"`, `CLIENT_ID_LEN = 32`,
`DASHBOARD_URL`, `SETUP_GUIDE` (pasos numerados; interpola `DASHBOARD_URL`
y `REDIRECT_URI`), `VERSION = env!("CARGO_PKG_VERSION")`, y los textos que
identifican las respuestas de Spotify de T1 (`invalid_client`, cuerpo del
403 de usuario no habilitado).

**`src/setup.rs` (nuevo, `pub`).**

- `validate(raw: &str) -> Result<String, InvalidFormat>` — recorta; vacío,
  largo ≠ 32 o no hexadecimal → `InvalidFormat` con el motivo en texto
  (AC-2). Acepta mayúsculas y las pasa a minúscula.
- `read_saved(dir) -> SavedClientId` — lee `client-id.txt`; inexistente →
  `Missing`; ilegible o que no pasa `validate` → `Invalid`.
- `save(dir, id) -> io::Result<()>` — crea la carpeta, escribe
  `client-id.txt.tmp` y renombra (como `settings::save`).
- `apply(config_before: Option<&Config>, id) -> Result<Applied, AppError>` —
  guarda, recalcula el Client ID efectivo y, si cambió respecto del
  anterior, borra `WEB_TOKEN_FILE` (el de audio no: es del Client ID de
  librespot). `Applied { changed, overridden_by_env }` para que CLI y
  escritorio armen el mismo aviso.

**Errores nuevos (`error.rs`).** Cada uno termina sugiriendo `setup`:
`InvalidClientId` (AC-6), `UserNotAllowed` (AC-7:
explica *User Management* y sugiere crear una app propia).
`MissingClientId { saved_invalid }` ya no dice "copiá `.env.example`" sino
"corré `setup`" (queda para contextos no interactivos).

**Login (`auth.rs`).** Antes de `oauth_client(...).get_access_token()` en
el camino interactivo de `TokenKind::Web`: `check_client(config)` hace
`POST https://accounts.spotify.com/api/token` con
`grant_type=authorization_code`, un código y un verifier falsos, el
`client_id` y la `redirect_uri`, con `reqwest` y `HTTP_TIMEOUT`. La
clasificación va en una función pura `classify_client_check(status, body)`:
`invalid_client` → `InvalidClientId`; cualquier otra respuesta (lo normal
es `invalid_grant`) → `Ok`, para no bloquear un login válido si Spotify
cambia el texto. Sin red → `Network`
(el login tampoco andaría). Además `login_error` mapea `invalid_client` en
el canje de código a `InvalidClientId`, por si igual llega.
`logout(config)` pasa a `logout(data_dir: &Path)` (cambio de firma: se
actualiza su contrato y el trait `Backend`).

**403 (`web.rs`).** `status_error` recibe el cuerpo de la respuesta (hoy
solo el status). 403 cuyo cuerpo coincide con el texto de T1 →
`UserNotAllowed`; cualquier otro 403 sigue como hoy.

**CLI (`cli.rs`, `main.rs`).** `Command::Setup` y `Command::Version`
(`version`, `--version`, `-V`, imprime `spotify-terminal X.Y.Z`). `run()`
parsea primero; `help`, `version`, `logout` y `setup` no cargan el Client
ID. Los demás llaman a `config_or_setup()`: si `Config::load` da
`MissingClientId`, imprime la guía (y el aviso si el guardado era
inválido), pide el Client ID con `setup::prompt(reader, writer)` hasta que
sea válido, lo aplica y vuelve a cargar `Config` para seguir con el login
en el mismo proceso (AC-1, AC-3). `prompt` es genérica sobre
`BufRead`/`Write` para testearla; `q` o fin de entrada (Ctrl+C / Ctrl+Z)
cancela sin guardar (AC-5).

**Escritorio (`engine.rs`, `shell.rs`, `backend.rs`, `app.rs`).**

- `Backend` suma `fn client_id_status(&self) -> ClientIdStatus` y
  `fn set_client_id(&self, id: &str) -> Result<Applied, AppError>`, así los
  tests del motor usan el backend falso sin tocar `%APPDATA%`.
- `Prompt::ClientId`: el motor entra en ese modo al arrancar si
  `client_id_status` dice que falta (AC-1), o con `setup` (AC-5). Muestra
  `SETUP_GUIDE` como líneas de la consola. En ese modo cada línea se valida
  como Client ID (inválido → aviso con el motivo y sigue esperando; válido
  → `set_client_id` y vuelve a `Ready`, avisando si hay que hacer `login`
  o si `.env` lo tapa). Esc lo cancela sin guardar, igual que `Choose`.
  Escribir un comando conocido mientras se espera **no** se toma como
  comando (un Client ID pegado nunca lo es, y así un error de tipeo no
  sale del modo); solo Esc sale.
- `version` imprime la versión en la consola (AC-12).
- `app.rs`: texto del prompt (`Client ID ›`) y ayuda al pie
  ("pegá el Client ID y Enter · Esc cancela").

**CRT estático.** `.cargo/config.toml`:
`[target.x86_64-pc-windows-msvc] rustflags = ["-C", "target-feature=+crt-static"]`.
Las crates con C (vía `cc`) respetan el flag. Se chequea con `dumpbin
/dependents` en local (T8) y en el workflow (T11), que falla si aparece
`VCRUNTIME*`, `MSVCP*` o `api-ms-win-crt-*`.

**Accesos directos.** Si `$PSScriptRoot\spotify-desktop.exe` existe → modo
zip: `-Exe` por defecto es ese y `WorkingDirectory` es `$PSScriptRoot`.
Si no → modo repo, idéntico a hoy. `dist/crear-accesos-directos.cmd`:
`powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0instalar-acceso-directo.ps1"`
y `pause` para que se lea el resultado.

**Workflow `release.yml`.** `on: push: tags: ['v*']` y
`workflow_dispatch`. `windows-latest`, `permissions: contents: write` solo
en el job que publica. Pasos:

1. Checkout; toolchain estable; cache de `target/` (`Swatinem/rust-cache`).
   Actions de terceros fijadas por SHA.
2. Si es tag: `v` + `version` de `Cargo.toml` (`cargo metadata
   --no-deps`) tiene que ser igual al tag, si no falla (AC-8).
3. `cargo fmt --check`, `cargo clippy --release -- -D warnings`, `cargo
   test --release`, `cargo build --release --locked`.
4. `dumpbin /dependents` sobre los dos `.exe` (AC-10).
5. Carpeta de staging con **lista explícita**: los dos `.exe`,
   `dist/LEEME.txt`, `dist/crear-accesos-directos.cmd`,
   `scripts/instalar-acceso-directo.ps1` → `Compress-Archive` a
   `spotify-terminal-vX.Y.Z-windows-x64.zip` (con `workflow_dispatch`, el
   nombre usa el SHA corto).
6. Tag → `gh release create` con el zip y notas del `CHANGELOG`;
   `workflow_dispatch` → `upload-artifact`.

**LEEME / README.** `dist/LEEME.txt` (UTF-8, CRLF): requisitos, qué hay en
el zip, aviso de SmartScreen, primer uso y pasos del Dashboard, comandos
básicos, accesos directos, dónde quedan los datos y cómo borrarlos
(carpeta `%APPDATA%\spotify-terminal`), limitaciones conocidas (5
usuarios, Client ID de librespot). README: sección "Usar" (bajar el zip
de Releases) separada de "Desarrollar" (lo actual).

### Tests necesarios

- `config`: prioridad env > `.env` > archivo con las combinaciones de las
  tres fuentes; env presente ignora un archivo inválido (AC-4).
- `setup::validate`: vacío, 31/33 caracteres, no hex, espacios alrededor,
  mayúsculas (AC-2).
- `setup::save` / `read_saved`: guarda recortado, no deja `.tmp`, archivo
  basura o vacío → `Invalid` (AC-3, riesgos).
- `setup::apply`: Client ID efectivo cambia → borra `token-web.json` y
  conserva `token-audio.json`; igual → no borra nada; con env activo →
  `overridden_by_env` y no borra (AC-5).
- `setup::prompt` con `Cursor`: inválido y después válido guarda el
  válido; `q` y fin de entrada no guardan (AC-2, AC-5).
- `cli::parse` y `shell::parse_line`: `setup`, `version`, `--version`,
  `-V` (AC-12).
- Motor con backend falso: arranque sin Client ID muestra la guía y queda
  en `Prompt::ClientId`; inválido avisa y sigue en ese modo; válido llama
  a `set_client_id` y vuelve a `Ready`; Esc cancela sin llamar a
  `set_client_id` (AC-1, AC-2, AC-5).
- `auth::classify_client_check` y `login_error` con las respuestas reales que
  anote T1 (AC-6).
- `web::status_error`: 403 con el cuerpo de T1 → `UserNotAllowed`; otro 403
  → `Spotify` (AC-7).
- Consistencia de textos: `dist/LEEME.txt` (vía `include_str!`) contiene
  `REDIRECT_URI`, `DASHBOARD_URL` y `%APPDATA%\spotify-terminal`, así si
  cambia una constante el test marca el LEEME desactualizado (AC-13).
- Manuales (anotados en Notas de verificación): AC-8 (tag bueno y tag
  que no coincide), AC-9 (PC limpia), AC-10 (`dumpbin`), AC-11 (script
  desde el zip y desde el repo).

### Contratos y docs

- Contratos a crear: todo `setup::*` público, `resolve_client_id`,
  `check_client` / `classify_client_check`, `Backend::client_id_status` /
  `set_client_id`.
- Contratos a actualizar: `Config::load` (nuevas fuentes y error),
  `auth::logout` (firma), `Backend` (logout y métodos nuevos),
  `status_error`, `cli::parse`, `shell::parse_line`.
- `docs/arquitectura.md`: módulo `setup` en el mapa (lo usan `main` y
  `app::backend`; depende de `config`), `client-id.txt` en "Quién posee
  qué estado", y una sección corta de distribución (workflow → zip).
- `docs/decisiones.md`: camino A vs. B, zip sin instalador, CRT estático,
  sin firma de código, `client-id.txt` fuera de `ajustes.json`, chequeo
  previo del Client ID contra `/api/token`.
- `docs/glosario.md`: Client ID, *User Management*, modo desarrollo.

## Tareas

- [ ] T1 — Verificar con una cuenta nueva si el dueño de una app del
      Dashboard necesita agregarse en User Management, y qué responde la Web
      API (código y cuerpo) para un Client ID inexistente y para un usuario
      no habilitado. Anotar la evidencia acá. Sumar: qué devuelve
      `GET /authorize` (status y cuerpo) con Client ID inexistente, con
      Redirect URI no registrada y con todo bien; y si el navegador redirige
      o no al callback en los dos casos de error. *(vos + spotify-api-agent)*
      - [x] T1a — Respuestas de `/authorize` y `/api/token` a un Client ID
            inexistente (ver Notas de verificación).
      - [ ] T1b — Con tu cuenta: ¿el dueño necesita agregarse en User
            Management? (crear una app nueva sin agregarse y hacer login).
      - [ ] T1c — Con una segunda cuenta Premium no cargada en User
            Management: status y cuerpo de `GET /me` (AC-7).
- [x] T2 — `config.rs` + `setup.rs`: constantes, `resolve_client_id`,
      `validate`, `read_saved`, `save`, `apply`, `prompt`; errores nuevos en
      `error.rs`. Tests de AC-2, AC-3, AC-4, AC-5 (lógica). *(spotify-api-agent)*
- [x] T3 — `auth.rs`: `check_client` + `classify_client_check`,
      `invalid_client` en `login_error`, `logout(data_dir)`. Tests con las
      respuestas de T1 (AC-6). *(spotify-api-agent)*
- [x] T4 — `web.rs`: 403 de usuario no habilitado → `UserNotAllowed`.
      Test (AC-7). *(spotify-api-agent)*
- [x] T5 — CLI: `setup`, `version`/`--version`/`-V`, guía interactiva al
      faltar el Client ID, `Config::load` solo donde hace falta. Tests de
      parseo (AC-1, AC-5, AC-12 en la CLI). *(tui-agent)*
- [x] T6 — Escritorio: `Backend` extendido, `Prompt::ClientId`, guía al
      arrancar, `setup` y `version` en el shell, Esc cancela. Tests del
      motor (AC-1, AC-2, AC-3, AC-5, AC-12 en el escritorio). *(tui-agent)*
- [ ] T7 — Prueba real en local: sin `.env` ni variable, borrar
      `client-id.txt`, correr CLI y escritorio de punta a punta hasta que
      suene; Client ID inventado (AC-6); `setup` cambiando y sin cambiar.
      *(vos)*
- [x] T8 — `.cargo/config.toml` con CRT estático; `cargo build --release`
      y `dumpbin /dependents` sin DLL del runtime de C++. Medir que RAM y
      tamaño del `.exe` no cambien de forma notable (AC-10). *(spotify-api-agent)*
- [x] T9 — Script de accesos directos en modo zip +
      `dist/crear-accesos-directos.cmd`. Probar desde una carpeta tipo zip y
      desde el repo (AC-11). *(tui-agent)*
- [x] T10 — `dist/LEEME.txt`, README "Usar" / "Desarrollar", test de
      consistencia de textos (AC-13). *(docs-agent)*
- [ ] T11 — `.github/workflows/release.yml`; corrida con
      `workflow_dispatch` y revisión del zip (contenido exacto, sin `.env`
      ni tokens, AC-10 en CI). *(pr-agent)*
- [x] T12 — Docs: contratos, `arquitectura.md`, `decisiones.md`,
      `glosario.md`, `CHANGELOG.md`. *(docs-agent)*
- [ ] T13 — Release real: tag que no coincide con `Cargo.toml` (tiene que
      fallar) y después `v0.1.0` (AC-8). *(vos)*
- [ ] T14 — AC-9 en la PC limpia que elijas (pendiente de la pregunta de
      arriba) con una cuenta Premium que nunca usó el Dashboard. *(vos)*

## Definition of Done

- [ ] Todos los AC tildados, o el estado es `Reabierto (parcial)` con el
      motivo explícito
- [ ] Tests corren y pasan
- [ ] Contratos de funciones públicas y doc de arquitectura actualizados si
      el spec cambió una firma, comportamiento o el mapa de módulos
- [ ] Decisiones de diseño relevantes documentadas (camino A vs. B, zip sin
      instalador, CRT estático, sin firma de código)
- [ ] Changelog actualizado
- [ ] Sin constantes/umbrales hardcodeados fuera de su lugar de config
- [ ] Sin secretos ni credenciales en el diff ni en el zip
- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test` pasan

## Notas de verificación

**T1a (2026-09-27, con `curl`, sin sesión de Spotify en el cliente HTTP):**

- `GET /authorize?client_id=…&response_type=code&redirect_uri=…&code_challenge…`
  → `303` a `accounts.spotify.com/es/login?continue=…` para el Client ID
  válido, para uno inexistente (`0123…cdef`), para uno válido con Redirect
  URI no registrada y para `zzz`. No distingue nada antes del login.
- `POST /api/token` `grant_type=authorization_code`, `code=codigofalso`:
  - Client ID válido → `400 {"error":"invalid_grant","error_description":"Invalid authorization code"}`
  - Client ID inexistente → `400 {"error":"invalid_client","error_description":"Failed to get client"}`
  - Client ID válido + Redirect URI no registrada → `400 invalid_grant` (igual
    que el válido: no se detecta).
- `POST /api/token` `grant_type=refresh_token` con Client ID inexistente →
  `400 invalid_grant` "Invalid refresh token" (el refresh no sirve para
  detectarlo).

**Implementación (2026-09-27):**

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y
  `cargo test` (161 tests) pasan.
- AC-2: `setup::tests::validate_rechaza_con_motivo`,
  `prompt_insiste_hasta_uno_valido`, y en el motor
  `client_id_invalido_insiste_sin_guardar`.
- AC-4: `config::tests::env_gana_sobre_el_archivo_y_se_recorta`,
  `sin_env_se_usa_el_archivo`, `sin_ninguno_es_missing_client_id`. `.env`
  entra por la misma variable (dotenvy no pisa una variable ya definida),
  así que con `.env` el valor usado es el mismo que antes, sin validar
  formato.
- AC-10 (local): `scripts/verificar-dependencias.ps1` sobre los `.exe`
  sin `+crt-static` lista `VCRUNTIME140.dll` y 5-6 `api-ms-win-crt-*`
  y falla; con `+crt-static` quedan solo DLL de Windows (kernel32,
  user32, bcrypt, ws2_32, gdi32...) y pasa. Tamaño: desktop 9.079.808 →
  9.290.240 bytes, terminal 4.624.896 → 4.820.480. Falta verlo en CI
  (T11).
- AC-11 (parcial): desde una carpeta con el zip descomprimido, el acceso
  directo apunta al `.exe` de esa carpeta y arranca ahí; desde el repo,
  a `target\release` y arranca en el repo (igual que antes). Falta abrir
  la app desde el acceso directo sin `.env` (T7).
- AC-12: `spotify-terminal --version` imprime `spotify-terminal 0.1.0`;
  `cli::tests::comandos_validos` y
  `engine::tests::version_muestra_la_de_cargo`.
- AC-13: `dist/LEEME.txt` y README con "Usar" / "Desarrollar";
  `config::tests::leeme_coincide_con_la_config` chequea que el LEEME
  tenga el link del Dashboard, la Redirect URI, la carpeta de datos, `setup`
  y el paso de SmartScreen.
- CLI a mano, desde una carpeta sin `.env`: `setup` con `hola` y después
  `q` rechaza el formato, cancela y no crea `client-id.txt`; `whoami`
  muestra la guía y, cancelado, termina con el mensaje que sugiere `setup`.
- Zip local (`scripts/armar-zip.ps1 -Version v0.1.0-local`): contiene
  exactamente los 5 archivos de la lista.
- T4: el texto del 403 (`USER_NOT_REGISTERED_MESSAGE`) es el que reporta
  la comunidad; se confirma en T1c antes de tildar AC-7.
