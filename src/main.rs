use std::process::ExitCode;

const PROGRAM: &str = env!("CARGO_PKG_NAME");

fn main() -> ExitCode {
    if let Some(command) = std::env::args().nth(1) {
        eprintln!("{PROGRAM}: unknown command '{command}'");
        eprintln!("usage: {PROGRAM}");
        return ExitCode::from(2);
    }
    if let Err(error) = matrix_code::run() {
        eprintln!("{PROGRAM}: {error:#}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
