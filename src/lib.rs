//! Columns of characters in the terminal, lit by trails that move down over
//! them.

use std::io::{self, IsTerminal, Write};

use anyhow::{Context, Result, bail};
use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};
use crossterm::{cursor, event, execute, queue, terminal};

/// An empty cell between columns keeps them apart, like on the operator screens.
const COLUMN_SPACING: usize = 2;
/// Color of the characters, from the 256-color palette.
const CHARACTER_COLOR: u8 = 46;

/// Shows the animation until a key is pressed.
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
        .and_then(|()| animate().context("could not show the animation"));
    let restored = restore_terminal().context("could not restore the terminal");
    animated.and(restored)
}

fn setup_terminal() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    execute!(io::stdout(), terminal::EnterAlternateScreen, cursor::Hide)
}

fn restore_terminal() -> io::Result<()> {
    let left = execute!(
        io::stdout(),
        ResetColor,
        cursor::Show,
        terminal::LeaveAlternateScreen
    );
    let cooked = terminal::disable_raw_mode();
    left.and(cooked)
}

fn animate() -> io::Result<()> {
    let (width, height) = terminal::size()?;
    let screen = Screen::new(width, height);
    screen.draw(&mut io::stdout().lock())?;
    while !event::read()?.is_key_press() {}
    Ok(())
}

struct Screen {
    rows: u16,
    columns: Vec<Column>,
}

impl Screen {
    fn new(width: u16, height: u16) -> Self {
        let characters = fallback_characters();
        let columns = (0..width)
            .step_by(COLUMN_SPACING)
            .map(|x| Column::new(x, height, &characters))
            .collect();
        Self {
            rows: height,
            columns,
        }
    }

    fn draw(&self, out: &mut impl Write) -> io::Result<()> {
        for column in &self.columns {
            for row in 0..self.rows {
                queue!(
                    out,
                    cursor::MoveTo(column.x, row),
                    SetForegroundColor(Color::AnsiValue(CHARACTER_COLOR)),
                    Print(column.characters[usize::from(row)])
                )?;
            }
        }
        out.flush()
    }
}

struct Column {
    x: u16,
    characters: Vec<char>,
}

impl Column {
    fn new(x: u16, rows: u16, characters: &[char]) -> Self {
        Self {
            x,
            characters: (0..rows).map(|_| random_character(characters)).collect(),
        }
    }
}

/// Half-width katakana take one cell each, like the digits.
fn fallback_characters() -> Vec<char> {
    ('\u{FF66}'..='\u{FF9D}').chain('0'..='9').collect()
}

fn random_character(characters: &[char]) -> char {
    characters[fastrand::usize(..characters.len())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_leave_an_empty_cell_between_them() {
        let screen = Screen::new(5, 3);
        let xs: Vec<u16> = screen.columns.iter().map(|column| column.x).collect();
        assert_eq!(xs, [0, 2, 4]);
    }
}
