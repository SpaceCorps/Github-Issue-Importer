//! Command dispatcher for gh-issue-import.

pub mod accounts;
pub mod import;
pub mod login;
pub mod validate;

use serde_json::Value;

use crate::account;
use crate::cli::Command;
use crate::error::Result;
use crate::output;
use crate::readme;

pub fn run(cmd: Command) -> Result<()> {
    match cmd {
        Command::Import(args) => import::run(args),
        Command::Validate(args) => validate::run(args),
        Command::Me(acc) => {
            let resolved = account::resolve(acc.account.as_deref())?;
            let user = resolved.client().user()?;
            print(user)
        }
        Command::Login(args) => login::run(args),
        Command::Accounts(cmd) => accounts::run(cmd),
        Command::AgentReadme => {
            readme::print();
            Ok(())
        }
    }
}

pub fn print(value: Value) -> Result<()> {
    output::write(&value);
    Ok(())
}
