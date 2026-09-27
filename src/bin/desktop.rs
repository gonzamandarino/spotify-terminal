//! App de escritorio (spec 004): abre la ventana con la consola. Sin
//! consola de Windows detrás (subsistema "windows"), así que los errores
//! fatales se muestran en un cuadro de diálogo.
#![windows_subsystem = "windows"]

use std::process::ExitCode;

fn main() -> ExitCode {
    match spotify_terminal::desktop::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            spotify_terminal::desktop::show_fatal_error(&e);
            ExitCode::FAILURE
        }
    }
}
