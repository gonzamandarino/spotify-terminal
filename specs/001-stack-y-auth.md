# 001 - Stack, reproductor de audio y login con Spotify

## Estado
`Draft`

## Contexto
Es la base de todo el proyecto: sin lenguaje elegido, sin forma de emitir
audio y sin sesión autenticada contra Spotify no se puede implementar
ninguna de las funciones objetivo (playlists, likes, shuffle, elegir canción,
generador de playlists). Este spec fija esas tres decisiones y entrega lo
mínimo que las prueba de punta a punta: iniciar sesión una vez, reabrir el
cliente sin volver a loguearse, y reproducir/pausar un tema desde la
terminal.

El objetivo del proyecto es consumir menos recursos que la app oficial, así
que el consumo se mide desde este primer spec y no se deja "para después".

Referencia: ya existen clientes de terminal (`ncspot`, `spotify-player`,
ambos en Rust sobre `librespot`). No se reutilizan como dependencia directa,
pero sirven para comparar consumo y enfoques.

## Preguntas / Supuestos

- **¿Tenés Spotify Premium?** → *Pendiente de respuesta.* Tanto controlar la
  reproducción vía Web API como usar `librespot` requieren Premium. Si la
  respuesta es no, el proyecto entero cambia de alcance.
- **¿Qué emite el audio?** → *Propuesta (pendiente de aprobación):* reproductor
  embebido basado en `librespot` dentro del mismo proceso, para que el
  cliente sea un único programa liviano y no dependa de tener otra app de
  Spotify abierta. Alternativas: (a) controlar un dispositivo Spotify Connect
  ya existente (celular, otra PC) — más simple, pero no reemplaza a la app;
  (b) `spotifyd` como proceso aparte — dos procesos que mantener.
- **¿Lenguaje?** → *Propuesta (pendiente de aprobación):* **Rust**, porque
  `librespot` es Rust (embebible sin puente), da binario único y el menor
  consumo de memoria. Alternativas: Go (`go-librespot`, más simple de
  escribir, consumo algo mayor), Python (lo más rápido de escribir, pero
  necesita un reproductor externo y consume más). Decisión final va a
  `docs/decisiones.md`.
- **¿Presupuesto de recursos?** → *Asumido (ajustable):* ≤ 60 MB de RAM
  (RSS) reproduciendo y < 2 % de CPU promedio en reposo con un tema sonando,
  medido en esta PC (Windows 10). Motivo: la app oficial suele usar varios
  cientos de MB; esto es un orden de magnitud menos y es alcanzable con los
  clientes de referencia.
- **¿Qué plataforma?** → *Asumido:* Windows 10 primero (es la máquina de
  desarrollo). Linux/macOS no se prueban en este spec.
- **¿Credenciales de la app de Spotify?** → *Asumido:* vos creás una app en
  el Spotify Developer Dashboard y el `Client ID` se lee de `.env`
  (se commitea `.env.example` sin valores). Flujo OAuth: Authorization Code
  + PKCE, sin client secret.

## Qué debe pasar (no cómo)
1. La primera vez que se ejecuta el cliente, si no hay sesión guardada, abre
   el navegador (o muestra la URL) para autorizar con Spotify, recibe el
   callback en localhost y guarda el token localmente.
2. Las siguientes ejecuciones usan el token guardado y lo renuevan solo
   cuando vence, sin pedir login de nuevo.
3. Ya autenticado, el cliente muestra en la terminal el nombre del usuario
   y permite reproducir un tema dado (por URI o ID de Spotify), pausarlo y
   reanudarlo, con el audio saliendo por esta PC.
4. Existe un comando para cerrar sesión que borra el token guardado.
5. Si falta el `Client ID` o la cuenta no es Premium, el cliente sale con un
   mensaje claro que dice qué hacer, sin stack trace.

## Criterios de aceptación

- [ ] **AC-1** — Con cache de token vacío, el cliente completa el login
      OAuth PKCE y crea el archivo de cache de token (ruta fuera del repo o
      ignorada por `.gitignore`).
- [ ] **AC-2** — Con cache válido, una segunda ejecución no pide login y
      muestra el nombre del usuario.
- [ ] **AC-3** — Con access token vencido y refresh token válido, el cliente
      lo renueva solo y sigue funcionando sin intervención.
- [ ] **AC-4** — Dado un URI de tema, el audio suena por esta PC; pausa y
      reanudación funcionan desde la terminal.
- [ ] **AC-5** — Consumo dentro del presupuesto: ≤ 60 MB RSS reproduciendo y
      < 2 % CPU promedio durante 5 minutos de reproducción sin interacción
      (medido y anotado en "Notas de verificación").
- [ ] **AC-6** — Sin `Client ID` configurado, el cliente sale con código
      distinto de 0 y un mensaje que indica cómo configurarlo.
- [ ] **AC-7** — El comando de logout borra el cache de token; la siguiente
      ejecución vuelve a pedir login.
- [ ] **AC-8** — Ni `.env` ni el cache de token aparecen en `git status`
      después de loguearse (verificación de `.gitignore` + hook).

## Riesgos / casos de falla

- **Envío de datos a terceros:** se autentica contra Spotify. Pedir solo los
  scopes necesarios para este spec (lectura de perfil + control de
  reproducción/streaming); los scopes de likes/playlists se agregan en sus
  specs.
- **Credenciales locales:** el refresh token da acceso a la cuenta. Se guarda
  solo en la máquina del usuario, nunca en logs ni en el repo.
- **Interrupción a mitad del login:** si se corta el callback, no debe quedar
  un cache corrupto; la próxima ejecución reintenta el login desde cero.
- **Spotify API caída o sin red de forma sostenida:** el cliente informa el
  error y sale (o reintenta con backoff acotado), sin loop de reintentos
  agresivo que consuma CPU/red.
- **Cambios de política de Spotify:** `librespot` no es oficial y Spotify
  puede romperlo; registrar la versión usada y la alternativa de respaldo
  (controlar un dispositivo Connect) en `docs/decisiones.md`.

## Plan técnico
*(lo completa el agente después de resolver las preguntas pendientes de
arriba; se revisa antes de escribir código)*

- Archivos que toca:
- Funciones/estructuras nuevas:
- Tests necesarios:
- Contratos a crear/actualizar y cambios en `docs/arquitectura.md`:

## Tareas
*(desglose del plan, se van tildando)*

- [ ] Resolver las preguntas pendientes y registrar decisiones en `docs/decisiones.md`
- [ ] Completar Plan técnico y hacerlo revisar

## Definition of Done

- [ ] Todos los AC tildados, o el estado es `Reabierto (parcial)` con el
      motivo explícito
- [ ] Tests corren y pasan
- [ ] Contratos de funciones públicas y doc de arquitectura actualizados si
      el spec cambió una firma, comportamiento o el mapa de módulos
- [ ] Decisiones de diseño relevantes documentadas (lenguaje, reproductor,
      flujo OAuth) en `docs/decisiones.md`
- [ ] Changelog actualizado
- [ ] Sin constantes/umbrales hardcodeados fuera de su lugar de config
- [ ] Sin secretos ni credenciales en el diff

## Notas de verificación
*(al cerrar: qué se probó y resultado — por AC cuando no sea obvio)*
