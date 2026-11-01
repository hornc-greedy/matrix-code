//! Blocks in the shell startup files that put the program on the `PATH` and
//! start it as a screensaver in zsh.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::{PROGRAM, bin_dir, home_dir};

const BEGIN: &str = "# Added by matrix-code, removed by `matrix-code uninstall`";
const END: &str = "# End of matrix-code";
/// Startup files of a bash login shell. It reads the first one that exists.
const BASH_LOGIN_FILES: [&str; 3] = [".bash_profile", ".bash_login", ".profile"];

/// Writes the block for the shell in `$SHELL`.
///
/// # Errors
///
/// Fails if the home directory cannot be found, or if a startup file cannot be
/// read or written.
pub fn install() -> Result<()> {
    let program_dir = bin_dir()?;
    let path_line = format!("export PATH=\"{}:$PATH\"", program_dir.display());
    match shell_name().as_deref() {
        // zsh reads ~/.zshenv before a ZDOTDIR set in it takes effect
        Some("zsh") => write_block(&home_dir()?.join(".zshenv"), &path_line)?,
        Some("bash") => write_block(&bash_file()?, &path_line)?,
        _ => println!(
            "{PROGRAM}: add {} to your PATH to start the program by its name",
            program_dir.display()
        ),
    }
    Ok(())
}

/// Writes the screensaver block to `.zshrc`, which starts the installed
/// program when the prompt has waited at least the given seconds without a key
/// press. Without seconds it removes the block.
///
/// # Errors
///
/// Fails if the home directory cannot be found, if the shell is not zsh or the
/// program is not installed when seconds are given, or if `.zshrc` cannot be
/// read or written.
pub fn screensaver(seconds: Option<u32>) -> Result<()> {
    let zshrc = zsh_dir()?.join(".zshrc");
    let Some(seconds) = seconds else {
        remove_block(&zshrc)?;
        print_zsh_reload_hint();
        return Ok(());
    };
    if shell_name().as_deref() != Some("zsh") {
        bail!("the screensaver needs zsh");
    }
    let program = bin_dir()?.join(PROGRAM);
    if !program.exists() {
        bail!("run `{PROGRAM} install` first");
    }
    // zsh calls TRAPALRM after TMOUT seconds at the prompt and every TMOUT
    // seconds after that. TTYIDLE counts the seconds since the last key press.
    // A trap that returns non-zero interrupts the input line, which the `if`
    // avoids.
    let block = format!(
        "TMOUT={seconds}\nTRAPALRM() {{ if (( TTYIDLE >= TMOUT )); then \"{}\"; fi; }}",
        program.display()
    );
    write_block(&zshrc, &block)?;
    print_zsh_reload_hint();
    Ok(())
}

/// Open zsh windows keep the old `TMOUT` and `TRAPALRM` until they reload.
fn print_zsh_reload_hint() {
    println!("{PROGRAM}: run `exec zsh` to use the change in this window");
}

/// Removes the blocks from the startup files of every supported shell, so
/// nothing is left after a change of shell.
///
/// # Errors
///
/// Fails if the home directory cannot be found, or if a startup file cannot be
/// read, written or deleted.
pub fn uninstall() -> Result<()> {
    let home = home_dir()?;
    let mut files = vec![
        home.join(".zshenv"),
        zsh_dir()?.join(".zshrc"),
        home.join(".bashrc"),
    ];
    files.extend(BASH_LOGIN_FILES.map(|name| home.join(name)));
    for file in files {
        remove_block(&file)?;
    }
    Ok(())
}

/// Removes the block from a file and deletes a file that only held the block.
fn remove_block(file: &Path) -> Result<()> {
    let text = read(file)?;
    let rest = without_block(&text);
    if rest == text {
        return Ok(());
    }
    if rest.trim().is_empty() {
        fs::remove_file(file).with_context(|| format!("could not delete {}", file.display()))?;
    } else {
        fs::write(file, rest).with_context(|| format!("could not write {}", file.display()))?;
    }
    println!("{PROGRAM}: removed the block from {}", file.display());
    Ok(())
}

/// Tells how to load the changed startup files in the current window.
pub fn print_reload_hint() {
    if let Some(shell) = shell_name().filter(|name| name == "zsh" || name == "bash") {
        println!("{PROGRAM}: run `exec {shell}` to use the changes in this window");
    }
}

/// Replaces the block in a file, or appends it if the file has none.
fn write_block(file: &Path, content: &str) -> Result<()> {
    let text = without_block(&read(file)?);
    let separator = if text.is_empty() || text.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let block = format!("{BEGIN}\n{content}\n{END}\n");
    fs::write(file, format!("{text}{separator}{block}"))
        .with_context(|| format!("could not write {}", file.display()))?;
    println!("{PROGRAM}: wrote the block to {}", file.display());
    Ok(())
}

/// The file's text, or an empty string if the file does not exist.
fn read(file: &Path) -> Result<String> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error).with_context(|| format!("could not read {}", file.display())),
    }
}

/// The text without the lines from `BEGIN` to `END`. A `BEGIN` without a
/// later `END` was edited by hand, so the text stays as it is.
fn without_block(text: &str) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let begin = lines.iter().position(|line| line.trim_end() == BEGIN);
    let end = lines.iter().position(|line| line.trim_end() == END);
    match (begin, end) {
        (Some(begin), Some(end)) if begin < end => {
            lines[..begin].concat() + &lines[end + 1..].concat()
        }
        _ => text.to_string(),
    }
}

/// The name of the login shell.
fn shell_name() -> Option<String> {
    let shell = std::env::var_os("SHELL")?;
    Some(
        Path::new(&shell)
            .file_name()?
            .to_string_lossy()
            .into_owned(),
    )
}

/// The folder of `.zshrc`, which zsh takes from `ZDOTDIR` or the home.
fn zsh_dir() -> Result<PathBuf> {
    match std::env::var_os("ZDOTDIR") {
        Some(dir) if !dir.is_empty() => Ok(PathBuf::from(dir)),
        _ => home_dir(),
    }
}

/// Terminals on macOS start bash as a login shell, which reads the first of
/// its login files that exists. Terminals on Linux start it as an interactive
/// non-login shell, which reads `.bashrc`.
fn bash_file() -> Result<PathBuf> {
    let home = home_dir()?;
    if !cfg!(target_os = "macos") {
        return Ok(home.join(".bashrc"));
    }
    let existing = BASH_LOGIN_FILES
        .iter()
        .map(|name| home.join(name))
        .find(|file| file.exists());
    Ok(existing.unwrap_or_else(|| home.join(BASH_LOGIN_FILES[0])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_block_is_removed_with_nothing_else() {
        let text = format!("one\n{BEGIN}\nexport A=1\n{END}\ntwo\n");
        assert_eq!(without_block(&text), "one\ntwo\n");
    }

    #[test]
    fn text_without_a_block_stays_the_same() {
        assert_eq!(without_block("one\ntwo"), "one\ntwo");
    }

    #[test]
    fn a_block_without_an_end_stays_the_same() {
        let text = format!("one\n{BEGIN}\nexport A=1\ntwo\n");
        assert_eq!(without_block(&text), text);
    }
}
