# AGENTS.md

Notes for whoever extends or maintains this codebase next.

`gh-issue-import` is a native Rust CLI for importing and synchronizing issues between structured
YAML/JSON files, remote issue sources, and GitHub repositories. It replaces the .NET prototype
`GithubIssueImporter` by Niels Bosma with a single standalone static binary (1–3 ms cold start)
running on Rust Edition 2024 with zero runtime dependencies.

For the manual the *agent* reads at runtime, run `gh-issue-import agent-readme` — that text lives
in `src/readme.rs` and provides the tool's machine interface. This file is for developers or agents
modifying the source code.

## Development & Test Commands

```bash
cargo build --release              # target/release/gh-issue-import
cargo test                         # unit tests + tests/cli.rs against an offline mock API
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --check
cargo install --path . --locked    # put gh-issue-import on PATH
```

Use a throwaway config directory when running tests manually so you never touch real credentials:

```bash
export GH_ISSUE_IMPORT_CONFIG_DIR=$(mktemp -d) GH_ISSUE_IMPORT_SECRET_STORE=plaintext
```

| Variable | Effect |
| --- | --- |
| `GH_ISSUE_IMPORT_CONFIG_DIR` | Overrides the config/secrets location |
| `GH_ISSUE_IMPORT_SECRET_STORE` | Forces a backend: `dpapi`, `keychain`, `libsecret`, `plaintext` |
| `GH_ISSUE_IMPORT_ALLOW_PLAINTEXT_STORE=1` | Permits the plaintext fallback where no keystore exists |
| `GH_ISSUE_IMPORT_API_URL` | Overrides GitHub API base URL — how `tests/cli.rs` points at its mock |

## Project Layout

```
src/
  main.rs          arg parsing, --json pre-scan, clap error formatting into error envelopes
  cli.rs           clap derive command definitions & help strings (import, validate, login, etc.)
  commands/
    mod.rs         command dispatcher and output printing
    accounts.rs    accounts add|list|test|remove with keystore integration
    import.rs      import command with idempotency checking and GitHub issue creation
    login.rs       interactive browser login and token verification
    validate.rs    schema and milestone validation for issue files
  client.rs        blocking HTTP (ureq + rustls), status -> ErrorCode mapping
  error.rs         ErrorCode enum, Error struct { code, message, detail, remediation }
  importer.rs      issue parsing, repo resolution, ImportLog idempotency state, milestone resolution
  output.rs        YAML default (serde_norway), JSON (--json), write_error envelope, obj! macro
  account.rs       multi-account resolution, identity prober
  config.rs        config.yaml metadata, atomic writes, cross-process lock
  secrets.rs       macOS Keychain, Linux secret-tool, Windows DPAPI, plaintext fallback
  readme.rs        agent-readme command handler and embedded agent rules
tests/
  cli.rs           in-process TCP mock HTTP server test suite (zero network activity)
```

## Architectural Tenets & Invariants

1. **Blocking HTTP over Tokio (Zero Async Runtime Overhead):**
   A CLI invocation makes 1 to a few requests. Tokio would cost more in startup than it saves.
   Where parallel requests are needed (`accounts list --check`), we use `std::thread::scope`.

2. **Mandatory Account Scoping:**
   Mutating API commands strictly require `--account <name>` (short `-a <name>`).
   There is no implicit global default account or automatic environment fallback for API calls,
   preventing accidental mutations across personal, organization, or client repositories.

3. **Cryptographic Keystore Isolation:**
   Tokens never touch `config.yaml` in plaintext. Tokens are stored in macOS Keychain (`/usr/bin/security`),
   Linux Secret Service (`secret-tool`), or Windows DPAPI. `GH_ISSUE_IMPORT_ALLOW_PLAINTEXT_STORE=1`
   is an explicit opt-out for headless runners.

4. **Idempotency by Default:**
   Every imported issue is recorded in `import-log.json`. Re-running the import command skips already
   imported issues. Use `--dry-run` to preview planned actions before mutating GitHub.

5. **Agentic Output Protocol:**
   Default output format is YAML (`serde_norway`). With `--json`, it renders pretty-printed JSON.
   Errors are emitted to `stderr` using the standard machine-readable JSON error envelope.
