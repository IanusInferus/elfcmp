mod cli;
mod elf;
mod manifest;
mod ops;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Copy(args) => ops::copy::run(args),
        Command::Map(args) => ops::map::run(args),
        Command::Check(args) => ops::check::run(args),
        Command::Patch(args) => ops::patch::run(args),
    }
}
