# github

GitHub CLI for managing issues, pull requests, teams, organizations, runners, and code search.

## Install

```bash
cargo install --path .
```

## Authentication

Token is resolved in order:

1. `GITHUB_TOKEN` environment variable
2. `~/.config/github-cli/config.json` (set via `github config --token <TOKEN>`)
3. `gh auth token` (from GitHub CLI if installed)

## Commands

### Issues

```bash
github issue list owner/repo                  # List issues
github issue list owner/repo -S "bug"         # Search issues
github issue list owner/repo -l 20            # Limit results
github issue view owner/repo 42               # View issue details
github issue comments owner/repo 42           # List issue comments
```

### Pull Requests

```bash
github pr list owner/repo                     # List open PRs
github pr list owner/repo -s closed           # List closed PRs
github pr view owner/repo 123                 # View PR details
github pr comment owner/repo 123 -m "LGTM"   # Comment on PR
github pr comments owner/repo 123             # List PR comments
github pr approve owner/repo 123              # Approve PR
github pr discussions owner/repo 123          # List review threads
github pr discussions owner/repo 123 --unresolved  # Unresolved only
github pr reply owner/repo 123 --comment ID -m "Fixed"  # Reply to review comment
github pr review owner/repo 123 -e APPROVE -b "Looks good"  # Submit review
github pr review owner/repo 123 -c "path:10:issue here"     # Review with inline comments
```

### Reactions

```bash
github react owner/repo 42                   # +1 (default)
github react owner/repo 42 rocket            # Specific reaction
```

Reactions: `+1`, `-1`, `laugh`, `confused`, `heart`, `hooray`, `rocket`, `eyes`

### Teams

```bash
github team list myorg                        # List teams
github team create myorg "team-name"          # Create team
github team view myorg team-slug              # View team details
github team members myorg team-slug           # List members
github team add-member myorg team-slug user   # Add member
github team add-member myorg team-slug user -r maintainer  # Add as maintainer
github team remove-member myorg team-slug user  # Remove member
github team add-repo myorg team-slug owner/repo  # Add repo to team
github team add-repo myorg team-slug owner/repo -p admin  # With permission
github team repos myorg team-slug             # List team repos
```

### Organizations

```bash
github org members myorg                      # List members
github org invite myorg user@example.com      # Invite by email
github org invite myorg user@example.com -t 123,456  # Invite to teams
github org invitations myorg                  # List pending invitations
```

### Repositories

```bash
github repo keys list owner/repo              # List deploy keys
github repo keys add owner/repo -t "CI" key.pub  # Add deploy key
github repo keys add owner/repo -t "CI" key.pub -w  # With write access
github repo keys remove owner/repo 12345      # Remove deploy key
github repo hooks list owner/repo             # List webhooks
```

### Runners

```bash
github runner list owner/repo                 # List repo runners
github runner list myorg --org                # List org runners
github runner view owner/repo 1234            # View runner details
github runner delete owner/repo 1234          # Delete runner
```

### Workflow Runs

```bash
github run list owner/repo                    # List recent workflow runs
github run view owner/repo 123456             # View run details and jobs
github run watch owner/repo 123456            # Poll until the run completes
github run watch owner/repo 123456 -i 10 -t 1800  # Poll every 10s, timeout after 30m
github run logs owner/repo 123456             # Show failed job logs
```

### Code Search

```bash
github code search "pattern"                  # Search all repos
github code search "pattern" -L rust          # Filter by language
github code search "pattern" -r owner/repo    # Filter by repo
github code search "pattern" -o myorg         # Filter by org
github code search "pattern" -p "src/"        # Filter by path
github code search "pattern" -f "config.toml" # Filter by filename
github code search "pattern" -l 50            # Limit results
```

### GitHub Apps

```bash
github app list myorg                         # List app installations
```

### Configuration

```bash
github config --token ghp_xxx                 # Save token
```
