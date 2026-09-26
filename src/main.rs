mod config;
mod error;

use std::process::ExitCode;

use config::Config;
use error::AppError;

fn main() -> ExitCode {
    // Todo error llega hasta acá como `AppError` y se muestra con su mensaje
    // para el usuario; nada de stack traces ni `panic!`.
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), AppError> {
    let config = Config::load()?;
    println!("Client ID: {}", config.masked_client_id());
    println!("Datos locales en: {}", config.data_dir.display());
    Ok(())
}
