# GitHub CLI

Rust CLI tool wrapping the GitHub REST API.

## Structure

Single-file project: `src/main.rs` (~1450 lines).

Sections in order:
1. **CLI definitions** - Clap derive structs and enums for all commands
2. **Config** - Token storage in `~/.config/github-cli/config.json`
3. **Client** - HTTP client with methods for each API endpoint
4. **Print helpers** - Output formatting functions (`print_issues`, `print_prs`, etc.)
5. **Main** - Command dispatch

## Adding a New Command

1. Add variant to `Commands` enum (or a sub-enum like `IssueCommands`)
2. Add API method to `Client` impl
3. Add `print_*` helper for output formatting
4. Add match arm in `main()`

## Conventions

- All API calls go through `Client` methods that return `serde_json::Value`
- Token resolution: `GITHUB_TOKEN` env > config file > `gh auth token`
- API version header: `2022-11-28`
- Output uses formatted text, not JSON (human-readable)
- Repos are specified as `owner/repo` strings, split with `split_once('/')`

## Build

```bash
cargo build --release
```

## Test

No tests yet.
