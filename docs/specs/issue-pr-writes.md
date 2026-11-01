# Issue and PR writes

The CLI supports bounded issue and pull request write actions. The command implementation is in `src/commands/issue.rs` and `src/commands/pr.rs`.

## What it must do

### Issues

- [ ] `issue create <repo>` requires `--title`/`-t` and accepts optional `--body`/`-b`, `--labels`, and `--assignees`.
- [ ] `issue edit <repo> <number>` accepts optional `--title`, `--body`, `--labels`, and `--assignees`.
- [ ] `issue close` and `issue reopen` accept a repo and issue number.
- [ ] `issue comment <repo> <number>` requires `--message`/`-m`.

### Pull requests

- [ ] `pr create <repo>` requires `--head`, `--base`, and `--title`; it accepts optional `--body`, `--draft`, `--labels`, and `--assignees`.
- [ ] `pr edit <repo> <number>` accepts optional `--title`, `--body`, `--base`, `--labels`, and `--assignees`.
- [ ] `pr close` and `pr reopen` accept a repo and PR number.
- [ ] PR metadata is written separately through the issue endpoint. If that write fails after PR creation or update, the error identifies the already-created or updated PR so creation is not retried blindly.
- [ ] Existing PR comment, approve, reply, review, and merge actions remain available.

### Fields

- [ ] On create and edit, `--labels` and `--assignees` accept comma-separated values and replace the full respective sets; an empty string clears the set.
- [ ] Omitted edit fields remain unchanged, and an empty body edit is accepted.

## How it works

- [Issue and PR writes architecture](../wiki/systems/issue-pr-writes.md) (not yet written).

## Implementation inventory

- `src/commands/issue.rs` — issue commands.
- `src/commands/pr.rs` — PR commands.
- `src/commands/issue_fields.rs` — issue metadata fields.
- `src/client.rs` — API client.

## Tests asserting this spec

- Not yet verified; the integration owner will identify passing tests.

## Known gaps (current cycle)

- [ ] Verify the contracts above against the integrated implementation and tests.

## Out of scope

- Additional issue or PR actions: this contract covers the requested bounded write actions and preserves the listed existing PR actions.
