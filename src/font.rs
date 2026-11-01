//! The bundled font and its place in the user's font folder.

use std::fs;
use std::io;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::{PROGRAM, home_dir};

/// Characters the bundled font draws. They lie in the Supplementary Private
/// Use Area-B, which other fonts leave empty, so terminals fall back to the
/// bundled font for them.
pub const CHARACTERS: RangeInclusive<char> = '\u{100000}'..='\u{100038}';

const FONT: &[u8] = include_bytes!("../assets/matrix-code.ttf");
const FILE_NAME: &str = "matrix-code.ttf";

/// Whether the font file is in the user's font folder.
pub fn is_installed() -> bool {
    font_dir().is_ok_and(|dir| dir.join(FILE_NAME).exists())
}

/// Copies the font into the user's font folder and returns whether it was not
/// there before.
///
/// # Errors
///
/// Fails if the font folder cannot be found or written, or if the font cache
/// cannot be refreshed.
pub fn install() -> Result<bool> {
    let dir = font_dir()?;
    fs::create_dir_all(&dir).with_context(|| format!("could not create {}", dir.display()))?;
    let path = dir.join(FILE_NAME);
    let is_new = !path.exists();
    fs::write(&path, FONT).with_context(|| format!("could not write {}", path.display()))?;
    refresh_font_cache(&dir)?;
    println!("{PROGRAM}: installed {}", path.display());
    if is_new {
        println!("{PROGRAM}: restart the terminal with all its windows to use the font");
    }
    Ok(is_new)
}

/// Removes the font from the user's font folder.
///
/// # Errors
///
/// Fails if the font folder cannot be found, the font cannot be removed, or
/// the font cache cannot be refreshed.
pub fn uninstall() -> Result<()> {
    let dir = font_dir()?;
    let path = dir.join(FILE_NAME);
    match fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            println!("{PROGRAM}: the font is not installed");
            return Ok(());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("could not remove {}", path.display()));
        }
    }
    refresh_font_cache(&dir)?;
    println!("{PROGRAM}: removed {}", path.display());
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
