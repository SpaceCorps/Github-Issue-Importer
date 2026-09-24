//! Core issue importing logic, file parsing, and idempotency tracking.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::client::Client;
use crate::config;
use crate::error::{Error, Result};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum MilestoneRef {
    Number(u64),
    Title(String),
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct IssueItem {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub labels: Option<Vec<String>>,
    #[serde(default)]
    pub milestone: Option<MilestoneRef>,
    #[serde(default)]
    pub assignees: Option<Vec<String>>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SourceConfig {
    pub repo: String,
    #[serde(default)]
    pub filter: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BatchConfig {
    #[serde(default)]
    pub inbox: Option<String>,
    #[serde(default, rename = "target_repo", alias = "targetRepo", alias = "repo")]
    pub target_repo: Option<String>,
    #[serde(default)]
    pub sources: Vec<SourceConfig>,
    #[serde(default)]
    pub issues: Vec<IssueItem>,
}

#[derive(Clone, Debug)]
pub enum ParsedInput {
    Issues(Vec<IssueItem>),
    Batch(BatchConfig),
}

/// Parses issues from a YAML or JSON file. Supports a direct list of issues,
/// an `{ issues: [...] }` object, or a batch config `{ inbox: ..., sources: [...] }`.
pub fn load_issues_file(path: &Path) -> Result<ParsedInput> {
    let content = fs::read_to_string(path)
        .map_err(|e| Error::invalid(format!("Could not read file '{}': {e}", path.display())))?;

    if content.trim().is_empty() {
        return Ok(ParsedInput::Issues(Vec::new()));
    }

    // Try direct array of issues first
    if let Ok(issues) = serde_norway::from_str::<Vec<IssueItem>>(&content) {
        return Ok(ParsedInput::Issues(issues));
    }

    // Try batch config / object wrapper
    if let Ok(batch) = serde_norway::from_str::<BatchConfig>(&content) {
        if !batch.sources.is_empty() || batch.inbox.is_some() {
            return Ok(ParsedInput::Batch(batch));
        }
        if !batch.issues.is_empty() {
            return Ok(ParsedInput::Issues(batch.issues));
        }
    }

    // Try json parsing as fallback
    if let Ok(issues) = serde_json::from_str::<Vec<IssueItem>>(&content) {
        return Ok(ParsedInput::Issues(issues));
    }
    if let Ok(batch) = serde_json::from_str::<BatchConfig>(&content) {
        if !batch.sources.is_empty() || batch.inbox.is_some() {
            return Ok(ParsedInput::Batch(batch));
        }
        if !batch.issues.is_empty() {
            return Ok(ParsedInput::Issues(batch.issues));
        }
    }

    Err(Error::invalid(format!(
        "File '{}' is not valid YAML or JSON issues file. Expected an array of issues or an object with 'issues' or 'sources'.",
        path.display()
    )))
}

/// Parses "owner/repo" or "https://github.com/owner/repo" into ("owner", "repo").
pub fn parse_repo(input: &str) -> Result<(String, String)> {
    let s = input.trim().trim_end_matches(".git");
    let path = if let Some(stripped) = s.strip_prefix("https://github.com/") {
        stripped
    } else if let Some(stripped) = s.strip_prefix("http://github.com/") {
        stripped
    } else if let Some(stripped) = s.strip_prefix("git@github.com:") {
        stripped
    } else {
        s
    };

    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Ok((parts[0].to_string(), parts[1].to_string()))
    } else {
        Err(Error::invalid(format!(
            "Invalid repository format '{input}'. Expected 'owner/repo' or 'https://github.com/owner/repo'."
        )))
    }
}

/// Idempotency log for imported issues.
pub struct ImportLog {
    path: PathBuf,
    entries: BTreeMap<String, Value>,
    keys: HashSet<String>,
}

impl ImportLog {
    pub fn open(path: PathBuf) -> Self {
        let (entries, keys) = Self::load(&path);
        ImportLog { path, entries, keys }
    }

    pub fn is_imported(&self, key: &str) -> bool {
        self.keys.contains(key)
    }

    pub fn mark_imported(&mut self, key: &str, number: Option<u64>, url: Option<&str>) -> Result<()> {
        let now = config::now_utc();
        let mut entry = serde_json::Map::new();
        entry.insert("key".into(), json!(key));
        entry.insert("importedAt".into(), json!(now));
        if let Some(n) = number {
            entry.insert("number".into(), json!(n));
        }
        if let Some(u) = url {
            entry.insert("url".into(), json!(u));
        }

        self.keys.insert(key.to_string());
        self.entries.insert(key.to_string(), Value::Object(entry));
        self.save()
    }

    #[allow(dead_code)]
    pub fn count(&self) -> usize {
        self.keys.len()
    }

    fn load(path: &Path) -> (BTreeMap<String, Value>, HashSet<String>) {
        if !path.exists() {
            return (BTreeMap::new(), HashSet::new());
        }
        let Ok(text) = fs::read_to_string(path) else {
            return (BTreeMap::new(), HashSet::new());
        };

        // Support NielsBosma string array format: ["repo#1", "repo#2"]
        if let Ok(list) = serde_json::from_str::<Vec<String>>(&text) {
            let mut entries = BTreeMap::new();
            let mut keys = HashSet::new();
            for item in list {
                keys.insert(item.clone());
                entries.insert(item.clone(), json!({ "key": item }));
            }
            return (entries, keys);
        }

        // Support object format: { "key1": { ... }, "key2": { ... } }
        if let Ok(map) = serde_json::from_str::<BTreeMap<String, Value>>(&text) {
            let keys: HashSet<String> = map.keys().cloned().collect();
            return (map, keys);
        }

        (BTreeMap::new(), HashSet::new())
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(&self.entries)
            .map_err(|e| Error::other(format!("Could not serialize import log: {e}")))?;
        fs::write(&self.path, json)
            .map_err(|e| Error::other(format!("Could not save import log to '{}': {e}", self.path.display())))?;
        Ok(())
    }
}

/// Resolves milestone reference (int or string) to milestone number.
pub fn resolve_milestone(client: &Client, owner: &str, repo: &str, m_ref: &MilestoneRef) -> Result<Option<u64>> {
    match m_ref {
        MilestoneRef::Number(n) => Ok(Some(*n)),
        MilestoneRef::Title(title) => {
            let milestones_val = client.list_milestones(owner, repo)?;
            if let Some(arr) = milestones_val.as_array() {
                for m in arr {
                    if let Some(t) = m.get("title").and_then(Value::as_str)
                        && t.eq_ignore_ascii_case(title)
                        && let Some(num) = m.get("number").and_then(Value::as_u64)
                    {
                        return Ok(Some(num));
                    }
                }
            }
            // If milestone title not found, return an error or None
            Err(Error::invalid(format!("Milestone '{title}' was not found in repository '{owner}/{repo}'.")))
        }
    }
}
