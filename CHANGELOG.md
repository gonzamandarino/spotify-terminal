# Changelog

## [Sin publicar]

### Agregado (spec 004 — app de escritorio)
- `spotify-desktop.exe`: ventana propia con una consola estilizada (tema
  oscuro, JetBrains Mono, barra de título propia) donde se escriben los
  mismos comandos que en la CLI, sin PowerShell.
- La música sigue mientras se escribe: `pause`, `next`/`n`, `prev`/`p`,
  `shuffle`/`s`, `queue <tema>`/`a <tema>`, `stop`, `clear`, `exit`, y los
  atajos Ctrl+Espacio, Ctrl+→ y Ctrl+←. Un `play` nuevo reemplaza lo que
  sonaba.
- Barra de "sonando ahora" con tema, artistas, posición en la lista,
  progreso, shuffle y temas en cola.
- Historial con ↑/↓, Tab completa comandos, Esc cancela una búsqueda.
- `scripts/instalar-acceso-directo.ps1`: accesos directos en el escritorio
  y el menú Inicio. Ícono propio (`scripts/generar-icono.py`).
- Consumo en reposo: ~26 MB de RAM y 0 % de CPU (dibujo por CPU, sin GPU).

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
