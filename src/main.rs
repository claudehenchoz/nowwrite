use nowwrite::platform::terminal::TerminalPlatform;
use nowwrite::platform::Platform;
use std::process::ExitCode;

fn main() -> ExitCode {
    // `run` consumes the platform, so the terminal is restored before any
    // error is printed.
    let result = TerminalPlatform::init().and_then(|(platform, backend)| nowwrite::run(platform, backend));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("nowwrite: {err}");
            ExitCode::FAILURE
        }
    }
}
