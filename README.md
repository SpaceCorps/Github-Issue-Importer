# GitHub Issue Importer

[![Release](https://img.shields.io/github/v/release/SpaceCorps/Github-Issue-Importer?color=blue&label=version)](https://github.com/SpaceCorps/Github-Issue-Importer/releases/latest)
[![CI](https://github.com/SpaceCorps/Github-Issue-Importer/actions/workflows/ci.yml/badge.svg)](https://github.com/SpaceCorps/Github-Issue-Importer/actions/workflows/ci.yml)
[![Docs](https://img.shields.io/badge/docs-online-success)](https://spacecorps.github.io/Github-Issue-Importer/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A blazing fast, native command-line tool and agent interface for importing and synchronizing issues from YAML/JSON files and remote sources into GitHub repositories with full idempotency tracking.

Ported to native Rust Edition 2024 under [SpaceCorps](https://github.com/SpaceCorps) from the original prototype by Niels Bosma (`nielsbosma/GithubIssueImporter`).

---

## Highlights

- ⚡ **Sub-3ms Startup**: Compiled as a native static binary with zero runtime dependencies. Executes in ~1–3 ms.
- 🛡️ **Idempotent Imports**: State tracking via `import-log.json` prevents duplicate issue creation even if imports are interrupted or re-run.
- 🏷️ **Comprehensive Issue Support**: Full support for titles, markdown bodies, labels, milestone resolution (by name or number), and assignees.
- 🔐 **OS Keystore Integration**: `gh-issue-import login` prompts for your token securely and saves it to native OS vaults (macOS Keychain, Linux Secret Service, Windows DPAPI).
- 🤖 **AI Agent Native**: Machine-readable `--json` output, standardized error envelopes with stable exit codes, and built-in `agent-readme` manual.
- 🏢 **Multi-Account Scoping**: Strict `--account <name>` requirement on mutating calls prevents accidental issue creation in unintended organizations.

---

## Installation

### Using Cargo

```bash
cargo install --git https://github.com/SpaceCorps/Github-Issue-Importer --locked
```

### Pre-built Standalone Binaries

Download standalone binary archives directly from the [GitHub Releases](https://github.com/SpaceCorps/Github-Issue-Importer/releases/latest) page:

| Platform | Architecture | Binary Package |
|:---|:---|:---|
| **macOS** | Apple Silicon (`aarch64`) | [`gh-issue-import-v1.0.0-aarch64-apple-darwin.tar.gz`](https://github.com/SpaceCorps/Github-Issue-Importer/releases/download/v1.0.0/gh-issue-import-v1.0.0-aarch64-apple-darwin.tar.gz) |
| **macOS** | Intel (`x86_64`) | [`gh-issue-import-v1.0.0-x86_64-apple-darwin.tar.gz`](https://github.com/SpaceCorps/Github-Issue-Importer/releases/download/v1.0.0/gh-issue-import-v1.0.0-x86_64-apple-darwin.tar.gz) |
| **Linux** | x86_64 (musl static) | [`gh-issue-import-v1.0.0-x86_64-unknown-linux-musl.tar.gz`](https://github.com/SpaceCorps/Github-Issue-Importer/releases/download/v1.0.0/gh-issue-import-v1.0.0-x86_64-unknown-linux-musl.tar.gz) |
| **Linux** | aarch64 (musl static) | [`gh-issue-import-v1.0.0-aarch64-unknown-linux-musl.tar.gz`](https://github.com/SpaceCorps/Github-Issue-Importer/releases/download/v1.0.0/gh-issue-import-v1.0.0-aarch64-unknown-linux-musl.tar.gz) |
| **Windows**| x64 (MSVC) | [`gh-issue-import-v1.0.0-x86_64-pc-windows-msvc.zip`](https://github.com/SpaceCorps/Github-Issue-Importer/releases/download/v1.0.0/gh-issue-import-v1.0.0-x86_64-pc-windows-msvc.zip) |

---

## Quickstart

### 1. Authenticate

Run `gh-issue-import login` to open your browser to GitHub's token creation page with the `repo` scope pre-selected:

```bash
# Interactive login (stores token in macOS Keychain / Secret Service / DPAPI under 'default')
gh-issue-import login

# Log in with a named account
gh-issue-import login work

# Headless / CI pipeline login (reads token from stdin with no shell history trace)
echo "$GITHUB_TOKEN" | gh-issue-import login ci --api-key-stdin
```

### 2. Validate Your Issues File

Verify issue syntax, required fields, and milestone existence without creating any issues:

```bash
gh-issue-import validate -f issues.yaml
gh-issue-import validate -f issues.yaml --repo owner/repo -a work
```

### 3. Import Issues Idempotently

```bash
# Preview what would be imported
gh-issue-import import issues.yaml --repo owner/repo -a work --dry-run

# Import issues into the repository
gh-issue-import import issues.yaml --repo owner/repo -a work

# Re-running skips already imported issues automatically
gh-issue-import import issues.yaml --repo owner/repo -a work
```

---

## Issue File Formats

`gh-issue-import` accepts YAML or JSON. You can format the file as a list of issues or an object with an `issues` list:

```yaml
issues:
  - title: "Support dark mode in web UI"
    body: "Provide user-configurable dark theme toggle in navigation bar."
    labels:
      - "enhancement"
      - "ui"
    milestone: "v1.0"
    assignees:
      - "octocat"
  - title: "Fix API rate limit backoff jitter"
    body: "Implement exponential backoff with full jitter on HTTP 429."
    labels:
      - "bug"
```

Or top-level list:

```yaml
- title: "Implement OAuth2 login flow"
  body: "Support GitHub and Google OAuth2 providers."
  labels: ["feature", "auth"]
- title: "Optimize database query latency"
  labels: ["performance"]
```

---

## Command Reference

| Command | Description | Example |
|:---|:---|:---|
| `import` | Import issues from YAML/JSON file or config into GitHub repo or inbox | `gh-issue-import import issues.yaml --repo owner/repo -a work` |
| `validate` | Validate issue file syntax, schema, and milestone existence | `gh-issue-import validate -f issues.yaml -r owner/repo -a work` |
| `login` | Authenticate with GitHub PAT and save to native OS keystore | `gh-issue-import login work` |
| `me` | Query authenticated user identity and token context | `gh-issue-import me -a work` |
| `accounts list` | List configured accounts and verify token validity with GitHub | `gh-issue-import accounts list --check` |
| `accounts test` | Test account connectivity and check for identity drift | `gh-issue-import accounts test work` |
| `accounts remove` | Remove account from config and clear token from OS keystore | `gh-issue-import accounts remove work --yes` |
| `agent-readme` | Print operating instructions and rules for AI agents | `gh-issue-import agent-readme --json` |

---

## Autonomous Agent Integration

When driving `gh-issue-import` programmatically from an LLM agent, MCP server, or pipeline:
- Always pass `--json` for structured output.
- Start by inspecting the built-in manual: `gh-issue-import agent-readme --json`.
- Failures emit standard machine-readable JSON envelopes on `stderr`:

```json
{
  "error": "The GitHub token was rejected.",
  "code": "auth_required",
  "detail": "HTTP 401: Bad credentials",
  "remediation": "Replace it: gh-issue-import accounts add <name> --token <token> --force"
}
```

Exit code numbers match the error contract:
- `0`: Success (`ok`)
- `1`: `error` - Unclassified failure
- `2`: `network` - Transport failure or 5xx server error
- `3`: `auth_required` - Token missing, expired, or rejected
- `4`: `not_found` - Repository or resource does not exist
- `5`: `rate_limited` - GitHub API rate limit reached
- `6`: `invalid_input` - Bad command flags, missing arguments, or invalid file schema
- `7`: `no_account` - `--account <name>` was not specified

---

## Documentation & Trust Links

- **Documentation Website**: [https://spacecorps.github.io/Github-Issue-Importer/](https://spacecorps.github.io/Github-Issue-Importer/)
- **Agent Specification (`llms.txt`)**: [https://spacecorps.github.io/Github-Issue-Importer/llms.txt](https://spacecorps.github.io/Github-Issue-Importer/llms.txt)
- **Authentication Guide**: [docs/auth.md](https://spacecorps.github.io/Github-Issue-Importer/auth.md)
- **Pricing & Licensing**: [docs/pricing.md](https://spacecorps.github.io/Github-Issue-Importer/pricing.md)
- **Privacy Policy**: [https://spacecorps.github.io/Github-Issue-Importer/privacy.html](https://spacecorps.github.io/Github-Issue-Importer/privacy.html)
- **Architecture Guide for Agents**: [AGENTS.md](AGENTS.md)

---

## License

Released under the [MIT License](LICENSE). Copyright &copy; 2026 SpaceCorps.
