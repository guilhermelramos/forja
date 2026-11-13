---
name: forja-cli
description: How a helper or agent operates Forja through the forja CLI to manage benches, terminals, requests, routines, and git state.
---

# Operating Forja via the CLI

As an AI agent or automated worker running inside or alongside Forja, you can control the environment and query workbench state using the `forja` command-line interface.

## 1. Invocation Syntax

The CLI accepts two equivalent naming styles for subcommands:
```sh
# Dot syntax
forja <group>.<action> [flags]

# Space syntax
forja <group> <action> [flags]
```

Use `--json` on any command to receive machine-readable JSON output:
```sh
forja --json bench.list
forja --json git.status
```

To view all supported RPC methods served by the running application:
```sh
forja metodos
```

---

## 2. Managing Benches (`bench.*`)

A **bench** (bancada) is an isolated workspace backed by a Git worktree, complete with its own terminals, active helper sessions, and stage tracking.

### Inspect current and listed benches
```sh
# Show active bench
forja bench.current

# List all open benches
forja bench.list

# List benches with card details (stage, branch, helper count)
forja bench.list_cards
```

### Create and switch benches
```sh
# Create a new bench from a branch or request name
forja bench.create --name feature-auth --project_id proj_backend

# Switch active focus to another bench
forja bench.activate --path C:/work/forja-benches/feature-auth
```

### Update stages
Benches transition through stages: `rascunho` (draft), `forjando` (in progress), `revisao` (review), and `pronto` (ready).
```sh
forja bench.stage.set --path C:/work/forja-benches/feature-auth --stage revisao
```

### Archive and teardown
```sh
# Archive a completed bench (optionally remove the git worktree branch)
forja bench.store --path C:/work/forja-benches/feature-auth --delete_branch false
```

---

## 3. Controlling Terminals (`terminal.*`)

You can launch commands, monitor output, and send keystrokes into ConPTY terminals owned by Forja.

### Launch a terminal
```sh
# Open a shell in the bench directory
forja terminal.open --cwd C:/work/forja-benches/feature-auth

# Open with a specific shell (pwsh, cmd, git-bash, wsl)
forja terminal.open --cwd C:/work/forja-benches/feature-auth --shell pwsh --title "Build"

# Open running a specific process
forja terminal.open --cwd C:/work/forja-benches/feature-auth --command "cargo test"
```

### List terminals
```sh
forja terminal.list
forja terminal.list --bench_path C:/work/forja-benches/feature-auth
```

### Read terminal output
```sh
# Read the last 100 lines of output from a terminal
forja terminal.read --terminal_id 1 --lines 100
```

### Send input or commands
```sh
# Send command followed by Enter
forja terminal.send --terminal_id 1 --text "cargo check" --enter true
```

---

## 4. Helper Management (`helper.*`)

Inspect available agent profiles and dispatch new helpers.

```sh
# List configured helper profiles
forja helper.list

# Detect installed AI CLIs on the host system
forja helper.detect

# Launch a helper session in a bench
forja helper.launch --profile_id claude-code --bench_path C:/work/forja-benches/feature-auth --prompt "Fix lint issues" --permission ask
```

---

## 5. Working with Requests (`request.*`)

Query incoming work items (issues, pull requests, tasks) synced from connected counters (GitHub, GitLab, Jira, Linear, or local):
```sh
# List requests for the bench or project
forja request.list
```

---

## 6. Running Routines (`routine.*`)

Trigger background scheduled tasks and inspect their execution logs:
```sh
# List registered routines
forja routine.list

# Trigger a routine by id
forja routine.run --id nightly-security-audit

# Inspect execution history
forja routine.runs
```

---

## 7. Inspecting Diff and Changes (`diff.*`, `git.*`)

Check what has changed within the bench:
```sh
# Check git status
forja git.status

# Inspect changed files
forja diff.changes

# Inspect detailed diff for a single file
forja diff.file --path src/main.rs

# Send inline review notes to an active helper session
forja diff.notes.send --notes '[{"file":"src/main.rs","line":42,"body":"Avoid unwrap here"}]'
```

---

## 8. Querying Archive and Skills (`archive.*`, `skill.*`)

Search through past agent sessions in the Acervo and check available skills:
```sh
# Search session logs
forja archive.search --query "database migration"

# List installed skills
forja skill.list
```
