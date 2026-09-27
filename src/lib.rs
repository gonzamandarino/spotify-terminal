//! Cliente de Spotify liviano. La lógica vive en esta librería; los
//! binarios son la CLI (`src/main.rs`) y la app de escritorio
//! (`src/bin/desktop.rs`).

pub mod app;
pub mod config;
pub mod desktop;
pub mod error;
pub mod setup;
pub mod spotify;
pub mod ui;
