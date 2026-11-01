# Issue and PR writes

The CLI creates, edits, closes and reopens issues and pull requests, and posts issue comments. Commands live in `src/commands/issue.rs` and `src/commands/pr.rs`.

## What it must do

### Issues

- [x] `issue create <repo>` requires `--title`/`-t` and accepts optional `--body`/`-b`, `--labels`, and `--assignees`.
- [x] `issue edit <repo> <number>` updates only supplied title, body, labels or assignees.
- [x] `issue close` and `issue reopen` update the requested issue state.
- [x] `issue comment <repo> <number>` requires `--message`/`-m` and posts its body.
- [x] Failed writes return GitHub's error without retrying the mutation.

### Pull requests

- [x] `pr create <repo>` requires `--head`, `--base`, and `--title`; it accepts optional `--body`, `--draft`, `--labels`, and `--assignees`.
- [x] `pr edit <repo> <number>` updates only supplied title, body, base, labels or assignees.
- [x] `pr close` and `pr reopen` update the requested PR state.
- [x] PR metadata uses a separate issue-endpoint write. A failure identifies an already-created or updated PR without repeating creation.
- [x] A failed initial PR write does not attempt a metadata update.

### Fields

- [x] `--labels` and `--assignees` accept comma-separated values and replace full sets; an empty string clears the set.
- [x] Omitted fields remain unchanged; an empty body edit clears the body.
- [x] An edit without fields fails before network IO. A metadata-only PR edit does not update pull fields.

## How it works

- [README command examples](../../README.md#issues).

## Implementation inventory

- `src/commands/issue.rs` — issue commands and writes.
- `src/commands/pr.rs` — PR commands and writes.
- `src/commands/issue_fields.rs` — shared label and assignee options.
- `src/client.rs` — authenticated JSON PATCH, POST and error propagation.

## Tests asserting this spec

- `src/commands/issue_write_tests.rs` — seven local HTTP-fixture and CLI tests.
- `src/commands/pr_write_tests.rs` — seven local HTTP-fixture and CLI tests.
- `src/commands/write_test_fixture.rs` — bounded local HTTP fixture; no live mutations.

## Known gaps (current cycle)

None in the bounded write contract. Remote authorization and repository policy remain GitHub's responsibility.

## Out of scope

Existing PR comment, approve, reply, review and merge behavior is unchanged. Destructive delete/lock actions and real GitHub mutations solely for testing are excluded.
