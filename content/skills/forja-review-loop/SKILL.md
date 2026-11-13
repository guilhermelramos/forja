---
name: forja-review-loop
description: How to execute the review and feedback loop in Forja, including inspecting diffs, sending line-by-line notes to helpers, opening PRs, and resolving CI checks.
---

# Forja Review Loop: From Diff to Pull Request

In Forja, code review is an interactive dialogue between the developer and AI helpers. You review diffs in the workbench, leave targeted line-level notes, dispatch corrections back to the helper, and verify automated CI checks before publishing.

---

## 1. Inspecting Changes on a Bench

Before initiating review, verify git status and enumerate modified files:

```sh
# View git status for the current bench
forja git.status

# List files with uncommitted changes
forja diff.changes

# Inspect unified diff for a specific file
forja diff.file --path src/kernel/runtime.rs
```

In the graphical user interface, switch to the **Diferenças** (Diff) tab to view side-by-side or inline colorized diffs.

---

## 2. Leaving Review Notes and Dispatching Corrections

Rather than manually editing code or describing changes in a loose chat, you can attach structured review comments directly to specific lines:

```sh
forja diff.notes.send --notes '[
  {
    "file": "src/kernel/runtime.rs",
    "line": 142,
    "body": "Use match with explicit error types instead of unwrap() to prevent panics during network reconnects."
  },
  {
    "file": "crates/modules/benches/src/db.rs",
    "line": 89,
    "body": "Ensure the database transaction commits before returning the bench reference."
  }
]'
```

### What happens when notes are sent:
1. The active helper receives the file paths, line numbers, and comment text as structured context.
2. The helper reads the target code, performs the requested refactorings, and runs relevant unit tests.
3. Once the helper finishes, the diff updates automatically, and resolved notes are marked.

---

## 3. Running Pre-Review Verification Recipes

Validate that changes adhere to workspace standards using terminal recipes:

```sh
# Run formatting check
forja terminal.send --terminal_id 1 --text "cargo fmt --all --check"

# Run compiler and clippy linter
forja terminal.send --terminal_id 1 --text "cargo clippy --workspace --all-targets -- -D warnings"

# Run the test suite
forja terminal.send --terminal_id 1 --text "cargo test --workspace"
```

---

## 4. Stage Progression and PR Submission

### Update bench stage to Review
Update the bench stage to notify team members and the visual board that work is being evaluated:
```sh
forja bench.stage.set --path C:/work/forja-benches/feature-auth --stage revisao
```

### Publishing the branch and opening PR
Once the diff meets all requirements:
```sh
# Push commits to remote origin
forja git.push

# Publish branch and generate PR metadata
forja git.publish
```

---

## 5. Fixing CI Checks and Finalizing

If automated CI checks fail on the remote repository:
1. Inspect the CI logs from the Checks panel or terminal.
2. Prompt the helper to reproduce and correct the failing check.
3. Review the subsequent diff and push fixes (`forja git.push`).
4. Once all checks pass and PR is approved, transition the bench stage to `pronto` (ready):
   ```sh
   forja bench.stage.set --path C:/work/forja-benches/feature-auth --stage pronto
   ```
5. Archive the bench when no longer needed:
   ```sh
   forja bench.store --path C:/work/forja-benches/feature-auth --delete_branch false
   ```
