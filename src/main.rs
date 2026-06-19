mod app;
mod backend;
mod client_buffer;
mod config;
mod core;
mod error;
mod input;
mod output;
mod render;

pub use core::{animation, compositor, scene, shell, state, window};

use std::process::ExitCode;

use crate::{app::App, error::AppError};

fn main() -> ExitCode {
    init_logging();

    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("LIME DE failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), AppError> {
    let mut app = App::new();

    app.initialize()?;
    let run_result = app.run();
    let shutdown_result = app.shutdown();

    run_result.and(shutdown_result)
}

fn init_logging() {
    println!("LIME DE starting");
}
