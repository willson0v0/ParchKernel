
#![allow(unused)]

use alloc::{string::String, sync::Arc};

use crate::{process::{push_intr_off, pop_intr_off, get_hart_id, get_processor}, utils::time::{get_time, get_time_ms, get_time_second}, println, print, print_no_lock};

use super::{UART0, SpinMutex, Mutex};
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

use lazy_static::*;
lazy_static!{
    /// dummy data member
    static ref PRINT_LOCK: SpinMutex<bool> = SpinMutex::new("KPuts", false);
}

// ======================== functions ========================
pub fn k_puts(ch: &str) {
	UART0.write_str_synced(ch);
}

struct  OutputFormatter;

impl Write for OutputFormatter {
	fn write_str(&mut self, s: &str) -> fmt::Result {
		k_puts(s);
		Ok(())
	}
}

pub fn print(args: fmt::Arguments) {
    let guard = PRINT_LOCK.acquire();
	print_no_lock(args);
}

pub fn print_no_lock(args: fmt::Arguments) {
    OutputFormatter.write_fmt(args).unwrap();
}

#[derive(PartialEq, PartialOrd, Clone, Copy)]
pub enum LogLevel {
    Verbose = 0,
    Debug = 1,
    Info = 2,
    Warning = 3,
    Error = 4,
    Milestone = 5,
    Fatal = 6
}

impl LogLevel {
    pub fn to_num(&self) -> usize {
        *self as usize
    }
}

static LOG_FG_COLOURS: &'static [u8] = &[
    FG_B_BLACK,
    FG_DEFAULT,
    FG_B_WHITE,
    FG_B_YELLOW,
    FG_B_RED,
    FG_B_GREEN,
    FG_B_WHITE
];

static LOG_BG_COLOURS: &'static [u8] = &[
    BG_DEFAULT,
    BG_DEFAULT,
    BG_DEFAULT,
    BG_DEFAULT,
    BG_DEFAULT,
    BG_DEFAULT,
    BG_RED
];

static LOG_TITLE: &'static [&str] = &[
    "VERBOSE    ",
    "DEBUG      ",
    "INFO       ",
    "WARNING    ",
    "ERROR      ",
    "MILESTONE  ",
    "FATAL      ",
];

pub fn do_log(log_level: LogLevel, args: fmt::Arguments) {
    let guard = PRINT_LOCK.acquire();
    // print_no_lock!("\x1b[{};{}m{}", LOG_FG_COLOURS[log_level.to_num()], LOG_BG_COLOURS[log_level.to_num()], LOG_TITLE[log_level.to_num()]);
    // print_no_lock!("[{:>10.5}] on hart {}: ", get_time_second(), get_hart_id());
    // print_no_lock(args);
    // print_no_lock!("\x1b[{};{}m\r\n", FG_DEFAULT, BG_DEFAULT)
    print_no_lock!(
        "\x1b[{};{}m[ {:>8.5} ] {} h {} p {} : ", 
        LOG_FG_COLOURS[log_level.to_num()], 
        LOG_BG_COLOURS[log_level.to_num()], 
        get_time_second(),
        LOG_TITLE[log_level.to_num()],
        get_hart_id(),
        if let Some(proc) = get_processor().current() {
            proc.pid.0
        } else {
            0
        }
    );
    print_no_lock(args);
    print_no_lock!("\x1b[{};{}m\r\n", FG_DEFAULT, BG_DEFAULT)
}


pub fn log(log_level: LogLevel, args: fmt::Arguments) {
    match log_level {
        LogLevel::Verbose => {
            if cfg!(feature = "log_verbose") {
                do_log(log_level, args);
            }
        },
        LogLevel::Debug => {
            if cfg!(feature = "log_debug") {
                do_log(log_level, args);
            }
        },
        LogLevel::Info => {
            if cfg!(feature = "log_info") {
                do_log(log_level, args);
            }
        },
        LogLevel::Warning => {
            if cfg!(feature = "log_warning") {
                do_log(log_level, args);
            }
        },
        LogLevel::Error => {
            if cfg!(feature = "log_error") {
                do_log(log_level, args);
            }
        },
        LogLevel::Milestone => {
            if cfg!(feature = "log_milestone") {
                do_log(log_level, args);
            }
        },
        LogLevel::Fatal => {
            if cfg!(feature = "log_fatal") {
                do_log(log_level, args);
            }
        },
    }
}


pub fn get_char() -> char {
    super::UART0.read()
}

pub fn get_byte() -> u8 {
    super::UART0.read_byte()
}

pub fn get_line() -> String {
    let mut line =  String::new();

    /// hard limit
    while line.len() < 1024 {
        let c = get_char();
        if c == '\n' {
            return line;
        } else {
            line.push(c);
        }
    }

    line
}

pub fn k_get_char() -> char {
    super::UART0.read_synced()
}

pub fn k_get_byte() -> u8 {
    super::UART0.read_byte_synced()
}

pub fn k_get_line() -> String {
    let mut line =  String::new();

    /// hard limit
    while line.len() < 1024 {
        let c = k_get_char();
        if c == '\n' {
            return line;
        } else {
            line.push(c);
        }
    }

    line
}

pub fn get_term_size() -> (usize, usize) {
    print!("\x1b[s\x1b[999;999H\x1b[6n");
    k_get_byte(); // \x1b
    k_get_byte(); // '['
    let mut height = 0usize;
    loop {
        let b = k_get_byte();
        if b >= b'0' && b <= b'9' {
            height *= 10;
            height += (b - b'0') as usize;
        } else {
            break;
        }
    }
    let mut width = 0usize;
    loop {
        let b = k_get_byte();
        if b >= b'0' && b <= b'9' {
            width *= 10;
            width += (b - b'0') as usize;
        } else {
            break;
        }
    }
    
    print!("\x1b[u");
    (height.into(), width.into())
}