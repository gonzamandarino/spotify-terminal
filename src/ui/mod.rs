//! Interfaz con el usuario: subcomandos de línea (`cli`), la elección de un
//! resultado de búsqueda (`select`) y la pantalla mínima de reproducción
//! (`playback`). La TUI llega en su propio spec.

use crossterm::terminal;

use crate::error::AppError;

pub mod cli;
pub mod playback;
pub mod select;

/// Modo raw de la terminal (teclas sin Enter) mientras vive el valor; al
/// soltarse (`Drop`) vuelve al modo normal, aunque se salga por un error.
struct RawMode;

impl RawMode {
    fn enable() -> Result<RawMode, AppError> {
        terminal::enable_raw_mode().map_err(AppError::Terminal)?;
        Ok(RawMode)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}
