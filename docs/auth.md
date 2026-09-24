---
title: "Authentication Guide"
description: "Authentication methods, token storage, and keystore integration for developers and AI agents using GitHub Issue Importer."
author: "SpaceCorps"
date: "2026-09-24"
---

# Authentication Guide for GitHub Issue Importer

This document outlines authentication methods, credential storage, and error remediation for developers and AI agents using `gh-issue-import`.

## Overview
GitHub Issue Importer interfaces directly with the GitHub REST API (v3). Authentication utilizes GitHub Personal Access Tokens (classic tokens with `repo` scope or fine-grained repository tokens). Tokens are stored securely in the host operating system's native keystore (macOS Keychain, Linux Secret Service, Windows DPAPI).

## Prerequisites
- A GitHub account ([github.com](https://github.com))
- A GitHub Personal Access Token with repository write permissions (`repo` scope)
- GitHub Issue Importer installed (`cargo install --git https://github.com/SpaceCorps/Github-Issue-Importer --locked`)

## Authentication Methods

### Interactive Browser Login (`gh-issue-import login`)
For developer workstations with a desktop browser:
```bash
gh-issue-import login [account_name]
```
1. The CLI launches your system browser to GitHub's token creation page with the `repo` scope pre-selected.
2. Generate and copy your token.
3. Paste the token into the CLI prompt (input characters are masked).
4. The CLI validates the token by requesting `GET /user`.
5. Upon confirmation, the token is stored in the OS keystore under the specified account name (defaults to `default`).

### Non-Interactive / Headless Login
For continuous integration runners, Docker containers, or autonomous agent tool loops:
```bash
echo "$GITHUB_TOKEN" | gh-issue-import login [account_name] --api-key-stdin
```
Or pass the token directly as a flag:
```bash
gh-issue-import login [account_name] --token "$GITHUB_TOKEN"
```

## Multi-Account Management
GitHub Issue Importer enforces explicit account scoping to prevent accidental mutations across disparate organizations:
```bash
# Add a named account
gh-issue-import accounts add work --token "$WORK_TOKEN"

# List configured accounts with live validation
gh-issue-import accounts list --check

# Test connectivity and inspect token identity
gh-issue-import accounts test work

# Remove an account
gh-issue-import accounts remove work --yes
```

## Environment Variables
- `GH_ISSUE_IMPORT_CONFIG_DIR`: Override directory for configuration and metadata.
- `GH_ISSUE_IMPORT_SECRET_STORE`: Force a specific keystore backend (`keychain`, `libsecret`, `dpapi`, `plaintext`).
- `GH_ISSUE_IMPORT_ALLOW_PLAINTEXT_STORE`: Set to `1` in headless environments lacking a desktop keyring daemon.
- `GH_ISSUE_IMPORT_API_URL`: Custom GitHub API URL (for GitHub Enterprise Server).

## Error Handling
When authentication fails, commands exit with non-zero status codes and emit structured error envelopes on stderr:
- `auth_required` (Exit 3): Token missing, expired, or invalid scope.
- `no_account` (Exit 7): Mandatory `--account` flag omitted or unknown account name.
- `rate_limited` (Exit 5): GitHub API rate limits reached.
- `invalid_input` (Exit 6): Input argument or schema error.
