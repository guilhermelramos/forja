# AGENTS.md — Working Guidelines for AI Agents and Contributors

Welcome to the project. This guide establishes technical conventions, operational boundaries, and quality requirements for human developers and AI agents working in this repository.

---

## 1. Core Principles

1. **Hermetic and local-first:** All tests, scripts, and build tasks must run locally without external network dependencies during routine test passes.
2. **Deterministic execution:** Never pause waiting for interactive terminal input. Always pass non-interactive flags (e.g. `--yes`, `--quiet`, `-y`).
3. **Explicit error handling:** Avoid unwraps and swallowed exceptions. Return typed errors with contextual diagnostic messages.
4. **Clean Git hygiene:** Keep commits atomic, well-tested, and accompanied by meaningful commit messages following Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`).

---

## 2. Repository Layout

```
project-root/
├── src/               # Application source code
├── tests/             # Integration and end-to-end tests
├── docs/              # User and architecture documentation
├── forja.toml         # Per-repository Forja configuration
├── .forja/            # Optional project-specific Forja extensions
│   ├── commands/      # Custom Anvil / chat commands (*.md)
│   ├── recipes/       # Saved terminal recipes (*.toml)
│   └── skills/        # Project-specific agent skills
└── README.md          # Project overview and quick start
```

---

## 3. Standard Verification Recipes

Always run the test and linter suite before submitting changes or declaring a task complete:

- **Build:** Compile the project in debug mode to check syntax and type constraints.
- **Test:** Run the automated unit and integration tests.
- **Lint:** Run the project linter with zero warnings tolerated.
- **Format:** Ensure all code adheres to project formatting rules.

---

## 4. Working with Forja Benches

When working inside a Forja bench:
- Confine all file changes to your assigned worktree. Never touch files in other checkouts or global system paths.
- Keep diffs focused: do not mix unrelated refactorings or reformatting passes with functional feature work.
- Use `forja` CLI commands when coordinating within a Mutirão (Muster): report progress with `forja muster note` and conclude with `forja muster handoff --result feito --evidence "..."`.
