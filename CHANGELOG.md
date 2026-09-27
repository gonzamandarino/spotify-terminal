# Changelog

## [Sin publicar]

### Agregado (spec 001 — stack, reproductor y login)
- `login` / `logout`: OAuth PKCE con dos tokens (audio y Web API), cacheados
  en `%APPDATA%\spotify-terminal\` y renovados solos.
- `whoami`: usuario y plan de la cuenta.
- `play <tema|álbum|playlist>`: reproduce por esta PC con `librespot`;
  espacio = pausa/reanudar, q = salir. Precarga el siguiente tema.
- Mensajes de error con instrucciones (sin Premium, sin red, login
  cancelado, sesión rechazada, límite de pedidos) y código de salida 1.
- `scripts/medir-consumo.ps1`: RAM y CPU del proceso durante N minutos.
