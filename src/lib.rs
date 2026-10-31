//! Columns of characters in the terminal, lit by trails that move down over
//! them.

use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use crossterm::style::{Attribute, Color, Print, SetAttribute, SetForegroundColor};
use crossterm::{cursor, event, execute, queue, terminal};

/// Time between two frames.
const FRAME_TIME: Duration = Duration::from_millis(33);
/// An empty cell between columns keeps them apart, like on the operator screens.
const COLUMN_SPACING: usize = 2;
/// Color of the cursor at the bottom of each trail, from the 256-color palette.
const CURSOR_COLOR: u8 = 231;
/// Rows per frame of the slowest column. Each column has a speed of its own.
const SLOWEST_SPEED: f32 = 0.5;
/// Rows per frame of the fastest column.
const FASTEST_SPEED: f32 = 1.0;
/// Shortest distance between the starts of two trails, as a share of the height.
const SHORTEST_GAP: f32 = 0.5;
/// Longest distance between the starts of two trails, as a share of the height.
const LONGEST_GAP: f32 = 1.5;

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
        SetAttribute(Attribute::Reset),
        cursor::Show,
        terminal::LeaveAlternateScreen
    );
    let cooked = terminal::disable_raw_mode();
    left.and(cooked)
}

fn animate() -> io::Result<()> {
    let (width, height) = terminal::size()?;
    let mut screen = Screen::new(width, height);
    let mut stdout = io::stdout().lock();
    loop {
        let next_frame = Instant::now() + FRAME_TIME;
        screen.advance();
        screen.draw(&mut stdout)?;
        while event::poll(next_frame.saturating_duration_since(Instant::now()))? {
            if event::read()?.is_key_press() {
                return Ok(());
            }
        }
    }
}

struct Screen {
    rows: u16,
    columns: Vec<Column>,
    /// The cells of the last frame, row by row.
    drawn: Vec<Option<(char, u8)>>,
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
            drawn: Vec::new(),
        }
    }

    fn advance(&mut self) {
        for column in &mut self.columns {
            column.advance(self.rows);
        }
    }

    /// Draws the cells that differ from the last frame, row by row. A new
    /// screen has no last frame, so its first frame draws every cell.
    /// Terminals with synchronized updates show each frame at once.
    fn draw(&mut self, out: &mut impl Write) -> io::Result<()> {
        queue!(out, terminal::BeginSynchronizedUpdate)?;
        let mut cells = Vec::with_capacity(self.drawn.len());
        for row in 0..self.rows {
            for column in &self.columns {
                let cell = column.cell(row);
                if self.drawn.get(cells.len()) != Some(&cell) {
                    queue!(out, cursor::MoveTo(column.x, row))?;
                    match cell {
                        None => queue!(out, Print(' '))?,
                        Some((character, color)) => {
                            queue!(
                                out,
                                SetAttribute(Attribute::Bold),
                                SetForegroundColor(Color::AnsiValue(color)),
                                Print(character)
                            )?;
                        }
                    }
                }
                cells.push(cell);
            }
        }
        self.drawn = cells;
        queue!(out, terminal::EndSynchronizedUpdate)?;
        out.flush()
    }
}

struct Column {
    x: u16,
    speed: f32,
    rows_until_next_trail: f32,
    characters: Vec<char>,
    /// Oldest trail first. All trails share the speed, so none overtakes another.
    trails: Vec<Trail>,
}

impl Column {
    fn new(x: u16, rows: u16, characters: &[char]) -> Self {
        Self {
            x,
            speed: random_between(SLOWEST_SPEED, FASTEST_SPEED),
            // Columns start at different times, so the screen fills gradually
            rows_until_next_trail: random_between(0.0, f32::from(rows)),
            characters: (0..rows).map(|_| random_character(characters)).collect(),
            trails: Vec::new(),
        }
    }

    fn advance(&mut self, rows: u16) {
        let height = f32::from(rows);
        for trail in &mut self.trails {
            trail.cursor_row += self.speed;
        }
        self.trails.retain(|trail| trail.cursor_row < height);

        self.rows_until_next_trail -= self.speed;
        if self.rows_until_next_trail <= 0.0 {
            self.trails.push(Trail { cursor_row: 0.0 });
            self.rows_until_next_trail = random_between(SHORTEST_GAP, LONGEST_GAP) * height;
        }
    }

    /// The character and color in a row, or `None` for an empty cell.
    fn cell(&self, row: u16) -> Option<(char, u8)> {
        let color = self.color(row)?;
        Some((self.characters[usize::from(row)], color))
    }

    /// Only the cursor of a trail is lit.
    fn color(&self, row: u16) -> Option<u8> {
        self.trails
            .iter()
            .any(|trail| trail.cursor_row as u16 == row)
            .then_some(CURSOR_COLOR)
    }
}

struct Trail {
    cursor_row: f32,
}

/// Half-width katakana take one cell each, like the digits.
fn fallback_characters() -> Vec<char> {
    ('\u{FF66}'..='\u{FF9D}').chain('0'..='9').collect()
}

fn random_character(characters: &[char]) -> char {
    characters[fastrand::usize(..characters.len())]
}

fn random_between(lowest: f32, highest: f32) -> f32 {
    lowest + fastrand::f32() * (highest - lowest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trails_never_overtake_each_other() {
        let mut column = Column::new(0, 40, &['a']);
        for _ in 0..500 {
            column.advance(40);
            for pair in column.trails.windows(2) {
                assert!(pair[0].cursor_row > pair[1].cursor_row);
            }
        }
    }

    #[test]
    fn trails_leave_once_they_are_off_screen() {
        let mut column = Column::new(0, 10, &['a']);
        column.speed = 0.5;
        column.rows_until_next_trail = 100.0;
        column.trails = vec![Trail { cursor_row: 9.6 }, Trail { cursor_row: 9.4 }];
        column.advance(10);
        assert_eq!(column.trails.len(), 1);
        assert!(column.trails[0].cursor_row < 10.0);
    }

    #[test]
    fn columns_leave_an_empty_cell_between_them() {
        let screen = Screen::new(5, 3);
        let xs: Vec<u16> = screen.columns.iter().map(|column| column.x).collect();
        assert_eq!(xs, [0, 2, 4]);
    }

    #[test]
    fn a_frame_without_changes_draws_nothing() {
        let mut screen = Screen::new(5, 3);
        let mut first = Vec::new();
        screen.draw(&mut first).unwrap();
        let mut second = Vec::new();
        screen.draw(&mut second).unwrap();
        let mut no_cells = Vec::new();
        queue!(
            no_cells,
            terminal::BeginSynchronizedUpdate,
            terminal::EndSynchronizedUpdate
        )
        .unwrap();
        assert!(first.len() > no_cells.len());
        assert_eq!(second, no_cells);
    }
}
