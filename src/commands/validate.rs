//! `gh-issue-import validate`. Validates issue syntax, schema, and references.

use std::path::Path;

use serde_json::{Value, json};

use crate::account;
use crate::cli::ValidateArgs;
use crate::commands::print;
use crate::error::{Error, Result};
use crate::importer::{self, IssueItem, MilestoneRef, ParsedInput};
use crate::obj;

pub fn run(args: ValidateArgs) -> Result<()> {
    let input_path = args
        .path
        .or(args.file)
        .ok_or_else(|| Error::invalid("An issues file or config path is required. Pass <PATH> or --file <PATH>."))?;

    let path = Path::new(&input_path);
    let parsed = importer::load_issues_file(path)?;

    match parsed {
        ParsedInput::Issues(issues) => validate_issues(&input_path, issues, args.repo, args.account),
        ParsedInput::Batch(batch) => validate_batch(&input_path, batch, args.repo, args.account),
    }
}

fn validate_issues(
    file_path: &str,
    issues: Vec<IssueItem>,
    repo_arg: Option<String>,
    account_arg: Option<String>,
) -> Result<()> {
    if issues.is_empty() {
        return Err(Error::invalid(format!("No issues found in '{file_path}'.")));
    }

    let mut validated_items = Vec::new();

    for (idx, issue) in issues.iter().enumerate() {
        if issue.title.trim().is_empty() {
            return Err(Error::invalid(format!("Issue #{} is missing a required 'title'.", idx + 1)));
        }

        let milestone_val = match &issue.milestone {
            Some(MilestoneRef::Number(n)) => json!(n),
            Some(MilestoneRef::Title(t)) => json!(t),
            None => Value::Null,
        };

        validated_items.push(obj! {
            "index" => idx + 1,
            "title" => issue.title,
            "hasBody" => issue.body.is_some(),
            "labelsCount" => issue.labels.as_ref().map(|l| l.len()).unwrap_or(0),
            "assigneesCount" => issue.assignees.as_ref().map(|a| a.len()).unwrap_or(0),
            "milestone" => milestone_val,
        });
    }

    let mut output = obj! {
        "status" => "valid",
        "file" => file_path,
        "issuesCount" => issues.len(),
        "issues" => validated_items,
    };

    if let Some(target_repo) = repo_arg {
        let (owner, repo) = importer::parse_repo(&target_repo)?;
        output["targetRepo"] = json!(format!("{owner}/{repo}"));

        if let Some(acct) = account_arg {
            let resolved = account::resolve(Some(&acct))?;
            let client = resolved.client();
            // Verify repo accessibility
            client.repo(&owner, &repo)?;

            // Verify milestones if any are title-based
            for issue in &issues {
                if let Some(m) = &issue.milestone {
                    importer::resolve_milestone(&client, &owner, &repo, m)?;
                }
            }
            output["repoChecked"] = json!(true);
        }
    }

    print(output)
}

fn validate_batch(
    file_path: &str,
    batch: importer::BatchConfig,
    repo_arg: Option<String>,
    account_arg: Option<String>,
) -> Result<()> {
    let mut sources_val = Vec::new();
    for (idx, s) in batch.sources.iter().enumerate() {
        if s.repo.trim().is_empty() {
            return Err(Error::invalid(format!("Source #{} is missing a required 'repo'.", idx + 1)));
        }
        let (owner, repo) = importer::parse_repo(&s.repo)?;
        sources_val.push(obj! {
            "source" => format!("{owner}/{repo}"),
            "filter" => s.filter,
        });
    }

    let target = repo_arg.or(batch.target_repo);

    let output = obj! {
        "status" => "valid",
        "file" => file_path,
        "sourcesCount" => batch.sources.len(),
        "issuesCount" => batch.issues.len(),
        "inbox" => batch.inbox.unwrap_or_default(),
        "targetRepo" => target.as_deref().unwrap_or_default(),
        "sources" => sources_val,
    };

    if !batch.issues.is_empty() {
        validate_issues(file_path, batch.issues, target, account_arg)?;
        return Ok(());
    }

    print(output)
}
