# Changelog

## [0.4.0] - 2026-09-28

### Agregado (spec 012 — botones del reproductor)
- La barra de abajo tiene botones: ⏮ anterior, ⏯ pausa / reanudar y ⏭
  siguiente en el centro, ♥ al lado del tema (agrega o quita de Tus me
  gusta) y 🔀 shuffle a la derecha. Hacen lo mismo que sus comandos y al
  pasar el mouse muestran su atajo. Sin nada sonando se ven apagados.
- Si la ventana es angosta, primero se ocultan la posición, la cola y el
  volumen de la barra, y el título se corta: los botones siempre se ven.

### Cambiado
- El ícono al lado del tema (▶ / ⏸) ya no cambia de color al pausar.

## [0.3.0] - 2026-09-28

### Agregado (spec 011 — like y mis playlists)
- `like` agrega el tema que suena a Tus me gusta y `unlike` lo quita. La
  barra de abajo muestra un ♥ encendido si el tema ya está likeado.
- `playlists` (o `pl`) lista tus playlists (las tuyas y las que seguís) y
  reproduce la que elijas por número; `playlists -s` arranca mezclado.

### Cambiado
- La app pide permisos nuevos de Spotify (biblioteca y playlists): la
  primera vez después de actualizar hay que autorizar de nuevo en el
  navegador.

## [0.2.0] - 2026-09-27

### Agregado (spec 010 — visualización)
- Personalización → Visualización en la app de escritorio: un panel a la
  derecha de la consola con la **onda** de lo que suena, **barras** del
  espectro o un **vinilo** girando con la tapa del disco del tema. Se
  anima a 15 cuadros por segundo solo mientras suena; en pausa o sin
  música no gasta CPU. Viene apagada (Ninguna).
- El ancho del panel se cambia arrastrando su borde izquierdo y queda
  guardado. Si la ventana es angosta el panel se achica para dejarle
  lugar a la consola, y se oculta si ni así entra.

### Corregido
- Cambiar o desconectar la salida de audio de Windows cortaba la música y
  colgaba la app (también al pausar o cerrarla). Ahora sigue sonando por
  el nuevo dispositivo por defecto; si no queda ninguno, pausa.

## [0.1.1] - 2026-09-27

### Cambiado (spec 009 — Personalización y tamaño de letra)
- La barra de menús de la app de escritorio queda con tres menús:
  **Personalización** (`Alt+P`), **Reproducción** (`Alt+R`) y **Ajustes**
  (`Alt+A`, antes `Alt+J`). Tema, Fuente, Atajos, Consola y Ventana son
  ahora submenús de Personalización; → abre un submenú y ← vuelve.
- `Alt+T`, `Alt+F`, `Alt+C`, `Alt+V` y `Alt+J` se pueden usar como atajos
  de ventana; `Alt+P` ya no (si un atajo guardado la usaba, vuelve a
  fábrica con un aviso).

### Corregido (spec 009)
- El tamaño de letra agrandaba toda la ventana (barras, menús, márgenes)
  en vez de la letra, y con letra grande la consola desaparecía. Ahora
  cambia solo el texto de la consola, la entrada y la barra "sonando";
  las barras crecen lo justo para que el texto entre y la ventana no
  cambia de tamaño.

## [0.1.0] - 2026-09-27

### Agregado (spec 008 — distribución)
- Zip para Windows en GitHub Releases: se descomprime y se usa, sin Rust
  ni Build Tools (los `.exe` ya no dependen de `vcruntime140.dll`).
- La primera vez, la CLI y la app de escritorio explican cómo crear tu
  app en el Spotify Developer Dashboard y piden el Client ID; se guarda en
  `%APPDATA%\spotify-terminal\client-id.txt` y sigue directo con el login.
  Ya no hace falta un `.env` ni abrir la app desde una carpeta en
  particular (`.env` y la variable de entorno siguen andando y tienen
  prioridad).
- Comando `setup` (CLI y app de escritorio) para cambiar el Client ID;
  al guardarlo sigue directo con el login.
- `spotify-terminal --version` y `version` en la app de escritorio.
- Mensajes claros para un Client ID que Spotify no reconoce (se detecta
  antes de abrir el navegador) y para una cuenta no habilitada en la app
  del Dashboard de otra persona.
- `crear-accesos-directos.cmd` en el zip; el script de accesos directos
  usa la carpeta del zip si está ahí.

### Cambiado (spec 008)
- `logout` anda aunque no haya Client ID configurado.

### Agregado (spec 007 — customización)
- Barra de menús en un renglón debajo de la barra de título de la app de
  escritorio: Tema, Fuente, Atajos, Reproducción, Consola, Ventana y
  Ajustes. Se abren con clic o `Alt`+letra y se navegan con flechas.
- Temas predefinidos (Spotify oscuro, Claro, Alto contraste) y editor de
  cada color; fuente (las monoespaciadas instaladas) y tamaño como zoom
  de toda la ventana (Ctrl++ / Ctrl+- / Ctrl+0 / Ctrl+rueda).
- Atajos de ventana y globales configurables: se graban apretando la
  combinación; avisa si es reservada o si ya la usa otro atajo. Nuevos
  atajos opcionales para stop, shuffle y limpiar consola.
- Paso de volumen, umbral de "anterior" y calidad de audio (96 / 160 /
  320 kbps, desde el próximo `play`).
- Prompt, líneas guardadas, historial y hora en cada línea; ventana
  siempre visible y recordar tamaño y posición.
- Todo se aplica al instante y se guarda en `ajustes.json` (solo lo que
  difiere de fábrica); un archivo roto nunca impide abrir la app.
  Ctrl+Shift+F12 restaura todo.
- `help` muestra los atajos vigentes (los configurados).

### Agregado (spec 006 — atajos globales)
- Atajos que andan con la app de escritorio minimizada o en segundo
  plano: Ctrl+Alt+P pausa / reanudar, Ctrl+Alt+→ / ← siguiente /
  anterior, Ctrl+Alt+Enter stop, Ctrl+Alt+↑ / ↓ volumen.
- Si una combinación ya la usa otra app, la consola lo avisa al abrir y
  el resto sigue andando. `help` lista los atajos globales activos.
- Sin polling ni hook de teclado: Windows avisa solo cuando se aprieta
  uno de los atajos.

### Agregado (spec 005 — volumen)
- Volumen propio de la app, sin tocar el de Windows ni el de otras apps.
  En la ventana: `vol`/`v` (ver), `vol <0-100>`, `vol +`/`vol -`,
  `mute`/`m`, y Ctrl+↑ / Ctrl+↓. En la CLI, `+` (o `=`) y `-`
  durante la reproducción.
- La barra de "sonando ahora" y la línea de estado de la CLI muestran el
  volumen.
- El último volumen se guarda en `%APPDATA%\spotify-terminal\volumen.txt`
  y la app arranca ahí (100 % la primera vez).

### Agregado (spec 004 — app de escritorio)
- `spotify-desktop.exe`: ventana propia con una consola estilizada (tema
  oscuro, fuente Consolas como la consola de Windows, barra de título
  propia) donde se escriben los mismos comandos que en la CLI, sin
  PowerShell.
- La música sigue mientras se escribe: `pause`, `next`/`n`, `prev`/`p`,
  `shuffle`/`s`, `queue <tema>`/`a <tema>`, `stop`, `clear`, `exit`, y los
  atajos Ctrl+Espacio, Ctrl+→ y Ctrl+←. Un `play` nuevo reemplaza lo que
  sonaba.
- Barra de "sonando ahora" con tema, artistas, posición en la lista,
  progreso, shuffle y temas en cola.
- Historial con ↑/↓, Tab completa comandos, Esc cancela una búsqueda.
- `scripts/instalar-acceso-directo.ps1`: accesos directos en el escritorio
  y el menú Inicio. Ícono propio (`scripts/generar-icono.py`).
- Consumo (dibujo por CPU, sin GPU): ~26 MB de RAM y 0 % de CPU en reposo;
  reproduciendo, máximo 36,9 MB y 0,07 % de CPU promedio en 10 min.

### Cambiado (spec 004)
- La lógica pasó a una librería (`src/lib.rs`) y la cola de reproducción a
  `app::queue`, compartida por la CLI y la app de escritorio. La CLI se usa
  igual que antes.

### Agregado (spec 003 — shuffle, siguiente/anterior y cola)
- Durante la reproducción: `n`/→ siguiente, `p`/← reinicia el tema o vuelve
  al anterior (según si lleva más de 3 s), `s` shuffle sí/no (el tema
  actual sigue; se mezcla lo que falta).
- `play -s` / `--shuffle`: arranca la lista mezclada desde un tema al azar.
- `a`: busca un tema sin cortar la música y lo agrega a la cola; suena
  después del actual, antes del resto de la lista. Con `play` de un solo
  tema, al terminar siguen los encolados.
- La línea de estado muestra shuffle y cantidad de temas en cola.

### Agregado (spec 002 — búsqueda)
- `play <nombre>`: busca temas y muestra los 5 mejores (artistas, álbum,
  duración); se elige con 1-5, Enter = el primero, q = cancelar.
- `play list <nombre>` / `play playlist <nombre>`: lo mismo con playlists
  (dueño y cantidad de temas). Las playlists editoriales de Spotify no
  aparecen (limitación de la API para apps en modo desarrollo).
- `play` con URI, link o ID funciona igual que antes.

### Agregado (spec 001 — stack, reproductor y login)
- `login` / `logout`: OAuth PKCE con dos tokens (audio y Web API), cacheados
  en `%APPDATA%\spotify-terminal\` y renovados solos.
- `whoami`: usuario y plan de la cuenta.
- `play <tema|álbum|playlist>`: reproduce por esta PC con `librespot`;
  espacio = pausa/reanudar, q = salir. Precarga el siguiente tema.
- Mensajes de error con instrucciones (sin Premium, sin red, login
  cancelado, sesión rechazada, límite de pedidos) y código de salida 1.
- `scripts/medir-consumo.ps1`: RAM y CPU del proceso durante N minutos.
- Consumo medido en 30 min de reproducción: ~19 MB de RAM, 0,07 % de CPU.
