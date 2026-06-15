extern crate clap;
extern crate crossterm;
extern crate rand;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{self, Stdout, Write};
use std::ops::Range;
use std::time::Duration;

pub mod config;
mod net;

use config::Config;

use clap::ValueEnum;
use crossterm::cursor;
use crossterm::event::{self, Event};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use rand::RngExt;
use rand::rngs::SmallRng;

use crate::net::NetState;

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

    fn new_move_down(&mut self) {
        let mut in_stream = false; // Reset for each column
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
                // blocks always have an internal random "bold" state but 'B' and 'n' options ignore it.
                block.bold = coin_flip();
                block.color = running_color;
                in_stream = false; // now out of stream
            }
            // Swapped to "pass on" whiteness and prepare the variable for the next iteration
            std::mem::swap(&mut last_was_white, &mut block.white);
        }
    }
}

pub struct Matrix {
    columns: Vec<Column>,
    update_ticker: u8, // ticker for async scroll
    net_ticker: u8,    // ticker for network check
    lines: usize,
    net_state: Option<NetState>,
}

impl Matrix {
    /// Create a new matrix with the dimensions of the screen
    pub fn new(config: &Config) -> Self {
        // Get the screen dimensions
        let (lines, cols) = get_term_size();
        let columns = (0..cols).map(|_| Column::new(lines)).collect();
        let net_state = config.net_interface.clone().map(NetState::new);
        // Create the matrix
        Matrix {
            columns,
            update_ticker: 0,
            net_ticker: 0,
            lines,
            net_state,
        }
    }
    /// Make the next iteration of matrix
    pub fn arrange(&mut self, config: &Config) {
        if self.update_ticker <= MAX_DELAY {
            self.update_ticker += 1;
        } else {
            self.update_ticker = 1;
        }

        for col in self.columns.iter_mut() {
            if config.asynch && self.update_ticker <= col.update_rate {
                // skip this column for asynchronous effect
                continue;
            }

            if col.head_is_empty() {
                // Not in stream
                if col.spaces > 0 {
                    // Decrement the spaces until the next stream starts
                    col.spaces -= 1;
                } else {
                    // Start a new stream
                    col.new_rand_head(config);
                    col.length = random_range(3..self.lines);

                    // Decrement length of stream
                    col.length -= 1;

                    // Reset number of spaces until next stream
                    col.spaces = random_range(1..self.lines + 1);
                }
            } else {
                // In stream
                if col.length > 0 {
                    // Continue producing stream
                    col.new_rand_char();
                    col.length -= 1;
                } else {
                    // Display spaces until next stream
                    col.rows[0].val = ' ';
                    col.rows[0].color = config.colour;
                }
            }

            if config.oldstyle {
                col.old_move_down();
            } else {
                col.new_move_down();
            }
        }
    }

    /// Draw the matrix on the screen
    pub fn draw(&mut self, config: &Config, terminal: &mut Terminal) -> io::Result<()> {
        // check network and paint blocks for each packet
        if !config.rainbow
            && let Some(net) = &mut self.net_state
        {
            if self.net_ticker <= config.net_threshold {
                self.net_ticker += 1;
            } else {
                self.net_ticker = 0;
                let stats = net.poll();
                if stats.tx_packets > 0 {
                    self.paint_packets(stats.tx_packets, config.tx_color);
                }
                if stats.rx_packets > 0 {
                    self.paint_packets(stats.rx_packets, config.rx_color);
                }
            }
        }

        let stdout = &mut terminal.stdout;
        let first_block = &self.columns[0].rows[0];
        let mut last_bold = first_block.bold;
        let mut last_colour = first_block.term_color();
        // Clear last_color of previous frame
        queue!(stdout, SetForegroundColor(last_colour))?;
        // Global bold/no-bold
        match config.bold {
            FontWeight::Bold => queue!(stdout, SetAttribute(Attribute::Bold))?,
            FontWeight::Regular => queue!(stdout, SetAttribute(Attribute::NormalIntensity))?,
            _ => {}
        }

        for (x, col) in self.columns.iter().enumerate() {
            for (y, block) in col.rows.iter().skip(1).enumerate() {
                let colour = block.term_color();
                queue!(stdout, cursor::MoveTo(2 * x as u16, y as u16))?; // Move the cursor
                if colour != last_colour {
                    queue!(stdout, SetForegroundColor(colour))?;
                    last_colour = colour;
                }

                if let FontWeight::SemiBold = config.bold
                    && last_bold != block.bold
                {
                    if block.bold {
                        queue!(stdout, SetAttribute(Attribute::Bold))?;
                    } else {
                        queue!(stdout, SetAttribute(Attribute::NormalIntensity))?;
                    }
                    last_bold = block.bold;
                };
                // Draw the character.
                queue!(stdout, Print(block.val))?;
            }
        }
        stdout.flush()?;
        Ok(())
    }

    fn paint_packets(&mut self, packets: u64, color: MatrixColor) {
        // Nothing here guarantees we're painting an actual sream.
        let x = random_range(0..self.columns.len());
        let col = &mut self.columns[x];
        let y = random_range(1..(col.rows.len() - 1));
        for block in col.rows.iter_mut().skip(y).take(packets as usize) {
            block.color = color;
            block.bold = true;
        }
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
