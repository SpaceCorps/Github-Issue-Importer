//! `gh-issue-import import`. Imports issues from YAML/JSON files or remote repos with idempotency.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::account;
use crate::cli::ImportArgs;
use crate::client::Client;
use crate::commands::print;
use crate::config;
use crate::error::{Error, Result};
use crate::importer::{self, ImportLog, IssueItem, ParsedInput};
use crate::obj;

pub fn run(args: ImportArgs) -> Result<()> {
    let input_file = resolve_input_path(&args)?;
    let parsed = importer::load_issues_file(&input_file)?;

    let log_path = args
        .log
        .map(PathBuf::from)
        .unwrap_or_else(|| input_file.parent().unwrap_or_else(|| Path::new(".")).join("import-log.json"));

    let mut import_log = ImportLog::open(log_path.clone());

    match parsed {
        ParsedInput::Issues(issues) => {
            let target_repo = args
                .repo
                .ok_or_else(|| Error::invalid("A target GitHub repository is required. Pass --repo <owner/repo>."))?;
            import_issues_to_repo(
                issues,
                &target_repo,
                args.account.as_deref(),
                args.dry_run,
                args.skip_existing,
                &mut import_log,
                &log_path,
            )
        }
        ParsedInput::Batch(batch) => {
            let target_repo = args.repo.or(batch.target_repo.clone());
            if let Some(target) = target_repo {
                let mut all_issues = batch.issues;
                // If there are sources and account, fetch remote issues
                if !batch.sources.is_empty() {
                    let resolved = account::resolve(args.account.as_deref())?;
                    let client = resolved.client();
                    let fetched = fetch_source_issues(&client, &batch.sources)?;
                    all_issues.extend(fetched);
                }
                import_issues_to_repo(
                    all_issues,
                    &target,
                    args.account.as_deref(),
                    args.dry_run,
                    args.skip_existing,
                    &mut import_log,
                    &log_path,
                )
            } else if let Some(inbox) = batch.inbox {
                // Local inbox mode (preserving original NielsBosma functionality)
                import_sources_to_inbox(
                    &batch.sources,
                    &inbox,
                    args.account.as_deref(),
                    args.dry_run,
                    &mut import_log,
                    &log_path,
                )
            } else {
                Err(Error::invalid("Either --repo <owner/repo> or an 'inbox' directory in config must be specified."))
            }
        }
    }
}

fn resolve_input_path(args: &ImportArgs) -> Result<PathBuf> {
    if let Some(p) = args.file.as_deref().or(args.path.as_deref()).or(args.config.as_deref()) {
        let path = PathBuf::from(p);
        if path.exists() {
            return Ok(path);
        }
        return Err(Error::invalid(format!("File '{}' not found.", path.display())));
    }

    // Fall back to default config locations
    let default_config = config::config_path();
    if default_config.exists() {
        return Ok(default_config);
    }
    let local_config = PathBuf::from("config.yaml");
    if local_config.exists() {
        return Ok(local_config);
    }

    Err(Error::invalid("No issues file or config found. Specify a path, --file <PATH>, or --config <PATH>."))
}

fn import_issues_to_repo(
    issues: Vec<IssueItem>,
    target_repo: &str,
    account_name: Option<&str>,
    dry_run: bool,
    skip_existing: bool,
    import_log: &mut ImportLog,
    log_path: &Path,
) -> Result<()> {
    let (owner, repo) = importer::parse_repo(target_repo)?;
    let resolved = account::resolve(account_name)?;
    let client = resolved.client();

    let existing_titles: HashSet<String> =
        if skip_existing { load_existing_titles(&client, &owner, &repo)? } else { HashSet::new() };

    let mut total = 0;
    let mut imported = 0;
    let mut skipped = 0;
    let mut planned = 0;
    let mut items = Vec::new();

    for (idx, issue) in issues.into_iter().enumerate() {
        total += 1;
        let title = issue.title.trim().to_string();
        if title.is_empty() {
            continue;
        }

        let key = issue
            .id
            .as_deref()
            .map(|id| format!("{owner}/{repo}#{id}"))
            .unwrap_or_else(|| format!("{owner}/{repo}#{title}"));

        if import_log.is_imported(&key) {
            skipped += 1;
            items.push(obj! {
                "index" => idx + 1,
                "title" => title,
                "status" => "already_imported",
                "key" => key,
            });
            continue;
        }

        if skip_existing && existing_titles.contains(&title.to_lowercase()) {
            skipped += 1;
            items.push(obj! {
                "index" => idx + 1,
                "title" => title,
                "status" => "already_exists",
                "key" => key,
            });
            continue;
        }

        let milestone_num = if let Some(m_ref) = &issue.milestone {
            importer::resolve_milestone(&client, &owner, &repo, m_ref)?
        } else {
            None
        };

        if dry_run {
            planned += 1;
            items.push(obj! {
                "index" => idx + 1,
                "title" => title,
                "status" => "planned",
                "milestone" => milestone_num,
                "labels" => issue.labels.clone().unwrap_or_default(),
                "assignees" => issue.assignees.clone().unwrap_or_default(),
            });
            continue;
        }

        let mut payload = serde_json::Map::new();
        payload.insert("title".into(), json!(title));
        if let Some(body) = issue.body {
            payload.insert("body".into(), json!(body));
        }
        if let Some(labels) = issue.labels {
            payload.insert("labels".into(), json!(labels));
        }
        if let Some(assignees) = issue.assignees {
            payload.insert("assignees".into(), json!(assignees));
        }
        if let Some(m) = milestone_num {
            payload.insert("milestone".into(), json!(m));
        }

        let res = client.create_issue(&owner, &repo, &Value::Object(payload))?;
        let number = res.get("number").and_then(Value::as_u64);
        let html_url = res.get("html_url").and_then(Value::as_str).unwrap_or("");

        import_log.mark_imported(&key, number, Some(html_url))?;
        imported += 1;

        items.push(obj! {
            "index" => idx + 1,
            "title" => title,
            "status" => "created",
            "number" => number,
            "url" => html_url,
        });
    }

    print(obj! {
        "status" => "ok",
        "targetRepo" => format!("{owner}/{repo}"),
        "total" => total,
        "imported" => imported,
        "planned" => planned,
        "skipped" => skipped,
        "dryRun" => dry_run,
        "logPath" => log_path.display().to_string(),
        "items" => items,
    })
}

fn load_existing_titles(client: &Client, owner: &str, repo: &str) -> Result<HashSet<String>> {
    let mut titles = HashSet::new();
    let issues_val =
        client.list_issues(owner, repo, &[("state", Some("all".into())), ("per_page", Some("100".into()))])?;

    if let Some(arr) = issues_val.as_array() {
        for item in arr {
            if let Some(t) = item.get("title").and_then(Value::as_str) {
                titles.insert(t.trim().to_lowercase());
            }
        }
    }
    Ok(titles)
}

fn fetch_source_issues(client: &Client, sources: &[importer::SourceConfig]) -> Result<Vec<IssueItem>> {
    let mut results = Vec::new();

    for s in sources {
        let (src_owner, src_repo) = importer::parse_repo(&s.repo)?;
        let query_str = if s.filter.trim().is_empty() {
            format!("repo:{src_owner}/{src_repo} is:issue state:open")
        } else {
            format!("repo:{src_owner}/{src_repo} is:issue {}", s.filter)
        };

        let search_res = client.search_issues(&query_str)?;
        if let Some(items) = search_res.get("items").and_then(Value::as_array) {
            for item in items {
                let title = item.get("title").and_then(Value::as_str).unwrap_or("").to_string();
                let body = item.get("body").and_then(Value::as_str).map(str::to_string);
                let number = item.get("number").and_then(Value::as_u64);
                let id = number.map(|n| format!("{src_owner}/{src_repo}#{n}"));

                let labels: Vec<String> = item
                    .get("labels")
                    .and_then(Value::as_array)
                    .map(|larr| {
                        larr.iter().filter_map(|l| l.get("name").and_then(Value::as_str).map(str::to_string)).collect()
                    })
                    .unwrap_or_default();

                results.push(IssueItem {
                    title,
                    body,
                    labels: if labels.is_empty() { None } else { Some(labels) },
                    milestone: None,
                    assignees: None,
                    id,
                    state: None,
                });
            }
        }
    }
    Ok(results)
}

fn import_sources_to_inbox(
    sources: &[importer::SourceConfig],
    inbox: &str,
    account_name: Option<&str>,
    dry_run: bool,
    import_log: &mut ImportLog,
    log_path: &Path,
) -> Result<()> {
    let resolved = account::resolve(account_name)?;
    let client = resolved.client();
    let inbox_path = PathBuf::from(inbox);

    if !dry_run {
        fs::create_dir_all(&inbox_path)?;
    }

    let mut total = 0;
    let mut imported = 0;
    let mut skipped = 0;
    let mut items = Vec::new();

    for s in sources {
        let (src_owner, src_repo) = importer::parse_repo(&s.repo)?;
        let query_str = if s.filter.trim().is_empty() {
            format!("repo:{src_owner}/{src_repo} is:issue state:open")
        } else {
            format!("repo:{src_owner}/{src_repo} is:issue {}", s.filter)
        };

        let search_res = client.search_issues(&query_str)?;
        if let Some(issue_items) = search_res.get("items").and_then(Value::as_array) {
            for item in issue_items {
                total += 1;
                let number = item.get("number").and_then(Value::as_u64).unwrap_or(0);
                let title = item.get("title").and_then(Value::as_str).unwrap_or("");
                let url = item.get("html_url").and_then(Value::as_str).unwrap_or("");
                let key = format!("{}#{}", s.repo, number);

                if import_log.is_imported(&key) {
                    skipped += 1;
                    items.push(obj! {
                        "number" => number,
                        "title" => title,
                        "status" => "already_imported",
                    });
                    continue;
                }

                if !dry_run {
                    let file_name = format!("issue-{number}.md");
                    let file_path = inbox_path.join(file_name);
                    fs::write(&file_path, format!("{url}\n"))?;
                    import_log.mark_imported(&key, Some(number), Some(url))?;
                }

                imported += 1;
                items.push(obj! {
                    "number" => number,
                    "title" => title,
                    "url" => url,
                    "status" => if dry_run { "planned" } else { "imported" },
                });
            }
        }
    }

    print(obj! {
        "status" => "ok",
        "inbox" => inbox,
        "total" => total,
        "imported" => imported,
        "skipped" => skipped,
        "dryRun" => dry_run,
        "logPath" => log_path.display().to_string(),
        "items" => items,
    })
}
