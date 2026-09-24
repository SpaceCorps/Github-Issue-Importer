//! HTTP client to the GitHub REST API, and translation from HTTP status to [`ErrorCode`].

use std::time::Duration;

use serde_json::Value;
use ureq::Agent;
use ureq::http::Response;

use crate::error::{Error, ErrorCode, Result};

const DEFAULT_BASE: &str = "https://api.github.com/";
const MAX_BODY: u64 = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct Client {
    agent: Agent,
    base: String,
    auth: String,
}

#[allow(dead_code)]
enum Method {
    Get,
    Post,
    Patch,
    Delete,
}

impl Client {
    pub fn new(token: &str) -> Client {
        let agent: Agent = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(60)))
            .timeout_connect(Some(Duration::from_secs(15)))
            .http_status_as_error(false)
            .user_agent(concat!("gh-issue-import/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();

        let mut base = std::env::var("GH_ISSUE_IMPORT_API_URL")
            .or_else(|_| std::env::var("GITHUB_API_URL"))
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BASE.to_string());
        if !base.ends_with('/') {
            base.push('/');
        }

        Client { agent, base, auth: format!("Bearer {token}") }
    }

    pub fn get(&self, path: &str) -> Result<Value> {
        self.send(Method::Get, path, None)
    }

    pub fn post(&self, path: &str, body: &Value) -> Result<Value> {
        self.send(Method::Post, path, Some(body))
    }

    #[allow(dead_code)]
    pub fn patch(&self, path: &str, body: &Value) -> Result<Value> {
        self.send(Method::Patch, path, Some(body))
    }

    #[allow(dead_code)]
    pub fn delete(&self, path: &str) -> Result<Value> {
        self.send(Method::Delete, path, None)
    }

    pub fn user(&self) -> Result<Value> {
        self.get("user")
    }

    pub fn repo(&self, owner: &str, repo: &str) -> Result<Value> {
        self.get(&format!("repos/{owner}/{repo}"))
    }

    pub fn create_issue(&self, owner: &str, repo: &str, issue: &Value) -> Result<Value> {
        self.post(&format!("repos/{owner}/{repo}/issues"), issue)
    }

    pub fn list_issues(&self, owner: &str, repo: &str, query_params: &[(&str, Option<String>)]) -> Result<Value> {
        self.get(&format!("repos/{owner}/{repo}/issues{}", query(query_params)))
    }

    pub fn list_milestones(&self, owner: &str, repo: &str) -> Result<Value> {
        self.get(&format!("repos/{owner}/{repo}/milestones?state=all&per_page=100"))
    }

    #[allow(dead_code)]
    pub fn list_labels(&self, owner: &str, repo: &str) -> Result<Value> {
        self.get(&format!("repos/{owner}/{repo}/labels?per_page=100"))
    }

    pub fn search_issues(&self, q: &str) -> Result<Value> {
        self.get(&format!("search/issues?q={}", seg(q)))
    }

    fn send(&self, method: Method, path: &str, body: Option<&Value>) -> Result<Value> {
        let clean_path = path.trim_start_matches('/');
        let url = format!("{}{}", self.base, clean_path);

        macro_rules! headers {
            ($req:expr) => {{
                $req.header("Authorization", &self.auth)
                    .header("Accept", "application/vnd.github+json")
                    .header("X-GitHub-Api-Version", "2022-11-28")
            }};
        }

        let result = match (method, body) {
            (Method::Get, _) => headers!(self.agent.get(&url)).call(),
            (Method::Delete, _) => headers!(self.agent.delete(&url)).call(),
            (Method::Post, Some(b)) => {
                let json = serde_json::to_vec(b).expect("a Value always serializes");
                headers!(self.agent.post(&url)).header("Content-Type", "application/json").send(&json[..])
            }
            (Method::Patch, Some(b)) => {
                let json = serde_json::to_vec(b).expect("a Value always serializes");
                headers!(self.agent.patch(&url)).header("Content-Type", "application/json").send(&json[..])
            }
            (Method::Post, None) => headers!(self.agent.post(&url)).send_empty(),
            (Method::Patch, None) => headers!(self.agent.patch(&url)).send_empty(),
        };

        let response = result.map_err(transport_error)?;
        read(response)
    }
}

fn read(mut response: Response<ureq::Body>) -> Result<Value> {
    let status = response.status().as_u16();
    let bytes = response.body_mut().with_config().limit(MAX_BODY).read_to_vec().map_err(transport_error)?;

    if !(200..300).contains(&status) {
        let body = String::from_utf8_lossy(&bytes).trim().to_string();
        return Err(status_error(status, &body));
    }

    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(Value::Object(Default::default()));
    }

    serde_json::from_slice(&bytes).or_else(|_| Ok(Value::String(String::from_utf8_lossy(&bytes).into_owned())))
}

fn transport_error(e: ureq::Error) -> Error {
    match e {
        ureq::Error::Timeout(_) => {
            Error::new(ErrorCode::Network, "The request timed out.").fix("Retry once, then stop.")
        }
        other => Error::new(ErrorCode::Network, "Could not reach the GitHub API.")
            .detail(other.to_string())
            .fix("Retry once, then stop."),
    }
}

pub fn status_error(status: u16, body: &str) -> Error {
    let mut detail = format!("HTTP {status}");
    if !body.is_empty() {
        detail.push_str(": ");
        detail.push_str(body);
    }

    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let message = parsed.as_ref().and_then(|v| v.get("message").and_then(Value::as_str)).unwrap_or("");

    let lower = body.to_ascii_lowercase();
    let is_rate_limit = status == 429 || (status == 403 && lower.contains("rate limit"));

    let e = match status {
        401 => Error::new(ErrorCode::AuthRequired, "The GitHub token was rejected.")
            .fix("Replace it: gh-issue-import accounts add <name> --token <token> --force"),
        403 if is_rate_limit => Error::new(ErrorCode::RateLimited, "GitHub API rate limit exceeded.")
            .fix("Wait for the rate limit window to reset before retrying."),
        403 => Error::new(ErrorCode::AuthRequired, "The GitHub token is not permitted to perform this action.")
            .fix("Ensure your token has 'repo' scope permissions."),
        404 => Error::new(ErrorCode::NotFound, "The GitHub repository or resource was not found."),
        429 => Error::new(ErrorCode::RateLimited, "GitHub API rate limit exceeded.")
            .fix("Wait for the rate limit window to reset before retrying."),
        422 => {
            let msg = if !message.is_empty() {
                format!("GitHub rejected the issue request: {message}")
            } else {
                "The GitHub API refused the request payload.".to_string()
            };
            Error::new(ErrorCode::InvalidInput, msg)
        }
        s if s >= 500 => Error::new(ErrorCode::Network, "GitHub returned a server error.")
            .fix("Retry; if it persists check https://www.githubstatus.com"),
        _ => Error::new(ErrorCode::Error, "The GitHub API request failed."),
    };
    e.detail(detail)
}

/// Percent-encodes one path segment or query value.
pub fn seg(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Builds `?a=1&b=2` from the pairs that have a value.
pub fn query(pairs: &[(&str, Option<String>)]) -> String {
    let parts: Vec<String> = pairs.iter().filter_map(|(k, v)| v.as_ref().map(|v| format!("{k}={}", seg(v)))).collect();
    if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seg_escapes_special() {
        assert_eq!(seg("repo:octo/cat is:open"), "repo%3Aocto%2Fcat%20is%3Aopen");
    }

    #[test]
    fn status_maps_properly() {
        assert_eq!(status_error(401, "").code, ErrorCode::AuthRequired);
        assert_eq!(status_error(404, "").code, ErrorCode::NotFound);
        assert_eq!(status_error(422, "").code, ErrorCode::InvalidInput);
        assert_eq!(status_error(429, "").code, ErrorCode::RateLimited);
        assert_eq!(status_error(403, r#"{"message":"API rate limit exceeded"}"#).code, ErrorCode::RateLimited);
        assert_eq!(status_error(500, "").code, ErrorCode::Network);
    }
}
