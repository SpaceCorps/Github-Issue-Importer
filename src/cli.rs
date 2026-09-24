//! The command tree for gh-issue-import.

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "gh-issue-import",
    version,
    about = "Fast, idempotent GitHub issue importer from YAML/JSON files and remote repos",
    after_help = "An LLM agent should start with: gh-issue-import agent-readme",
    propagate_version = true,
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Print raw JSON instead of YAML, for scripting
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Clone)]
pub struct AccountOpt {
    /// Account to run against (see 'gh-issue-import accounts list')
    #[arg(short = 'a', long, value_name = "ACCOUNT")]
    pub account: Option<String>,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Command {
    /// Import issues from YAML/JSON file or remote sources into a GitHub repo or inbox
    Import(ImportArgs),
    /// Validate issues YAML/JSON syntax, schema, and references without modifying anything
    Validate(ValidateArgs),
    /// Get current identity and GitHub token context
    Me(AccountOpt),
    /// Log in with a GitHub Personal Access Token (opens browser to create token)
    Login(LoginArgs),
    /// Manage accounts and their GitHub Personal Access Tokens
    #[command(subcommand)]
    Accounts(AccountsCommand),
    /// Print the operating manual for an LLM agent driving this CLI
    AgentReadme,
}

// ---------------------------------------------------------------------------------------------
// import

#[derive(Args, Clone)]
pub struct ImportArgs {
    /// Positional path to YAML/JSON issues file or config
    #[arg(value_name = "PATH")]
    pub path: Option<String>,

    /// Path to YAML or JSON file containing issues
    #[arg(short = 'f', long, value_name = "FILE")]
    pub file: Option<String>,

    /// Target GitHub repository (e.g. 'owner/repo' or 'https://github.com/owner/repo')
    #[arg(short = 'r', long, value_name = "REPO")]
    pub repo: Option<String>,

    /// Path to batch/sources config.yaml (default: ~/.config/github-issue-importer/config.yaml if no file)
    #[arg(short = 'c', long, value_name = "CONFIG")]
    pub config: Option<String>,

    /// Account to run against (see 'gh-issue-import accounts list')
    #[arg(short = 'a', long, value_name = "ACCOUNT")]
    pub account: Option<String>,

    /// Dry run: preview issues that would be created without making modifications
    #[arg(long)]
    pub dry_run: bool,

    /// Path to import log for idempotency tracking (default: import-log.json)
    #[arg(long, value_name = "PATH")]
    pub log: Option<String>,

    /// Check target repo for existing issues with the same title and skip if found
    #[arg(long)]
    pub skip_existing: bool,
}

// ---------------------------------------------------------------------------------------------
// validate

#[derive(Args, Clone)]
pub struct ValidateArgs {
    /// Positional path to YAML/JSON issues file or config to validate
    #[arg(value_name = "PATH")]
    pub path: Option<String>,

    /// Path to YAML or JSON file containing issues
    #[arg(short = 'f', long, value_name = "FILE")]
    pub file: Option<String>,

    /// Target GitHub repository to check milestone/label existence
    #[arg(short = 'r', long, value_name = "REPO")]
    pub repo: Option<String>,

    /// Account to run against for repository checks
    #[arg(short = 'a', long, value_name = "ACCOUNT")]
    pub account: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// login

#[derive(Args, Clone)]
pub struct LoginArgs {
    /// Account name to store (default: "default")
    #[arg(value_name = "NAME", default_value = "default")]
    pub name: String,

    /// GitHub Personal Access Token (prompted for securely if omitted)
    #[arg(long, value_name = "TOKEN", alias = "api-key", conflicts_with = "api_key_stdin")]
    pub token: Option<String>,

    /// Read the token from stdin, e.g. `pbpaste | gh-issue-import login --api-key-stdin`
    #[arg(long, alias = "token-stdin")]
    pub api_key_stdin: bool,

    /// Do not open the browser to the GitHub tokens page automatically
    #[arg(long)]
    pub no_browser: bool,

    /// Replace the token on an account that already exists
    #[arg(long)]
    pub force: bool,

    /// Store the token without calling the GitHub API to check it first
    #[arg(long)]
    pub no_verify: bool,
}

// ---------------------------------------------------------------------------------------------
// accounts

#[derive(Subcommand, Clone)]
pub enum AccountsCommand {
    /// Add a GitHub account and store its Personal Access Token securely
    Add {
        /// Name for this account (e.g. 'work', 'personal', 'org')
        #[arg(value_name = "NAME")]
        name: String,

        /// GitHub Personal Access Token (prompted for securely if omitted)
        #[arg(long, value_name = "TOKEN", alias = "api-key", conflicts_with = "api_key_stdin")]
        token: Option<String>,

        /// Read token from stdin, e.g. `printf %s "$TOKEN" | gh-issue-import accounts add work --api-key-stdin`
        #[arg(long, alias = "token-stdin")]
        api_key_stdin: bool,

        /// Replace the token if this account already exists
        #[arg(long)]
        force: bool,

        /// Store the token without verifying it against GitHub first
        #[arg(long)]
        no_verify: bool,
    },
    /// List configured accounts and report keystore status
    List {
        /// Call the GitHub API to verify each stored token
        #[arg(long)]
        check: bool,
    },
    /// Verify an account's token against GitHub and report current user details
    Test {
        /// Account to test
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// Delete an account and remove its token from the keystore
    Remove {
        /// Account to remove
        #[arg(value_name = "NAME")]
        name: String,

        /// Do not prompt for confirmation
        #[arg(long)]
        yes: bool,
    },
}
