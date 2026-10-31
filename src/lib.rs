//! Columns of characters in the terminal, lit by trails that move down over
//! them.

use std::io::{self, IsTerminal};

use anyhow::{Context, Result, bail};
use crossterm::{cursor, event, execute, terminal};

/// Holds an empty screen until a key is pressed.
///
/// # Errors
///
/// Fails if the output is not a terminal, or if the terminal cannot be set
/// up, used or restored.
pub fn run() -> Result<()> {
    if !io::stdout().is_terminal() {
        bail!("needs a terminal, but the output is redirected");
    }
    // A panic would otherwise leave the terminal in raw mode and hide its message
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        default_panic(info);
    }));

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
