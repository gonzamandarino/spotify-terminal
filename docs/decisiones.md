# Decisiones

Una entrada por decisión: fecha, decisión, alternativas, motivo, y
**Estado** (`Vigente` | `Reemplazada por <fecha/spec>`). No se borran
entradas viejas — se marcan reemplazadas.

## 2026-09-26 — Lenguaje: Rust
- **Decisión:** el cliente se escribe en Rust.
- **Alternativas:** Go (`go-librespot`), Python (con reproductor externo).
- **Motivo:** `librespot` es Rust y se embebe sin puente; binario único y el
  menor consumo de memoria. Además, aprender Rust es un objetivo del
  proyecto (ver `rust-mentor`).
- **Estado:** Vigente (spec 001)

## 2026-09-26 — Audio: `librespot` embebido en el mismo proceso
- **Decisión:** el cliente es a la vez dispositivo de reproducción, usando
  las crates de `librespot`, sin depender de otra app de Spotify abierta.
- **Alternativas:** controlar un dispositivo Spotify Connect existente vía
  Web API; `spotifyd` como proceso aparte.
- **Motivo:** un solo proceso liviano que reemplaza a la app oficial.
  Requiere Premium (confirmado). Respaldo si Spotify rompe `librespot`:
  controlar un dispositivo Connect vía Web API.
- **Estado:** Vigente (spec 001)

## 2026-09-26 — Consumo: RAM baja, pero la reproducción manda
- **Decisión:** objetivo ≤ 60 MB de RAM reproduciendo. Si reducir memoria
  (buffers de audio, caché) provoca cortes o demoras audibles, se prioriza
  la reproducción y se documenta el consumo real acá.
- **Motivo:** pedido explícito — "bajo, pero que no afecte la reproducción".
- **Estado:** Vigente (spec 001)
