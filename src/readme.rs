//! The manual an agent reads before its first call. Markdown by default so it can be pasted
//! into a system prompt or a CLAUDE.md; `--json` gives the same rules as data.

use crate::{obj, output};

pub fn print() {
    if output::json() {
        output::write(&obj! {
            "tool" => "gh-issue-import",
            "version" => env!("CARGO_PKG_VERSION"),
            "rules" => RULES,
            "exitCodes" => obj! {
                "0" => "ok",
                "1" => "error - unclassified, report and stop",
                "2" => "network - retry once, then stop",
                "3" => "auth_required - stop, surface the remediation to a human",
                "4" => "not_found - do not retry",
                "5" => "rate_limited - back off before retrying",
                "6" => "invalid_input - fix the call",
                "7" => "no_account - run gh-issue-import accounts list",
            },
        });
        return;
    }
    println!("{README}");
}

const RULES: &[&str] = &[
    "Pass --account (-a) on commands that interact with the GitHub API. There is no default account.",
    "Run 'gh-issue-import accounts list' first if you do not know which accounts exist.",
    "On exit code 3 (auth_required), stop immediately and surface the remediation string.",
    "Use 'validate' on issue files before running 'import' to verify schemas and required fields.",
    "Imports are tracked in import-log.json for idempotency. Re-running will not duplicate imported issues.",
    "Use --dry-run with 'import' to preview issues that would be created without touching GitHub.",
    "Use --json when you are going to parse or process the CLI output.",
];

const README: &str = r#"# gh-issue-import - agent operating manual

A native Rust CLI for importing GitHub issues from YAML/JSON files and remote repositories
with full idempotency tracking, milestone mapping, labels, and assignees.

Results are YAML on stdout, errors are YAML on stderr, and `--json` switches both to JSON.
Prompts and warnings go to stderr, so stdout is always safe to parse.

## Always name the account

`--account` (short `-a`) is required on every command that touches the GitHub API. There is no
implicit default account - this prevents accidents when multiple organizations are configured.

    gh-issue-import accounts list
    gh-issue-import import issues.yaml --repo owner/repo -a work

### Managing accounts

    gh-issue-import login [<name>] [--token <token>]  # opens browser to create token
    gh-issue-import accounts add <name> --token <token> [--force]
    printf %s "$TOKEN" | gh-issue-import accounts add <name> --api-key-stdin
    gh-issue-import accounts list [--check]
    gh-issue-import accounts test <name>
    gh-issue-import accounts remove <name> --yes

Tokens are stored securely in the native OS keystore (macOS Keychain, Linux Secret Service,
Windows DPAPI).

## Validating issue files

Before importing, validate the YAML/JSON structure and required fields locally:

    gh-issue-import validate -f issues.yaml
    gh-issue-import validate issues.json --repo owner/repo -a work

## Importing issues

Import issues into a target GitHub repository:

    gh-issue-import import issues.yaml --repo owner/repo -a work
    gh-issue-import import issues.yaml --repo owner/repo -a work --dry-run
    gh-issue-import import issues.yaml --repo owner/repo -a work --skip-existing
    gh-issue-import import -c config.yaml -a work

### Idempotency tracking

Every created issue is recorded in `import-log.json` (or custom `--log <path>`). If an issue
has already been imported, subsequent runs skip it automatically.

### File format (YAML or JSON)

A list of issues or an object with an `issues` list:

```yaml
issues:
  - title: "Implement dark mode toggle"
    body: "Provide user-configurable dark theme toggle in navigation bar."
    labels: ["enhancement", "ui"]
    milestone: "v1.0"
    assignees: ["octocat"]
  - title: "Fix connection timeout in client"
    body: "Handle socket timeouts gracefully when offline."
    labels: ["bug"]
```

## Errors and exit codes

Failures output a structured envelope on stderr with a stable machine-readable `code`:

    0  ok
    1  error          unclassified - report it and stop
    2  network        retry once, then stop
    3  auth_required  stop; give the human the remediation string verbatim
    4  not_found      repository or resource does not exist; do not retry
    5  rate_limited   GitHub API rate limit exceeded; back off before retrying
    6  invalid_input  fix the arguments or file format
    7  no_account     run `gh-issue-import accounts list`
"#;
