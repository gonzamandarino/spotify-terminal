# Glosario

Vocabulario del dominio, para que specs, código y docs usen el mismo término
con el mismo significado. Se agrega una entrada cuando un spec introduce un
concepto nuevo — no antes, no de antemano especulando.

- **Ajustes** — lo que el usuario personaliza en la app de escritorio
  desde la barra de menús (spec 007): tema, fuente, atajos, reproducción,
  consola y ventana. Se guardan en `ajustes.json`; los valores de
  `config.rs` son los de fábrica.
- **Atajo de ventana** — combinación que anda solo con la ventana de la
  app enfocada (ej. Ctrl+→). Ver también **atajo global**.
- **Atajo global** — combinación registrada en Windows que anda con la app
  minimizada o con otra app enfocada (ej. Ctrl+Alt+P, spec 006).
