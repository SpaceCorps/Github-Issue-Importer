//! `gh-issue-import` - native Rust CLI for importing GitHub issues from YAML/JSON files and remote repos.

mod account;
mod cli;
mod client;
mod commands;
mod config;
mod error;
mod importer;
mod output;
mod readme;
mod secrets;

use std::process::ExitCode;

use clap::Parser;
use clap::error::ErrorKind;

use crate::error::Error;

fn main() -> ExitCode {
    output::set_json(std::env::args_os().skip(1).take_while(|a| a != "--").any(|a| a == "--json"));

    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => return parse_error(e),
    };
    output::set_json(cli.json);

    match commands::run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => ExitCode::from(output::write_error(&e) as u8),
    }
}

fn parse_error(e: clap::Error) -> ExitCode {
    match e.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            let _ = e.print();
            ExitCode::SUCCESS
        }
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            let _ = e.print();
            ExitCode::from(error::ErrorCode::InvalidInput as u8)
        }
        _ => {
            let rendered = e.render().to_string();
            let (what, usage) = rendered.split_once("\n\n").unwrap_or((&rendered, ""));
            let message = what
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
                .trim_start_matches("error: ")
                .to_string();
            let mut err = Error::invalid(if message.is_empty() { "Invalid arguments.".into() } else { message });
            if !usage.trim().is_empty() {
                err = err.detail(usage.trim());
            }
            ExitCode::from(output::write_error(&err) as u8)
        }
    }
}
