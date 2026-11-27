#![forbid(unsafe_code)]

//! Equaform CLI — offline audio renderer binary.
//!
//! Usage: equaform-cli <command> [options]
//! Commands: process, preset (list|show), help

use equaform_cli::run_cli;
use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let code = run_cli(
        std::env::args().skip(1),
        &mut io::stdout(),
        &mut io::stderr(),
    );
    ExitCode::from(code as u8)
}
