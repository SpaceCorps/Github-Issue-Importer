---
title: "GitHub Issue Importer"
description: "A fast native Rust CLI and agent interface for importing issues from YAML/JSON files and remote repos into GitHub repositories."
author: "SpaceCorps"
date: "2026-09-24"
canonical: "https://spacecorps.github.io/Github-Issue-Importer/index.md"
---

# GitHub Issue Importer

Fast, idempotent GitHub issue importer from YAML/JSON files and remote repositories into GitHub repos. Built in native Rust Edition 2024 for developers and autonomous AI agents.

## Quickstart

```bash
# Authenticate interactively via browser token flow
gh-issue-import login

# Validate an issues file locally
gh-issue-import validate -f issues.yaml

# Import issues into your repository (with dry-run preview)
gh-issue-import import issues.yaml --repo owner/repo -a default --dry-run
gh-issue-import import issues.yaml --repo owner/repo -a default
```

## Features

- **Blazing Fast Native Rust**: Sub-millisecond startup times with zero runtime dependencies.
- **Idempotent Imports**: State tracking via `import-log.json` prevents duplicate issue creation.
- **Rich Issue Support**: Titles, markdown bodies, labels, milestones (by name or number), and assignees.
- **AI Agent Native**: Structured JSON output (`--json`) and standardized error envelopes.
- **Secure Keystore Integration**: Token storage in native macOS Keychain, Linux Secret Service, and Windows DPAPI.
- **Multi-Account Workspaces**: Isolate personal, client, and organizational accounts safely.

## When to Use This CLI

Use the `gh-issue-import` CLI whenever you need to:
- Bulk import issues from YAML or JSON files into a GitHub repository.
- Synchronize issues from remote source repositories into a target repo or local markdown inbox.
- Validate issue file schema and references prior to import.
- Automate issue creation in CI/CD pipelines without risk of duplication.

## Documentation Links

- [llms.txt](https://spacecorps.github.io/Github-Issue-Importer/llms.txt)
- [Full Agent Manual](https://spacecorps.github.io/Github-Issue-Importer/llms-full.txt)
- [Pricing & Licensing](https://spacecorps.github.io/Github-Issue-Importer/pricing.md)
- [Authentication Guide](https://spacecorps.github.io/Github-Issue-Importer/auth.md)
- [About the Project](https://spacecorps.github.io/Github-Issue-Importer/about.html)
- [Contact & Support](https://spacecorps.github.io/Github-Issue-Importer/contact.html)
- [Privacy Policy](https://spacecorps.github.io/Github-Issue-Importer/privacy.html)
- [GitHub Repository](https://github.com/SpaceCorps/Github-Issue-Importer)
