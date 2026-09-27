# Changelog

## [Sin publicar]

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
