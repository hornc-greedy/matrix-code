use std::process::ExitCode;

use matrix_code::{DEFAULT_SCREENSAVER_SECONDS, PROGRAM};

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let result = match words.as_slice() {
        [] => matrix_code::run(),
        ["install"] => matrix_code::install(),
        ["uninstall"] => matrix_code::uninstall(),
        ["screensaver"] => matrix_code::screensaver(Some(DEFAULT_SCREENSAVER_SECONDS)),
        ["screensaver", "off"] => matrix_code::screensaver(None),
        ["screensaver", seconds] => match parse_seconds(seconds) {
            Some(seconds) => matrix_code::screensaver(Some(seconds)),
            None => {
                eprintln!("{PROGRAM}: SECONDS must be a whole number above 0");
                return usage();
            }
        },
        _ => {
            eprintln!("{PROGRAM}: unknown command '{}'", words.join(" "));
            return usage();
        }
    };
    if let Err(error) = result {
        eprintln!("{PROGRAM}: {error:#}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Zero would turn the zsh timeout off, so it counts as invalid.
fn parse_seconds(word: &str) -> Option<u32> {
    word.parse().ok().filter(|&seconds| seconds > 0)
}

fn usage() -> ExitCode {
    eprintln!("usage: {PROGRAM} [install | uninstall | screensaver [SECONDS | off]]");
    ExitCode::from(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_are_positive_numbers() {
        assert_eq!(parse_seconds("600"), Some(600));
        assert_eq!(parse_seconds("0"), None);
        assert_eq!(parse_seconds("ten"), None);
    }
}
