//! Columns of characters in the terminal, lit by trails that move down over
//! them.

use std::io;

use anyhow::{Context, Result};
use crossterm::{cursor, event, execute, terminal};

/// Holds an empty screen until a key is pressed.
///
/// # Errors
///
/// Fails if the terminal cannot be set up, used or restored.
pub fn run() -> Result<()> {
    let animated = setup_terminal()
        .context("could not set up the terminal")
        .and_then(|()| wait_for_key().context("could not wait for a key"));
    let restored = restore_terminal().context("could not restore the terminal");
    animated.and(restored)
}

fn setup_terminal() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    execute!(io::stdout(), terminal::EnterAlternateScreen, cursor::Hide)
}

fn restore_terminal() -> io::Result<()> {
    let left = execute!(io::stdout(), cursor::Show, terminal::LeaveAlternateScreen);
    let cooked = terminal::disable_raw_mode();
    left.and(cooked)
}

fn wait_for_key() -> io::Result<()> {
    while !event::read()?.is_key_press() {}
    Ok(())
}
