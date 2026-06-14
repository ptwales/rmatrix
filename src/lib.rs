extern crate clap;
extern crate crossterm;
extern crate rand;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{self, Stdout, Write};
use std::ops::Range;
use std::time::Duration;

pub mod config;

use config::Config;

use clap::ValueEnum;
use crossterm::cursor;
use crossterm::event::{self, Event};
use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use rand::RngExt;
use rand::rngs::SmallRng;

thread_local! {
    static RNG: RefCell<SmallRng> = RefCell::new(rand::make_rng());
}

fn random_range(range: Range<usize>) -> usize {
    RNG.with_borrow_mut(|rng| rng.random_range(range))
}

fn rand_char() -> char {
    let (randnum, randmin) = (93, 33);
    RNG.with_borrow_mut(|rng| rng.random::<u8>() % randnum + randmin) as char
}

fn coin_flip() -> bool {
    RNG.with_borrow_mut(|rng| rng.random())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum MatrixColor {
    Black,
    Green,
    White,
    Red,
    Cyan,
    Magenta,
    Blue,
    Yellow,
}

pub enum FontWeight {
    Regular,
    SemiBold, // sometimes bold
    Bold,
}

#[derive(Clone)]
pub struct Block {
    val: char,
    white: bool,
    color: MatrixColor,
    bold: bool,
}

impl Block {
    fn is_space(&self) -> bool {
        self.val == ' '
    }

    pub fn term_color(&self) -> Color {
        if self.white {
            Color::White
        } else if self.bold {
            match self.color {
                MatrixColor::Black => Color::DarkGrey,
                MatrixColor::Green => Color::Green,
                MatrixColor::White => Color::White,
                MatrixColor::Red => Color::Red,
                MatrixColor::Cyan => Color::Cyan,
                MatrixColor::Magenta => Color::Magenta,
                MatrixColor::Blue => Color::Blue,
                MatrixColor::Yellow => Color::Yellow,
            }
        } else {
            match self.color {
                MatrixColor::Black => Color::Black,
                MatrixColor::Green => Color::DarkGreen,
                MatrixColor::White => Color::Grey,
                MatrixColor::Red => Color::DarkRed,
                MatrixColor::Cyan => Color::DarkCyan,
                MatrixColor::Magenta => Color::DarkMagenta,
                MatrixColor::Blue => Color::DarkBlue,
                MatrixColor::Yellow => Color::DarkYellow,
            }
        }
    }
}

impl Default for Block {
    fn default() -> Self {
        Block {
            val: ' ',
            white: false,
            color: MatrixColor::Red,
            bold: false,
        }
    }
}

static MAX_DELAY: u8 = 3;

pub struct Column {
    length: usize,         // The length of the stream
    spaces: usize,         // The spaces between streams
    update_rate: u8,       // The steps between updates (only enabled in async)
    rows: VecDeque<Block>, // The actual column
}

impl Column {
    /// Return a column keyed by a random number generator
    fn new(lines: usize) -> Self {
        Column {
            length: random_range(3..lines),
            spaces: random_range(1..lines + 1),
            update_rate: random_range(1..(MAX_DELAY as usize)) as u8,
            rows: (0..lines).map(|_| Block::default()).collect(),
        }
    }

    fn head_is_empty(&self) -> bool {
        self.rows[1].val == ' '
    }

    fn new_rand_char(&mut self) {
        self.rows[0].val = rand_char();
        self.rows[0].color = self.rows[1].color;
    }

    fn new_rand_head(&mut self, config: &Config) {
        self.rows[0].val = rand_char();
        self.rows[0].color = if config.rainbow {
            match random_range(0..6) {
                0 => MatrixColor::Green,
                1 => MatrixColor::Blue,
                2 => MatrixColor::White,
                3 => MatrixColor::Yellow,
                4 => MatrixColor::Cyan,
                5 => MatrixColor::Magenta,
                _ => unreachable!(),
            }
        } else {
            config.colour
        };
        // 50/50 chance the head is white
        self.rows[0].white = coin_flip();
    }

    fn old_move_down(&mut self) {
        self.rows.pop_back();
        self.rows.push_back(Block::default()); // Put a Blank space at the head.
        self.rows.rotate_right(1);
    }

    fn new_move_down(&mut self, config: &Config) {
        // Reset for each column
        let mut in_stream = false;

        let mut last_was_white = false; // Keep track of white heads
        let mut running_color = MatrixColor::Cyan;

        for block in self.rows.iter_mut() {
            if !in_stream {
                if !block.is_space() {
                    block.val = ' ';
                    in_stream = true; // We're now in a stream
                    running_color = block.color;
                }
            } else if block.is_space() {
                // New rand char for head of stream
                block.val = rand_char();
                block.white = last_was_white;
                in_stream = false;
                block.bold = match config.bold {
                    FontWeight::Regular => false,
                    FontWeight::Bold => true,
                    FontWeight::SemiBold => coin_flip(),
                };
            }
            // Swapped to "pass on" whiteness and prepare the variable for the next iteration
            std::mem::swap(&mut last_was_white, &mut block.white);
            block.color = running_color;
        }
    }
}

pub struct Matrix {
    columns: Vec<Column>,
    update_count: u8,
    lines: usize,
}

impl Default for Matrix {
    /// Create a new matrix with the dimensions of the screen
    fn default() -> Self {
        // Get the screen dimensions
        let (lines, cols) = get_term_size();
        let columns = (0..cols).map(|_| Column::new(lines)).collect();

        // Create the matrix
        Matrix {
            columns,
            update_count: 0,
            lines,
        }
    }
}

impl Matrix {
    /// Make the next iteration of matrix
    pub fn arrange(&mut self, config: &Config) {
        if self.update_count <= MAX_DELAY {
            self.update_count += 1;
        } else {
            self.update_count = 1;
        }

        for col in self.columns.iter_mut() {
            if col.head_is_empty() && col.spaces != 0 {
                // Decrement the spaces until the next stream starts
                col.spaces -= 1;
            } else if col.head_is_empty() && col.spaces == 0 {
                // Start a new stream
                col.new_rand_head(config);

                // Decrement length of stream
                col.length -= 1;

                // Reset number of spaces until next stream
                col.spaces = random_range(1..self.lines + 1);
            } else if col.length != 0 {
                // Continue producing stream
                col.new_rand_char();
                col.length -= 1;
            } else {
                // Display spaces until next stream
                col.rows[0].val = ' ';
                col.length = random_range(3..self.lines);
            }

            if !config.asynch || self.update_count > col.update_rate {
                if config.oldstyle {
                    col.old_move_down();
                } else {
                    col.new_move_down(config);
                }
            }
        }
    }

    /// Draw the matrix on the screen
    pub fn draw(&self, terminal: &mut Terminal) -> io::Result<()> {
        let stdout = &mut terminal.stdout;

        let mut last_colour = self.columns[0].rows[0].term_color();

        for (x, col) in self.columns.iter().enumerate() {
            for (y, block) in col.rows.iter().skip(1).enumerate() {
                let colour = block.term_color();
                queue!(stdout, cursor::MoveTo(2 * x as u16, y as u16))?; // Move the cursor
                if colour != last_colour {
                    queue!(stdout, SetForegroundColor(colour))?;
                    last_colour = colour;
                }
                queue!(stdout, Print(block.val))?;
            }
        }
        stdout.flush()?;
        Ok(())
    }
}

/// Terminal state object
pub struct Terminal {
    stdout: Stdout,
    active: bool,
}

impl Terminal {
    /// Set up the screen and set important variables
    pub fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;

        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, cursor::Hide, Clear(ClearType::All)) {
            let _ = terminal::disable_raw_mode();
            return Err(error);
        }

        Ok(Terminal {
            stdout,
            active: true,
        })
    }

    /// Return the next terminal event, if one is ready
    pub fn get_event(&self, timeout: Duration) -> io::Result<Option<Event>> {
        if event::poll(timeout)? {
            return Ok(Some(event::read()?));
        }
        Ok(None)
    }

    /// Clean up terminal stuff when we're ready to exit
    pub fn finish(mut self) -> io::Result<()> {
        self.restore()
    }

    /// Clear the terminal when the window changes size
    pub fn resize_window(&mut self) -> io::Result<()> {
        execute!(self.stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        self.stdout.flush()
    }

    fn restore(&mut self) -> io::Result<()> {
        if self.active {
            let terminal_result = execute!(self.stdout, ResetColor, cursor::Show);
            let raw_mode_result = terminal::disable_raw_mode();
            self.active = false;
            terminal_result?;
            raw_mode_result?;
        }
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn get_term_size() -> (usize, usize) {
    match terminal::size() {
        Ok((mut width, mut height)) => {
            // Minimum size for terminal
            if width < 10 {
                width = 10
            }
            if height < 10 {
                height = 10
            }
            if width % 2 != 0 {
                // Makes odd-columned screens print on the rightmost edge
                ((height + 1) as usize, (width / 2 + 1) as usize)
            } else {
                ((height + 1) as usize, (width / 2) as usize)
            }
        }
        Err(_) => (10, 10),
    }
}
