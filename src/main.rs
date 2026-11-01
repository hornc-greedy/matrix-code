use std::process::ExitCode;

use matrix_code::{PROGRAM, font};

fn main() -> ExitCode {
    let result = match std::env::args().nth(1).as_deref() {
        None => matrix_code::run(),
        Some("install") => font::install(),
        Some(command) => {
            eprintln!("{PROGRAM}: unknown command '{command}'");
            eprintln!("usage: {PROGRAM} [install]");
            return ExitCode::from(2);
        }
    };
    if let Err(error) = result {
        eprintln!("{PROGRAM}: {error:#}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
