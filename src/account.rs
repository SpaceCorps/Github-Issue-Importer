//! The account is always explicit - no stored default, no implicit fallback when only one
//! account is configured.
//!
//! [`resolve`] is the only way a token enters the process.

use serde_json::Value;

use crate::client::Client;
use crate::config::{self, AccountConfig, Config};
use crate::error::{Error, ErrorCode, Result};
use crate::secrets;

pub struct Resolved {
    pub name: String,
    pub config: AccountConfig,
    pub token: String,
}

impl Resolved {
    pub fn client(&self) -> Client {
        Client::new(&self.token)
    }
}

pub fn resolve(requested: Option<&str>) -> Result<Resolved> {
    let config = config::load()?;

    let Some(requested) = requested.map(str::trim).filter(|s| !s.is_empty()) else {
        return Err(Error::new(ErrorCode::NoAccount, "No account specified. Pass --account <name>.")
            .detail(describe(&config))
            .fix("gh-issue-import accounts list"));
    };

    let Some((name, account)) = config.find(requested) else {
        return Err(Error::new(ErrorCode::NoAccount, format!("No account named '{requested}'."))
            .detail(describe(&config))
            .fix("gh-issue-import accounts list"));
    };

    let key = secrets::store()?.get(&secrets::account_key(name))?;

    let Some(token) = key.filter(|k| !k.trim().is_empty()) else {
        return Err(Error::new(ErrorCode::AuthRequired, format!("Account '{name}' has no stored token."))
            .detail("The config entry exists but the keystore has nothing under it.")
            .fix(format!("gh-issue-import accounts add {name} --token <token>")));
    };

    Ok(Resolved { name: name.clone(), config: account.clone(), token })
}

fn describe(config: &Config) -> String {
    if config.accounts.is_empty() {
        return "No accounts are configured yet. Run 'gh-issue-import accounts add <name> --token <token>'.".into();
    }
    let listed: Vec<String> = config
        .sorted()
        .into_iter()
        .map(|(k, v)| if v.user.trim().is_empty() { k.clone() } else { format!("{k} ({})", v.user) })
        .collect();
    format!("Configured accounts: {}", listed.join(", "))
}

pub mod identity {
    use super::Value;

    pub fn user(me: &Value) -> String {
        me.get("login")
            .and_then(Value::as_str)
            .or_else(|| me.get("user").and_then(Value::as_str))
            .unwrap_or_default()
            .to_string()
    }

    pub fn name(me: &Value) -> String {
        me.get("name").and_then(Value::as_str).unwrap_or_default().to_string()
    }

    pub fn email(me: &Value) -> String {
        me.get("email").and_then(Value::as_str).unwrap_or_default().to_string()
    }

    #[allow(dead_code)]
    pub fn describe(me: &Value) -> String {
        let u = user(me);
        if !u.is_empty() {
            return u;
        }
        let e = email(me);
        if !e.is_empty() {
            return e;
        }
        name(me)
    }

    #[cfg(test)]
    mod tests {
        use serde_json::json;

        #[test]
        fn reads_github_user_shape() {
            let me = json!({"login": "octocat", "name": "The Octocat", "email": "octocat@github.com"});
            assert_eq!(super::user(&me), "octocat");
            assert_eq!(super::name(&me), "The Octocat");
            assert_eq!(super::email(&me), "octocat@github.com");
            assert_eq!(super::describe(&me), "octocat");
        }
    }
}
