//! `accounts add|list|test|remove`. Manages stored GitHub accounts and credentials.

use std::io::{BufRead, IsTerminal, Write};

use serde_json::Value;

use crate::account::{self, identity};
use crate::cli::AccountsCommand;
use crate::client::Client;
use crate::commands::print;
use crate::config::{self, AccountConfig};
use crate::error::{Error, ErrorCode, Result};
use crate::obj;
use crate::secrets::{self, Store};

pub fn run(c: AccountsCommand) -> Result<()> {
    match c {
        AccountsCommand::Add { name, token, api_key_stdin, force, no_verify } => {
            let token = if api_key_stdin { Some(read_stdin_token()?) } else { token };
            add(name, token, force, no_verify)
        }
        AccountsCommand::List { check } => list(check),
        AccountsCommand::Test { name } => test(&name),
        AccountsCommand::Remove { name, yes } => remove(&name, yes),
    }
}

fn add(name: String, token: Option<String>, force: bool, no_verify: bool) -> Result<()> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(Error::invalid("An account name is required."));
    }

    let store = secrets::store()?;
    let config = config::load()?;

    let existing = config.find(&name).map(|(k, _)| k.clone());
    if let Some(existing) = &existing
        && !force
    {
        return Err(Error::invalid(format!("An account named '{existing}' already exists.")).fix(format!(
            "Pick a different name, or replace its token: gh-issue-import accounts add {existing} --token <token> --force"
        )));
    }
    let name = existing.clone().unwrap_or(name);

    let token = match token.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
        Some(t) => t,
        None => prompt_token(&name)?,
    };

    let (mut user_login, mut display_name, mut email) = (String::new(), String::new(), String::new());
    if !no_verify {
        let user = Client::new(&token).user()?;
        user_login = identity::user(&user);
        display_name = identity::name(&user);
        email = identity::email(&user);
    }

    {
        let _lock = config::lock()?;
        store.set(&secrets::account_key(&name), &token)?;

        let mut config = config::load()?;
        config.accounts.insert(
            name.clone(),
            AccountConfig {
                user: user_login.clone(),
                name: display_name.clone(),
                email: email.clone(),
                added_at: config::now_utc(),
            },
        );
        config::save(&config)?;
    }

    print(obj! {
        "status" => if existing.is_none() { "added" } else { "replaced" },
        "name" => name,
        "user" => user_login,
        "displayName" => display_name,
        "email" => email,
        "verified" => !no_verify,
        "secretStore" => store.name(),
        "configDir" => config::config_dir().display().to_string(),
        "nextStep" => format!("gh-issue-import import issues.yaml --repo owner/repo --account {name}"),
    })
}

fn read_stdin_token() -> Result<String> {
    let mut token = String::new();
    std::io::stdin().lock().read_line(&mut token).map_err(|e| Error::invalid(format!("Could not read stdin: {e}")))?;
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err(Error::invalid("--api-key-stdin was given but stdin was empty."));
    }
    Ok(token)
}

fn prompt_token(name: &str) -> Result<String> {
    if !std::io::stdin().is_terminal() {
        return Err(Error::invalid("No token given and no terminal to prompt on.")
            .fix(format!("pbpaste | gh-issue-import accounts add {name} --api-key-stdin")));
    }
    loop {
        let token = rpassword::prompt_password(format!("GitHub Personal Access Token for {name}: "))
            .map_err(|e| Error::other("Could not read the token.").detail(e.to_string()))?;
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
        eprintln!("Cannot be empty");
    }
}

fn list(check: bool) -> Result<()> {
    let store = secrets::store()?;
    let config = config::load()?;
    let sorted = config.sorted();

    let statuses: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> =
            sorted.iter().map(|(name, acct)| scope.spawn(move || status_of(name, acct, store, check))).collect();
        handles.into_iter().map(|h| h.join().unwrap_or_else(|_| "unreachable".into())).collect()
    });

    let accounts: Vec<Value> = sorted
        .iter()
        .zip(statuses)
        .map(|((name, a), status)| {
            obj! {
                "name" => name,
                "user" => a.user,
                "displayName" => a.name,
                "email" => a.email,
                "addedAt" => a.added_at,
                "tokenStatus" => status,
            }
        })
        .collect();

    print(obj! {
        "count" => accounts.len(),
        "accounts" => accounts,
        "secretStore" => store.name(),
        "configDir" => config::config_dir().display().to_string(),
    })
}

fn status_of(name: &str, _acct: &AccountConfig, store: Store, check: bool) -> String {
    let token = match store.get(&secrets::account_key(name)) {
        Ok(Some(k)) if !k.trim().is_empty() => k,
        Ok(_) => return "missing_token".into(),
        Err(_) => return "unreadable".into(),
    };
    if !check {
        return "stored".into();
    }
    match Client::new(&token).user() {
        Ok(_) => "valid".into(),
        Err(e) if e.code == ErrorCode::AuthRequired => "rejected".into(),
        Err(_) => "unreachable".into(),
    }
}

fn test(name: &str) -> Result<()> {
    let account = account::resolve(Some(name))?;
    let user = account.client().user()?;
    let user_login = identity::user(&user);
    let display_name = identity::name(&user);
    let email = identity::email(&user);

    let mut result = obj! {
        "name" => account.name,
        "user" => user_login,
        "displayName" => display_name,
        "email" => email,
        "tokenStatus" => "valid",
        "githubUser" => user,
    };

    if !account.config.user.trim().is_empty()
        && !user_login.trim().is_empty()
        && !user_login.eq_ignore_ascii_case(&account.config.user)
    {
        result["warning"] = Value::String(format!(
            "Account was registered as '{}', but the stored token now authenticates as '{}'.",
            account.config.user, user_login
        ));
    }

    print(result)
}

fn remove(requested: &str, yes: bool) -> Result<()> {
    let store = secrets::store()?;
    let config = config::load()?;

    let Some((name, acct)) = config.find(requested) else {
        return Err(Error::new(ErrorCode::NoAccount, format!("No account named '{requested}'."))
            .fix("gh-issue-import accounts list"));
    };
    let (name, acct) = (name.clone(), acct.clone());

    if !yes {
        if !std::io::stdin().is_terminal() {
            return Err(Error::invalid(format!(
                "Removing '{name}' needs confirmation and there is no terminal to ask on."
            ))
            .fix(format!("gh-issue-import accounts remove {name} --yes")));
        }
        let label = if acct.user.trim().is_empty() { name.clone() } else { format!("{name} ({})", acct.user) };
        eprint!("Remove account {label}? [y/N] ");
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        let _ = std::io::stdin().lock().read_line(&mut answer);
        if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes") {
            return Err(Error::invalid("Cancelled."));
        }
    }

    {
        let _lock = config::lock()?;
        store.delete(&secrets::account_key(&name))?;
        let mut config = config::load()?;
        config.accounts.shift_remove(&name);
        config::save(&config)?;
    }

    print(obj! {
        "status" => "removed",
        "name" => name,
        "user" => acct.user,
        "note" => "The token was deleted locally. Revoke it at https://github.com/settings/tokens if desired.",
    })
}
