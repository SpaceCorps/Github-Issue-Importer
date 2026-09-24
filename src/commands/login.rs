//! `gh-issue-import login`. Authenticates with GitHub via Personal Access Token,
//! opening the GitHub tokens creation page in the browser if interactive, verifying against `/user`,
//! and storing the token in the OS keystore.

use std::io::{BufRead, IsTerminal, Write};

use crate::account::identity;
use crate::cli::LoginArgs;
use crate::client::Client;
use crate::commands::print;
use crate::config::{self, AccountConfig};
use crate::error::{Error, Result};
use crate::obj;
use crate::secrets;

const GITHUB_TOKENS_URL: &str = "https://github.com/settings/tokens/new?scopes=repo&description=gh-issue-import";

pub fn run(args: LoginArgs) -> Result<()> {
    let LoginArgs { name, token, api_key_stdin, no_browser, force, no_verify } = args;

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
        return Err(Error::invalid(format!("An account named '{existing}' already exists."))
            .fix(format!("Use --force to replace its token: gh-issue-import login {existing} --force")));
    }
    let name = existing.clone().unwrap_or(name);

    let token = if api_key_stdin {
        read_stdin_token()?
    } else if let Some(t) = token.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
        t
    } else {
        prompt_login_token(&name, no_browser)?
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

    if std::io::stderr().is_terminal() {
        if !user_login.is_empty() {
            eprintln!("Successfully logged in as {user_login} to account '{name}'.");
        } else {
            eprintln!("Successfully logged in to account '{name}'.");
        }
    }

    print(obj! {
        "status" => "logged_in",
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

fn prompt_login_token(name: &str, no_browser: bool) -> Result<String> {
    if !std::io::stdin().is_terminal() {
        return Err(Error::invalid("No token given and no terminal to prompt on.")
            .fix(format!("pbpaste | gh-issue-import login {name} --api-key-stdin")));
    }

    eprintln!("To log in, create a GitHub Personal Access Token with 'repo' scope:");
    eprintln!("  {GITHUB_TOKENS_URL}\n");

    if !no_browser {
        eprintln!("Opening {GITHUB_TOKENS_URL} in your browser...");
        open_browser(GITHUB_TOKENS_URL);
    }

    let _ = std::io::stderr().flush();

    loop {
        let token = rpassword::prompt_password(format!("Paste your GitHub Personal Access Token for '{name}': "))
            .map_err(|e| Error::other("Could not read the token.").detail(e.to_string()))?;
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
        eprintln!("Token cannot be empty. Paste your Personal Access Token from {GITHUB_TOKENS_URL}");
    }
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}
