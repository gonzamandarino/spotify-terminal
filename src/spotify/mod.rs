//! Integración con Spotify: login/tokens (`auth`), tapas de los discos
//! (`cover`, spec 010), reproductor (`player`),
//! copia de lo que suena para dibujarlo (`tap`, spec 010) y Web API
//! (`web`).

pub mod auth;
pub mod cover;
pub mod player;
pub mod tap;
pub mod web;
