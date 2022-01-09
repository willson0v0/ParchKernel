
#![allow(unused)]

use super::UART0;
use core::fmt::{self, Write};

// ======================== color constants ========================
const FG_BLACK      :u8 = 30;
const FG_RED        :u8 = 31;
const FG_GREEN      :u8 = 32;
const FG_YELLOW     :u8 = 33;
const FG_BLUE       :u8 = 34;
const FG_MAGENTA    :u8 = 35;
const FG_CYAN       :u8 = 36;
const FG_WHITE      :u8 = 37;

const FG_B_BLACK    :u8 = 90;
const FG_B_RED      :u8 = 91;
const FG_B_GREEN    :u8 = 92;
const FG_B_YELLOW   :u8 = 93;
const FG_B_BLUE     :u8 = 94;
const FG_B_MAGENTA  :u8 = 95;
const FG_B_CYAN     :u8 = 96;
const FG_B_WHITE    :u8 = 97;

const FG_DEFAULT    :u8 = 39;

const BG_BLACK      :u8 = 40;
const BG_RED        :u8 = 41;
const BG_GREEN      :u8 = 42;
const BG_YELLOW     :u8 = 43;
const BG_BLUE       :u8 = 44;
const BG_MAGENTA    :u8 = 45;
const BG_CYAN       :u8 = 46;
const BG_WHITE      :u8 = 47;

const BG_B_BLACK    :u8 = 100;
const BG_B_RED      :u8 = 101;
const BG_B_GREEN    :u8 = 102;
const BG_B_YELLOW   :u8 = 103;
const BG_B_BLUE     :u8 = 104;
const BG_B_MAGENTA  :u8 = 105;
const BG_B_CYAN     :u8 = 106;
const BG_B_WHITE    :u8 = 107;

const BG_DEFAULT    :u8 = 49;

// ======================== functions ========================
pub fn k_puts(ch: &str) {
	UART0.write_synced(ch);
}

struct  OutputFormatter;

impl Write for OutputFormatter {
	fn write_str(&mut self, s: &str) -> fmt::Result {
		k_puts(s);
		Ok(())
	}
}

pub fn print(args: fmt::Arguments) {
	OutputFormatter.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::utils::print(format_args!($($arg)*));
    }
}

/// The great println! macro. Prints to the standard output. Also prints a linefeed (`\\n`, or U+000A).
#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    
    ($($arg:tt)*) => {
        $crate::print!("{}\n", format_args!($($arg)*))
    };
}