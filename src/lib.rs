//! Columns of characters in the terminal, lit by trails that move down over
//! them.

use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use crossterm::style::{Attribute, Color, Print, SetAttribute, SetForegroundColor};
use crossterm::{cursor, event, execute, queue, terminal};

mod font;
mod shell;

/// Name of the program at the start of its messages.
pub const PROGRAM: &str = env!("CARGO_PKG_NAME");
/// Seconds the zsh prompt waits at least without a key press before the
/// screensaver starts, unless `screensaver` gets other seconds.
pub const DEFAULT_SCREENSAVER_SECONDS: u32 = 300;
/// Time between two frames.
const FRAME_TIME: Duration = Duration::from_millis(33);
/// An empty cell between columns keeps them apart and leaves room for the
/// wider glyphs of the bundled font.
const COLUMN_SPACING: usize = 2;
/// Color of the cursor at the bottom of each trail, from the 256-color palette.
const CURSOR_COLOR: u8 = 231;
/// Colors of the trail above the cursor, from bright to dark.
const TRAIL_COLORS: [u8; 7] = [157, 83, 46, 40, 34, 28, 22];
/// Rows per frame of the slowest column. Each column has a speed of its own.
const SLOWEST_SPEED: f32 = 0.5;
/// Rows per frame of the fastest column.
const FASTEST_SPEED: f32 = 1.0;
/// Shortest trail as a share of the screen height.
const SHORTEST_TRAIL: f32 = 0.25;
/// Longest trail as a share of the screen height.
const LONGEST_TRAIL: f32 = 0.67;
/// Shortest distance between the starts of two trails, as a share of the height.
const SHORTEST_GAP: f32 = 0.5;
/// Longest distance between the starts of two trails, as a share of the height.
const LONGEST_GAP: f32 = 1.5;
/// Chance per frame that a lit character turns into another one.
const CHARACTER_CHANGE_CHANCE: f32 = 0.011;

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
    // A terminal only finds a new font after a restart, so the run that
    // installs the font still uses the fallback characters
    let installed = font::is_installed();
    let characters: Vec<char> = if installed {
        font::CHARACTERS.collect()
    } else {
        fallback_characters()
    };
    if !installed && let Err(error) = font::install() {
        eprintln!("{PROGRAM}: could not install the font: {error:#}");
    }
    // A panic would otherwise leave the terminal in raw mode and hide its message
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        default_panic(info);
    }));

    let animated = setup_terminal()
        .context("could not set up the terminal")
        .and_then(|()| animate(&characters).context("could not show the animation"));
    let restored = restore_terminal().context("could not restore the terminal");
    animated.and(restored)
}

/// Copies the program to `~/.local/bin`, installs the font and writes the
/// block in the shell startup file.
///
/// # Errors
///
/// Fails if a folder cannot be found, if the program, the font or a startup
/// file cannot be read or written, or if the font cache cannot be refreshed.
pub fn install() -> Result<()> {
    install_program()?;
    let font_is_new = font::install()?;
    shell::install()?;
    // Restarting the terminal for a new font also reloads its shells
    if !font_is_new {
        shell::print_reload_hint();
    }
    Ok(())
}

/// Starts the animation as a screensaver in zsh when the prompt has waited at
/// least the given seconds without a key press, or turns the screensaver off
/// without seconds.
///
/// # Errors
///
/// Fails if the home directory cannot be found, if the shell is not zsh or the
/// program is not installed when seconds are given, or if `.zshrc` cannot be
/// read or written.
pub fn screensaver(seconds: Option<u32>) -> Result<()> {
    shell::screensaver(seconds)
}

/// Removes the font, the blocks in the shell startup files and the program
/// in `~/.local/bin`.
///
/// # Errors
///
/// Fails if a folder cannot be found, if the font, a block or the program
/// cannot be removed, or if the font cache cannot be refreshed.
pub fn uninstall() -> Result<()> {
    font::uninstall()?;
    shell::uninstall()?;
    remove_program()
}

/// The folder for a user's programs, as the XDG Base Directory Specification
/// names it.
fn bin_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".local/bin"))
}

/// Copies the running program to `~/.local/bin` unless it runs from there.
/// The copy replaces the program by a rename, because Linux refuses to write
/// into a running program and macOS stops a signed one that changes in place.
fn install_program() -> Result<()> {
    let current = std::env::current_exe()
        .and_then(fs::canonicalize)
        .context("could not find the program")?;
    let dir = bin_dir()?;
    let program = dir.join(PROGRAM);
    if program.canonicalize().is_ok_and(|path| path == current) {
        return Ok(());
    }
    fs::create_dir_all(&dir).with_context(|| format!("could not create {}", dir.display()))?;
    let copy = dir.join(format!(".{PROGRAM}.new"));
    fs::copy(&current, &copy).with_context(|| format!("could not write {}", copy.display()))?;
    fs::rename(&copy, &program)
        .with_context(|| format!("could not replace {}", program.display()))?;
    println!("{PROGRAM}: installed {}", program.display());
    Ok(())
}

/// Deletes the program in `~/.local/bin`. Linux and macOS free the file of a
/// running program only once it exits.
fn remove_program() -> Result<()> {
    let program = bin_dir()?.join(PROGRAM);
    match fs::remove_file(&program) {
        Ok(()) => println!("{PROGRAM}: removed {}", program.display()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("could not delete {}", program.display()));
        }
    }
    Ok(())
}

fn home_dir() -> Result<PathBuf> {
    std::env::home_dir().context("could not find the home directory")
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

fn animate(characters: &[char]) -> io::Result<()> {
    let (width, height) = terminal::size()?;
    let mut screen = Screen::new(width, height, characters);
    let mut stdout = io::stdout().lock();
    loop {
        let next_frame = Instant::now() + FRAME_TIME;
        screen.advance();
        screen.draw(&mut stdout)?;
        while event::poll(next_frame.saturating_duration_since(Instant::now()))? {
            match event::read()? {
                event::Event::Resize(width, height) => {
                    screen = Screen::new(width, height, characters);
                }
                event if event.is_key_press() => return Ok(()),
                _ => {}
            }
        }
    }
}

struct Screen {
    rows: u16,
    characters: Vec<char>,
    columns: Vec<Column>,
    /// The cells of the last frame, row by row.
    drawn: Vec<Option<(char, u8)>>,
}

impl Screen {
    fn new(width: u16, height: u16, characters: &[char]) -> Self {
        // The last column needs both cells of a font character on the screen
        let columns = (0..width.saturating_sub(1))
            .step_by(COLUMN_SPACING)
            .map(|x| Column::new(x, height, characters))
            .collect();
        Self {
            rows: height,
            characters: characters.to_vec(),
            columns,
            drawn: Vec::new(),
        }
    }

    fn advance(&mut self) {
        for column in &mut self.columns {
            column.advance(self.rows, &self.characters);
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
                        // Terminals give a character of the bundled font two
                        // cells, and some only clear both for two spaces
                        None => queue!(out, Print("  "))?,
                        Some((character, color)) => {
                            let weight = if color == CURSOR_COLOR {
                                Attribute::Bold
                            } else {
                                Attribute::NormalIntensity
                            };
                            queue!(
                                out,
                                SetAttribute(weight),
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

    fn advance(&mut self, rows: u16, characters: &[char]) {
        let height = f32::from(rows);
        for trail in &mut self.trails {
            trail.cursor_row += self.speed;
        }
        // A trail is gone once its top has left the screen
        self.trails.retain(|trail| trail.top_row() < height);

        self.rows_until_next_trail -= self.speed;
        if self.rows_until_next_trail <= 0.0 {
            self.trails.push(Trail {
                cursor_row: 0.0,
                length: (random_between(SHORTEST_TRAIL, LONGEST_TRAIL) * height) as u16,
            });
            self.rows_until_next_trail = random_between(SHORTEST_GAP, LONGEST_GAP) * height;
        }

        for row in 0..rows {
            if self.color(row).is_some() && fastrand::f32() < CHARACTER_CHANGE_CHANCE {
                self.characters[usize::from(row)] = random_character(characters);
            }
        }
    }

    /// The character and color in a row, or `None` for an empty cell.
    fn cell(&self, row: u16) -> Option<(char, u8)> {
        let color = self.color(row)?;
        Some((self.characters[usize::from(row)], color))
    }

    /// The newest trail at or below a row passed it last and decides its color.
    fn color(&self, row: u16) -> Option<u8> {
        let trail = self
            .trails
            .iter()
            .rev()
            .find(|trail| trail.cursor_row as u16 >= row)?;
        trail_color(trail.cursor_row as u16 - row, trail.length)
    }
}

struct Trail {
    cursor_row: f32,
    length: u16,
}

impl Trail {
    fn top_row(&self) -> f32 {
        self.cursor_row - f32::from(self.length)
    }
}

/// Color of a cell `distance` rows above the cursor of a trail.
fn trail_color(distance: u16, trail_length: u16) -> Option<u8> {
    if distance == 0 {
        return Some(CURSOR_COLOR);
    }
    if distance > trail_length {
        return None;
    }
    let index = usize::from(distance - 1) * TRAIL_COLORS.len() / usize::from(trail_length);
    Some(TRAIL_COLORS[index])
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
    fn trail_fades_from_cursor_to_dark() {
        assert_eq!(trail_color(0, 14), Some(CURSOR_COLOR));
        assert_eq!(trail_color(1, 14), Some(TRAIL_COLORS[0]));
        assert_eq!(trail_color(14, 14), Some(TRAIL_COLORS[6]));
        assert_eq!(trail_color(15, 14), None);
    }

    #[test]
    fn every_trail_length_stays_inside_the_colors() {
        for trail_length in 1..100 {
            for distance in 1..=trail_length {
                assert!(trail_color(distance, trail_length).is_some());
            }
        }
    }

    #[test]
    fn trails_never_overtake_each_other() {
        let mut column = Column::new(0, 40, &['a']);
        for _ in 0..500 {
            column.advance(40, &['a']);
            for pair in column.trails.windows(2) {
                assert!(pair[0].cursor_row > pair[1].cursor_row);
            }
        }
    }

    #[test]
    fn trails_leave_once_their_top_is_off_screen() {
        let mut column = Column::new(0, 10, &['a']);
        column.speed = 0.5;
        column.rows_until_next_trail = 100.0;
        column.trails = vec![
            Trail {
                cursor_row: 12.6,
                length: 3,
            },
            Trail {
                cursor_row: 12.4,
                length: 3,
            },
        ];
        column.advance(10, &['a']);
        assert_eq!(column.trails.len(), 1);
        assert!(column.trails[0].cursor_row < 13.0);
    }

    #[test]
    fn columns_leave_an_empty_cell_between_them() {
        let screen = Screen::new(6, 3, &['a']);
        let xs: Vec<u16> = screen.columns.iter().map(|column| column.x).collect();
        assert_eq!(xs, [0, 2, 4]);
    }

    #[test]
    fn a_frame_without_changes_draws_nothing() {
        let mut screen = Screen::new(6, 3, &['a']);
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

    #[test]
    fn the_last_column_keeps_two_cells_on_screen() {
        let screen = Screen::new(5, 3, &['a']);
        let xs: Vec<u16> = screen.columns.iter().map(|column| column.x).collect();
        assert_eq!(xs, [0, 2]);
    }
}
