use std::fmt::Debug;

use colored::{ColoredString, Colorize};
use serde::Serialize;
use thiserror::Error;

static mut LOG: Output = Output::new(false, Verbosity::Trace);

pub fn init(stdout_json: bool, verbosity: Verbosity) {
    unsafe { LOG = Output::new(stdout_json, verbosity) }
}

fn get_verbosity() -> Verbosity {
    unsafe { LOG.verbosity }
}

fn output_as_json() -> bool {
    unsafe { LOG.stdout_json }
}

macro_rules! warning {
    ($($arg:tt)*) => {{
        let out = format!("WARN:\t{}", format_args!($($arg)*));
        $crate::logging::warn_str(&out)
    }};
}
pub(crate) use warning;

pub fn warn_str(str: &str) {
    if Verbosity::Quiet != get_verbosity() {
        eprintln!("{}", str.dimmed())
    }
}

macro_rules! info {
    ($($arg:tt)*) => {{
        let out = format!("INFO:\t{}", format_args!($($arg)*));
        $crate::logging::info_str(&out)
    }};
}
pub(crate) use info;

pub fn info_str(str: &str) {
    match get_verbosity() {
        Verbosity::Info | Verbosity::Debug | Verbosity::Trace => {
            eprintln!("{}", str.dimmed())
        }
        _ => {}
    }
}

macro_rules! debug {
    ($($arg:tt)*) => {{
        let out = format!("DEBUG:\t{}", format_args!($($arg)*));
        $crate::logging::debug_str(&out)
    }};
}
pub(crate) use debug;

pub fn debug_str(str: &str) {
    match get_verbosity() {
        Verbosity::Debug | Verbosity::Trace => {
            eprintln!("{}", str.dimmed())
        }
        _ => {}
    }
}

macro_rules! trace {
    ($($arg:tt)*) => {{
        let out = format!("TRACE:\t{}", format_args!($($arg)*));
        $crate::logging::trace_str(&out)
    }};
}
pub(crate) use trace;

pub fn trace_str(str: &str) {
    if get_verbosity() == Verbosity::Trace {
        eprintln!("{}", str.dimmed())
    }
}

pub fn println<T: Message>(out: T) {
    println!("{}", out.standard(get_verbosity()));
}

#[derive(Debug, Error)]
pub enum StdoutError {
    #[error("Couldn't serialize stdout: {source}")]
    JSONConversionException { source: serde_json::Error },
}

pub fn stdoutln<T: Stdout>(out: T) -> Result<(), StdoutError> {
    if output_as_json() {
        let out_str = serde_json::to_string(&out)
            .map_err(|source| StdoutError::JSONConversionException { source })?;
        println!("{}", out_str);
    } else {
        println!("{}", out.standard(get_verbosity()));
    }
    Ok(())
}

pub fn println_stderr(msg: &str) {
    eprintln!("{}", msg);
}

pub fn err(err: anyhow::Error) {
    match get_verbosity() {
        Verbosity::Quiet | Verbosity::Normal => {
            eprintln!("{}", err);
        }
        Verbosity::Info | Verbosity::Debug | Verbosity::Trace => {
            eprintln!("{:?}", err);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Verbosity {
    Normal,
    Quiet,
    Info,
    Debug,
    Trace,
}

impl Verbosity {
    pub fn new(verbosity: u8, quiet: bool) -> Self {
        if quiet {
            return Verbosity::Quiet;
        }
        match verbosity {
            0 => Verbosity::Normal,
            1 => Verbosity::Info,
            2 => Verbosity::Debug,
            x if x >= 3 => Verbosity::Trace,
            _ => {
                unreachable!()
            }
        }
    }
}

#[derive(Clone)]
struct Output {
    verbosity: Verbosity,
    stdout_json: bool,
}

impl Output {
    const fn new(stdout_json: bool, verbosity: Verbosity) -> Self {
        Output {
            stdout_json,
            verbosity,
        }
    }
}

pub trait Message {
    fn standard(&self, verbosity: Verbosity) -> ColoredString;
}

pub trait Stdout: Serialize + Message {}

#[derive(Serialize)]
pub struct Msg(pub String);

impl Message for Msg {
    fn standard(&self, _: Verbosity) -> ColoredString {
        ColoredString::from(self.0.clone().as_str())
    }
}

pub struct Empty;

impl Message for Empty {
    fn standard(&self, _: Verbosity) -> ColoredString {
        ColoredString::from("")
    }
}
