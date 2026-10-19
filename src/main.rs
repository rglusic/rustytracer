mod tracer;
use std::process::ExitCode;

fn main() -> ExitCode {
    match tracer::generate("output/image.png", 200, 100) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Render failed: {error}");
            ExitCode::FAILURE
        }
    }
}
