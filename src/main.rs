mod config;
mod error;
mod spotify;
mod ui;

use std::process::ExitCode;

use config::Config;
use error::AppError;
use spotify::auth;
use ui::cli::{self, Command};

// Un solo hilo alcanza: la red es async y librespot reproduce en su propio
// hilo. Menos hilos = menos memoria.
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    // Todo error llega hasta acá como `AppError` y se muestra con su mensaje
    // para el usuario; nada de stack traces ni `panic!`.
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), AppError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = Config::load()?;

    match cli::parse(&args)? {
        Command::Help => println!("{}", cli::USAGE),
        Command::Login => {
            auth::get_valid_token(&config).await?;
            println!(
                "Sesión lista (guardada en {}).",
                config.token_cache_path().display()
            );
        }
        Command::Logout => {
            if auth::logout(&config)? {
                println!("Sesión cerrada.");
            } else {
                println!("No había una sesión guardada.");
            }
        }
    }
    Ok(())
}
