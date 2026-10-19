mod benchmark;
mod cli;
mod tracer;
use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    match benchmark::run(cli::Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Render failed: {error}");
            ExitCode::FAILURE
        }
    }
}
