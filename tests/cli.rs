//! Drives the built binary against an in-process mock of the GitHub REST API.
//! Every test gets its own config directory and the plaintext store, so nothing touches a real keystore or account.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct Recorded {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<Value>,
}

type Route = (&'static str, &'static str, u16, Value);

struct Mock {
    url: String,
    log: Arc<Mutex<Vec<Recorded>>>,
}

impl Mock {
    fn start(routes: Vec<Route>) -> Mock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let log = Arc::new(Mutex::new(Vec::new()));
        let log2 = log.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let routes = routes.clone();
                let log = log2.clone();
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    let mut parts = line.split_whitespace();
                    let method = parts.next().unwrap_or("").to_string();
                    let path = parts.next().unwrap_or("").trim_start_matches('/').to_string();
                    let mut headers = Vec::new();
                    let mut len = 0usize;
                    loop {
                        let mut h = String::new();
                        reader.read_line(&mut h).unwrap();
                        let h = h.trim_end();
                        if h.is_empty() {
                            break;
                        }
                        if let Some((k, v)) = h.split_once(':') {
                            let (k, v) = (k.trim().to_lowercase(), v.trim().to_string());
                            if k == "content-length" {
                                len = v.parse().unwrap_or(0);
                            }
                            headers.push((k, v));
                        }
                    }
                    let mut buf = vec![0; len];
                    reader.read_exact(&mut buf).unwrap();
                    let body = (len > 0).then(|| serde_json::from_slice(&buf).unwrap_or(Value::Null));
                    log.lock().unwrap().push(Recorded { method: method.clone(), path: path.clone(), headers, body });

                    let (status, resp) = routes
                        .iter()
                        .find(|(m, p, _, _)| *m == method && *p == path)
                        .map(|(_, _, s, b)| (*s, b.clone()))
                        .unwrap_or((
                            404,
                            json!({"message": "Not Found", "documentation_url": "https://docs.github.com"}),
                        ));
                    let text = if status == 204 { String::new() } else { resp.to_string() };
                    let _ = write!(
                        stream,
                        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                        text.len()
                    );
                });
            }
        });
        Mock { url, log }
    }

    fn requests(&self) -> Vec<Recorded> {
        self.log.lock().unwrap().clone()
    }

    fn last(&self, method: &str) -> Recorded {
        self.requests().into_iter().rev().find(|r| r.method == method).expect("no such request")
    }
}

struct Env {
    dir: PathBuf,
    api: String,
}

impl Env {
    fn new(mock: &Mock) -> Env {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "gh-issue-import-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Env { dir, api: mock.url.clone() }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_gh-issue-import"))
            .args(args)
            .env("GH_ISSUE_IMPORT_CONFIG_DIR", &self.dir)
            .env("GH_ISSUE_IMPORT_SECRET_STORE", "plaintext")
            .env("GH_ISSUE_IMPORT_API_URL", &self.api)
            .output()
            .unwrap()
    }

    fn json(&self, args: &[&str]) -> (i32, Value, Value) {
        let mut all = args.to_vec();
        all.push("--json");
        let out = self.run(&all);
        let parse = |b: &[u8]| {
            let s = String::from_utf8_lossy(b);
            let s = s.lines().filter(|l| !l.starts_with("warning:")).collect::<Vec<_>>().join("\n");
            serde_json::from_str(&s).unwrap_or(Value::Null)
        };
        (out.status.code().unwrap_or(-1), parse(&out.stdout), parse(&out.stderr))
    }

    fn with_account(self) -> Env {
        let (code, out, err) = self.json(&["accounts", "add", "work", "--token", "gh_test_token"]);
        assert_eq!(code, 0, "{err}");
        assert_eq!(out["status"], "added");
        self
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn user_route() -> Route {
    ("GET", "user", 200, json!({"login": "octocat", "name": "The Octocat", "email": "octocat@github.com"}))
}

#[test]
fn accounts_lifecycle() {
    let mock = Mock::start(vec![user_route()]);
    let env = Env::new(&mock).with_account();

    assert_eq!(mock.last("GET").headers.iter().find(|(k, _)| k == "authorization").unwrap().1, "Bearer gh_test_token");

    let (code, out, _) = env.json(&["accounts", "list"]);
    assert_eq!(code, 0);
    assert_eq!(out["count"], 1);
    assert_eq!(out["accounts"][0]["user"], "octocat");
    assert_eq!(out["accounts"][0]["displayName"], "The Octocat");
    assert_eq!(out["accounts"][0]["tokenStatus"], "stored");
    assert_eq!(out["secretStore"], "plaintext");

    let (_, out, _) = env.json(&["accounts", "list", "--check"]);
    assert_eq!(out["accounts"][0]["tokenStatus"], "valid");

    // Case-insensitive duplicate check without --force
    let (code, _, err) = env.json(&["accounts", "add", "WORK", "--token", "x"]);
    assert_eq!(code, 6);
    assert!(err["remediation"].as_str().unwrap().contains("--force"));

    let (code, out, _) = env.json(&["accounts", "test", "Work"]);
    assert_eq!(code, 0);
    assert_eq!(out["tokenStatus"], "valid");

    // Remove without --yes
    let (code, _, err) = env.json(&["accounts", "remove", "work"]);
    assert_eq!(code, 6);
    assert_eq!(err["remediation"], "gh-issue-import accounts remove work --yes");

    let (code, out, _) = env.json(&["accounts", "remove", "work", "--yes"]);
    assert_eq!(code, 0);
    assert_eq!(out["status"], "removed");

    let (_, out, _) = env.json(&["accounts", "list"]);
    assert_eq!(out["count"], 0);
}

#[test]
fn token_from_stdin() {
    let mock = Mock::start(vec![user_route()]);
    let env = Env::new(&mock);
    let mut child = Command::new(env!("CARGO_BIN_EXE_gh-issue-import"))
        .args(["accounts", "add", "piped", "--api-key-stdin", "--json"])
        .env("GH_ISSUE_IMPORT_CONFIG_DIR", &env.dir)
        .env("GH_ISSUE_IMPORT_SECRET_STORE", "plaintext")
        .env("GH_ISSUE_IMPORT_API_URL", &env.api)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"gh_piped_token\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let auth = mock.last("GET").headers.into_iter().find(|(k, _)| k == "authorization").unwrap().1;
    assert_eq!(auth, "Bearer gh_piped_token");
}

#[test]
fn config_is_readable_yaml_without_secrets() {
    let mock = Mock::start(vec![user_route()]);
    let env = Env::new(&mock).with_account();
    let yaml = std::fs::read_to_string(env.dir.join("config.yaml")).unwrap();
    assert!(yaml.contains("work:"), "{yaml}");
    assert!(yaml.contains("user: octocat"), "{yaml}");
    assert!(!yaml.contains("gh_test_token"), "token leaked into config.yaml");
}

#[test]
fn account_is_required() {
    let mock = Mock::start(vec![user_route()]);
    let env = Env::new(&mock).with_account();

    let (code, _, err) = env.json(&["me"]);
    assert_eq!(code, 7);
    assert_eq!(err["code"], "no_account");
    assert!(err["detail"].as_str().unwrap().contains("work (octocat)"));

    let (code, _, err) = env.json(&["me", "-a", "nope"]);
    assert_eq!(code, 7);
    assert_eq!(err["remediation"], "gh-issue-import accounts list");
}

#[test]
fn http_errors_map_to_exit_codes() {
    let mock = Mock::start(vec![
        user_route(),
        ("GET", "repos/octo/denied", 401, json!({"message": "Bad credentials"})),
        ("GET", "repos/octo/secret", 403, json!({"message": "API rate limit exceeded"})),
        ("GET", "repos/octo/missing", 404, json!({"message": "Not Found"})),
        ("POST", "repos/octo/invalid/issues", 422, json!({"message": "Validation Failed"})),
        ("GET", "repos/octo/broken", 500, json!({"message": "Server Error"})),
    ]);
    let env = Env::new(&mock).with_account();

    let file_path = env.dir.join("test.yaml");
    std::fs::write(&file_path, "issues:\n  - title: Bug\n").unwrap();

    // 401 AuthRequired
    let (code, _, err) = env.json(&["validate", "-f", file_path.to_str().unwrap(), "-r", "octo/denied", "-a", "work"]);
    assert_eq!(code, 3);
    assert_eq!(err["code"], "auth_required");

    // 403 RateLimited
    let (code, _, err) = env.json(&["validate", "-f", file_path.to_str().unwrap(), "-r", "octo/secret", "-a", "work"]);
    assert_eq!(code, 5);
    assert_eq!(err["code"], "rate_limited");

    // 404 NotFound
    let (code, _, err) = env.json(&["validate", "-f", file_path.to_str().unwrap(), "-r", "octo/missing", "-a", "work"]);
    assert_eq!(code, 4);
    assert_eq!(err["code"], "not_found");

    // 422 InvalidInput
    let (code, _, err) = env.json(&["import", file_path.to_str().unwrap(), "--repo", "octo/invalid", "-a", "work"]);
    assert_eq!(code, 6);
    assert_eq!(err["code"], "invalid_input");

    // 500 Network
    let (code, _, err) = env.json(&["validate", "-f", file_path.to_str().unwrap(), "-r", "octo/broken", "-a", "work"]);
    assert_eq!(code, 2);
    assert_eq!(err["code"], "network");
}

#[test]
fn parse_errors_are_envelopes() {
    let mock = Mock::start(vec![]);
    let env = Env::new(&mock);
    let (code, _, err) = env.json(&["import", "--unknown-flag"]);
    assert_eq!(code, 6);
    assert_eq!(err["code"], "invalid_input");

    let out = env.run(&["--help"]);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn yaml_is_the_default() {
    let mock = Mock::start(vec![user_route()]);
    let env = Env::new(&mock).with_account();
    let out = env.run(&["accounts", "list"]);
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("count: 1"), "{stdout}");
    assert!(stdout.contains("user: octocat"), "{stdout}");
}

#[test]
fn agent_readme_as_data() {
    let mock = Mock::start(vec![]);
    let env = Env::new(&mock);
    let (code, out, _) = env.json(&["agent-readme"]);
    assert_eq!(code, 0);
    assert_eq!(out["exitCodes"]["7"], "no_account - run gh-issue-import accounts list");
    assert_eq!(out["tool"], "gh-issue-import");

    let md = String::from_utf8(env.run(&["agent-readme"]).stdout).unwrap();
    assert!(md.starts_with("# gh-issue-import - agent operating manual"));
}

#[test]
fn login_command_lifecycle() {
    let mock = Mock::start(vec![user_route(), user_route(), user_route()]);
    let env = Env::new(&mock);

    let (code, out, err) = env.json(&["login", "--token", "gh_login_token"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(out["status"], "logged_in");
    assert_eq!(out["name"], "default");
    assert_eq!(out["user"], "octocat");

    let (code, _, err) = env.json(&["login", "--token", "gh_new_token"]);
    assert_eq!(code, 6);
    assert!(err["remediation"].as_str().unwrap().contains("--force"));

    let (code, out, _) = env.json(&["login", "--token", "gh_new_token", "--force"]);
    assert_eq!(code, 0);
    assert_eq!(out["status"], "logged_in");

    let (code, out, _) = env.json(&["login", "staging", "--token", "gh_staging"]);
    assert_eq!(code, 0);
    assert_eq!(out["name"], "staging");
}

#[test]
fn validate_command() {
    let mock = Mock::start(vec![]);
    let env = Env::new(&mock);

    let valid_yaml = r#"
issues:
  - title: "Bug: login button broken"
    body: "Button does not respond to clicks"
    labels: ["bug", "ui"]
    milestone: 1
    assignees: ["octocat"]
  - title: "Feature: export to csv"
    labels: ["enhancement"]
"#;
    let file = env.dir.join("valid.yaml");
    std::fs::write(&file, valid_yaml).unwrap();

    let (code, out, _) = env.json(&["validate", "-f", file.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert_eq!(out["status"], "valid");
    assert_eq!(out["issuesCount"], 2);

    let invalid_yaml = r#"
issues:
  - body: "No title provided"
"#;
    let inv_file = env.dir.join("invalid.yaml");
    std::fs::write(&inv_file, invalid_yaml).unwrap();

    let (code, _, err) = env.json(&["validate", "-f", inv_file.to_str().unwrap()]);
    assert_eq!(code, 6);
    assert!(err["error"].as_str().unwrap().contains("missing a required 'title'"));
}

#[test]
fn import_issues_idempotency() {
    let mock = Mock::start(vec![
        user_route(),
        (
            "POST",
            "repos/octo/test-repo/issues",
            201,
            json!({
                "number": 101,
                "title": "Bug: login button broken",
                "html_url": "https://github.com/octo/test-repo/issues/101"
            }),
        ),
    ]);
    let env = Env::new(&mock).with_account();

    let issues_yaml = r#"
- title: "Bug: login button broken"
  body: "Details here"
  labels: ["bug"]
"#;
    let file = env.dir.join("issues.yaml");
    std::fs::write(&file, issues_yaml).unwrap();
    let log_file = env.dir.join("custom-log.json");

    // Dry run
    let (code, out, _) = env.json(&[
        "import",
        "-f",
        file.to_str().unwrap(),
        "--repo",
        "octo/test-repo",
        "-a",
        "work",
        "--dry-run",
        "--log",
        log_file.to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert_eq!(out["planned"], 1);
    assert_eq!(out["imported"], 0);
    assert_eq!(mock.requests().iter().filter(|r| r.method == "POST").count(), 0);

    // Real import
    let (code, out, err) = env.json(&[
        "import",
        "-f",
        file.to_str().unwrap(),
        "--repo",
        "octo/test-repo",
        "-a",
        "work",
        "--log",
        log_file.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(out["imported"], 1);
    assert_eq!(out["skipped"], 0);
    assert_eq!(out["items"][0]["number"], 101);

    // Idempotent re-run skips
    let (code, out, _) = env.json(&[
        "import",
        "-f",
        file.to_str().unwrap(),
        "--repo",
        "octo/test-repo",
        "-a",
        "work",
        "--log",
        log_file.to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert_eq!(out["imported"], 0);
    assert_eq!(out["skipped"], 1);
    assert_eq!(out["items"][0]["status"], "already_imported");
}
