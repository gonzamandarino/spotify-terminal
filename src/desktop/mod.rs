//! App de escritorio: una ventana propia con la consola (spec 004). La
//! ventana (`window`) y la consola (`app`) corren en el hilo principal; el
//! motor (`app::engine`) en el suyo, y el audio en los de `spotify::player`.

mod app;
mod theme;
mod window;

use std::{fs::OpenOptions, io::Write};

use crate::{app::engine, config, error::AppError};

/// Abre la ventana y la atiende hasta que se cierra.
///
/// - Post: al volver, el motor terminó y el audio está cortado (el
///   reproductor se suelta en el hilo del motor antes de que este termine).
///   Un panic se anota en `config::PANIC_LOG_FILE`, en la carpeta de datos,
///   porque la app no tiene consola donde mostrarlo.
/// - Errores: `Internal` si no arranca el motor o no se puede abrir la
///   ventana. Los errores de Spotify no llegan acá: se muestran en la
///   consola y la app sigue abierta.
pub fn run() -> Result<(), AppError> {
    log_panics();
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let wake = ctx.clone();
    let engine::EngineHandle {
        inputs,
        outputs,
        thread,
    } = engine::spawn(move || wake.request_repaint())?;

    let mut console = app::DesktopApp::new(inputs, outputs);
    let result = window::run(ctx, &mut console);
    // Sin la consola se suelta el emisor de comandos: el motor corta el
    // audio y termina.
    drop(console);
    if thread.join().is_err() {
        return Err(AppError::Internal("el motor terminó con un panic".into()));
    }
    result
}

/// Muestra un error fatal en un cuadro de diálogo nativo (la app no tiene
/// consola). Para `bin/desktop.rs`.
pub fn show_fatal_error(error: &AppError) {
    let text = format!("{error}\0");
    let title = format!("{}\0", config::WINDOW_TITLE);
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
        let wide = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
        let (text, title) = (wide(&text), wide(&title));
        // SAFETY: los dos textos terminan en \0 y viven durante la llamada;
        // sin ventana dueña (0).
        unsafe {
            MessageBoxW(0, text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
        }
    }
    #[cfg(not(windows))]
    eprintln!("{}", text.trim_end_matches('\0'));
}

/// Suma al hook de panic la escritura del mensaje en el log de la carpeta
/// de datos. Si no se puede escribir, se sigue igual.
fn log_panics() {
    let Ok(dir) = config::data_dir() else {
        return;
    };
    let path = dir.join(config::PANIC_LOG_FILE);
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(file, "{info}");
        }
        default_hook(info);
    }));
}
