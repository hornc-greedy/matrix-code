use std::process::ExitCode;

use matrix_code::PROGRAM;

fn main() -> ExitCode {
    let result = match std::env::args().nth(1).as_deref() {
        None => matrix_code::run(),
        Some("install") => matrix_code::install(),
        Some("uninstall") => matrix_code::uninstall(),
        Some(command) => {
            eprintln!("{PROGRAM}: unknown command '{command}'");
            eprintln!("usage: {PROGRAM} [install | uninstall]");
            return ExitCode::from(2);
        }
    };
    if let Err(error) = result {
        eprintln!("{PROGRAM}: {error:#}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
