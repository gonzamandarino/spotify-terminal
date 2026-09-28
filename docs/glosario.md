# Glosario

Vocabulario del dominio, para que specs, código y docs usen el mismo término
con el mismo significado. Se agrega una entrada cuando un spec introduce un
concepto nuevo — no antes, no de antemano especulando.

- **Ajustes** — lo que el usuario personaliza en la app de escritorio
  desde la barra de menús (spec 007): tema, fuente, atajos, reproducción,
  consola y ventana. Se guardan en `ajustes.json`; los valores de
  `config.rs` son los de fábrica.
- **Visualización** — lo que se dibuja de la canción en el panel derecho
  de la app de escritorio (spec 010): onda, barras (espectro) o vinilo
  con la tapa del disco. Se elige en Personalización → Visualización.
- **Tamaño de letra** — el del texto de la consola, la línea de entrada
  y la barra "sonando" (spec 009). Los menús, diálogos y la barra de
  título tienen letra fija.
- **Atajo de ventana** — combinación que anda solo con la ventana de la
  app enfocada (ej. Ctrl+→). Ver también **atajo global**.
- **Atajo global** — combinación registrada en Windows que anda con la app
  minimizada o con otra app enfocada (ej. Ctrl+Alt+P, spec 006).
- **Client ID (propio)** — identificador de la app que cada usuario crea
  en el Spotify Developer Dashboard; se usa para el token de la Web API.
  No es secreto (con PKCE no hay client secret). Se carga con `setup` y
  queda en `client-id.txt` (spec 008). No confundir con el Client ID de
  librespot, fijo en `config.rs`, que se usa para el audio.
- **Modo desarrollo** — estado de toda app nueva del Dashboard: hasta 5
  cuentas, cargadas a mano en **User Management**, y deja de andar si su
  dueño pierde Premium. Por eso cada usuario usa su propio Client ID.
- **User Management** — sección de una app del Dashboard donde su dueño
  habilita a otras cuentas (modo desarrollo). Una cuenta no habilitada
  recibe 403 de la Web API.
