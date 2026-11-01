//! The bundled font and its place in the user's font folder.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::PROGRAM;

const FONT: &[u8] = include_bytes!("../assets/matrix-code.ttf");
const FILE_NAME: &str = "matrix-code.ttf";

/// Copies the font into the user's font folder.
///
/// # Errors
///
/// Fails if the font folder cannot be found or written, or if the font cache
/// cannot be refreshed.
pub fn install() -> Result<()> {
    let dir = font_dir()?;
    fs::create_dir_all(&dir).with_context(|| format!("could not create {}", dir.display()))?;
    let path = dir.join(FILE_NAME);
    fs::write(&path, FONT).with_context(|| format!("could not write {}", path.display()))?;
    refresh_font_cache(&dir)?;
    println!("{PROGRAM}: installed {}", path.display());
    println!("{PROGRAM}: restart the terminal with all its windows to use the font");
    Ok(())
}

/// The user's font folder. fontconfig on Linux reads `$XDG_DATA_HOME/fonts`,
/// macOS reads `~/Library/Fonts`.
fn font_dir() -> Result<PathBuf> {
    if cfg!(target_os = "macos") {
        return Ok(home_dir()?.join("Library/Fonts"));
    }
    match std::env::var_os("XDG_DATA_HOME") {
        // The XDG Base Directory Specification treats an empty or relative
        // value as unset
        Some(data) if Path::new(&data).is_absolute() => Ok(PathBuf::from(data).join("fonts")),
        _ => Ok(home_dir()?.join(".local/share/fonts")),
    }
}

fn home_dir() -> Result<PathBuf> {
    std::env::home_dir().context("could not find the home directory")
}

/// macOS notices new fonts by itself. fontconfig on Linux needs `fc-cache`.
fn refresh_font_cache(dir: &Path) -> Result<()> {
    if cfg!(target_os = "macos") {
        return Ok(());
    }
    // -f because overwriting the font keeps the folder's modification time
    let status = Command::new("fc-cache")
        .arg("-f")
        .arg(dir)
        .status()
        .context("could not run fc-cache")?;
    if !status.success() {
        bail!("fc-cache failed with {status}");
    }
    Ok(())
}
