# Changelog

## [Sin publicar]

### Agregado (spec 007 — customización)
- Barra de menús en la barra de título de la app de escritorio, como en
  VS Code: Tema, Fuente, Atajos, Reproducción, Consola, Ventana y
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
