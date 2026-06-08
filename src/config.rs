use clap::Parser;

use super::MatrixColor;

#[derive(Debug, Parser)]
#[command(version, about)]
/// Shows a scrolling 'Matrix' like screen in your terminal
struct Opt {
    #[arg(short, action=clap::ArgAction::Count)]
    /// Bold characters on
    bold: u8,

    #[arg(short, long)]
    /// Linux mode (use matrix console font)
    console: bool,

    #[arg(short, long)]
    /// Use old-style scrolling
    oldstyle: bool,

    #[arg(short, long)]
    /// "Screensaver" mode, exits on first keystroke
    screensaver: bool,

    #[arg(short, long)]
    /// X window mode, use if your xterm is using mtx.pcf
    xwindow: bool,

    #[arg(short, long, default_value = "4", value_parser = clap::value_parser!(u8).range(1..11))]
    /// Screen update delay
    update: u8,

    #[arg(short = 'C', long, value_enum, default_value = "green")]
    colour: MatrixColor,

    #[arg(short, long)]
    /// Rainbow mode
    rainbow: bool,
}

/// The global state object
pub struct Config {
    pub bold: u8,
    pub console: bool,
    pub oldstyle: bool,
    pub screensaver: bool,
    pub xwindow: bool,
    pub update: u8,
    pub colour: MatrixColor,
    pub rainbow: bool,
    pub pause: bool,
}

impl Config {
    /// Get the new config object based on command line arguments
    pub fn from_opts() -> Self {
        let opt = Opt::parse();

        Config {
            bold: opt.bold,
            console: opt.console,
            oldstyle: opt.oldstyle,
            screensaver: opt.screensaver,
            xwindow: opt.xwindow,
            update: opt.update,
            rainbow: opt.rainbow,
            colour: opt.colour,
            pause: false,
        }
    }
}

impl Config {
    /// Update the config based on any keypresses
    pub fn handle_keypress(&mut self, keypress: char) -> bool {
        // Exit if in screensaver mode
        if self.screensaver {
            return true;
        }

        match keypress {
            'q' => return true,
            'b' => self.bold = 1,
            'B' => self.bold = 2,
            'n' => self.bold = 0,
            '!' => {
                self.colour = MatrixColor::Red;
                self.rainbow = false;
            }
            '@' => {
                self.colour = MatrixColor::Green;
                self.rainbow = false;
            }
            '#' => {
                self.colour = MatrixColor::Yellow;
                self.rainbow = false;
            }
            '$' => {
                self.colour = MatrixColor::Blue;
                self.rainbow = false;
            }
            '%' => {
                self.colour = MatrixColor::Magenta;
                self.rainbow = false;
            }
            'r' => {
                self.rainbow = true;
            }
            '^' => {
                self.colour = MatrixColor::Cyan;
                self.rainbow = false;
            }
            '&' => {
                self.colour = MatrixColor::White;
                self.rainbow = false;
            }
            'p' | 'P' => self.pause = !self.pause,
            '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' | '0' => {
                self.update = keypress as u8 - 48 // Sneaky way to avoid parsing
            }
            _ => {}
        }
        false
    }
}
