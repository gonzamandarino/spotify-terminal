//! Lógica compartida por la CLI y la app de escritorio, sin I/O de pantalla:
//! la cola de reproducción (`queue`), los comandos de la consola (`shell`)
//! y el motor de la app de escritorio (`engine`, con `backend` como costura
//! hacia Spotify).

pub(crate) mod backend;
pub(crate) mod engine;
pub mod queue;
pub(crate) mod shell;
