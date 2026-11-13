---
name: forja-muster-coordinator
description: How to orchestrate a multi-agent Mutirão in Forja, including goal decomposition, task dependency management, dispatching, and inbox resolution.
---

# Forja Muster: Coordinator Protocol

A **Mutirão** (Muster) is Forja's coordination engine for multi-agent workflows. As a coordinator, you decompose a high-level project goal into a dependency graph of actionable tasks, dispatch them to helpers across isolated benches, and supervise their progress.

---

## 1. Creating a Muster Run (`muster.run`)

Initialize the coordination session with a clear title and strategic objective:
```sh
forja muster run \
  --title "Migrate persistence layer to SQLx" \
  --objective "Replace legacy diesel ORM queries with sqlx async queries, update test suite, and ensure clean migration scripts."
```

This returns a JSON object containing the assigned `run.id` (e.g. `run-1`).

---

## 2. Defining Tasks and Dependencies (`muster.task`)

Break down the objective into small, well-bounded tasks. Use `--deps` to define prerequisite tasks that must be delivered before a task can start.

```sh
# Task 1: Foundation (no dependencies)
forja muster task \
  --run run-1 \
  --title "Define SQLx connection pool and schema migrations" \
  --spec "Add sqlx crate dependencies, configure SqlitePool in kernel, and write migration scripts for benches and settings tables."

# Task 2: Refactor queries (depends on Task 1)
forja muster task \
  --run run-1 \
  --title "Rewrite bench repository queries using SQLx" \
  --spec "Update crates/modules/benches/src/db.rs to use sqlx query_as!. Ensure all repository tests pass." \
  --deps "task-1"

# Task 3: Test suite update (depends on Task 2)
forja muster task \
  --run run-1 \
  --title "Update integration test harness" \
  --spec "Verify all workspace tests pass with new sqlite pool. Ensure fixture isolation works." \
  --deps "task-2"
```

---

## 3. Dispatching Tasks to Helpers (`muster.start`)

Start helpers on unblocked tasks. Forja automatically sets up preambles and launches agent CLIs.

```sh
# Start task on a dedicated new git worktree bench
forja muster start \
  --task task-1 \
  --profile_id claude-code \
  --new_bench true \
  --permission ask

# Or start task in an existing bench
forja muster start \
  --task task-1 \
  --bench_path C:/work/forja-benches/sqlx-migration \
  --permission automatic
```

> [!NOTE]
> Forja enforces a concurrency cap controlled by `muster.dispatch.max_parallel` (default: 4 concurrent helpers). If the limit is reached, wait for active tasks to complete before dispatching new ones.

---

## 4. Monitoring Progress and the Inbox (`muster.status`, `muster.inbox`)

### Check overall run status
Inspect task states (`pending`, `running`, `delivered`, `failed`, `blocked`) and active assignments:
```sh
forja muster status --run run-1
```

### Review incoming notes and questions
Helpers send notes into the coordinator's inbox:
```sh
# List unread notes across the run
forja muster inbox --run run-1

# List all notes (including previously addressed items)
forja muster inbox --run run-1 --all true
```

---

## 5. Answering Helper Inquiries (`muster.reply`)

When a helper asks a question (note kind: `pergunta`), read the note ID and dispatch your decision:
```sh
forja muster reply \
  --note note-15 \
  --body "Use SQLite in WAL mode with a busy timeout of 5000ms. Do not create separate read and write pools."
```
The helper receives the reply directly and unblocks without manual shell interaction.

---

## 6. Reviewing and Accepting Deliveries

When a helper issues a `muster.handoff` with `--result feito`, the task state transitions to `delivered`.

1. **Verify evidence:** Check the `--evidence` field in the handoff note.
2. **Inspect changes:** Check git status and diffs on the worker's bench:
   ```sh
   forja diff.changes --bench_path C:/work/forja-benches/task-1-bench
   ```
3. **Dispatch dependent tasks:** Unblocked downstream tasks can now be started via `forja muster start`.
4. **Completion:** When every task in the run reaches `delivered`, the run state transitions to `done`.
