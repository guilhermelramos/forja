# Helper profile (TOML format)

> A **helper profile** (`profile`) tells Forja how to find, start, pass the initial prompt, ask for permissions, resume and fork a session, choose a model and locate the transcripts of a coding-agent CLI. It is a text file: edit it without recompiling.

Where profiles live (the same format in each place):
- `content/helpers/*.toml`: built-in, ours;
- `~/.forja/helpers/*.toml`: user;
- `<project>/.forja/helpers/*.toml`: project.

Concept and vocabulary: `design/product.md`, section 2 ("Perfil de ajudante", id `profile`). The list of profiles and the default order: `design/helpers.md`.

## 1. Rules

1. **Official source per field.** Each value comes from the vendor's documentation or from the CLI's own `--help`/`--version`. The source is cited in `[sources]`. No data comes from another product of the same kind (AGENTS.md, section 1).
2. **Only confirmed content.** An unconfirmed field stays commented with `# a confirmar`. A missing field means Forja does not offer that function for the CLI. Forja never invents an equivalent.
3. **Placeholders** in braces are replaced by Forja when it builds the command: `{prompt}` (the prompt text), `{id}` (the session id or name that the Archive stores), `{model}` and `{effort}`.
4. **Nothing is written outside Forja without consent.** A profile describes where the CLI keeps its settings and hooks, but Forja only writes to those files with explicit, recorded consent (product, section 5; AGENTS.md, section 4).
5. **Windows executables.** The lookup in `[detect]` tries the name and also `.exe`, `.cmd` and `.ps1`. npm installers create `.ps1` shims (on this machine: `opencode.ps1`, `gemini.ps1`).
6. **Identifiers in English.** Keys, section names, mode names and placeholders are in English. Comments and the text shown to people are in Portuguese (AGENTS.md, section 4).

## 2. Fields

| Section | Field | Type | Meaning |
|---|---|---|---|
| root | `id` | text | lowercase identifier with hyphens (`claude-code`, `codex`); unique in the catalog |
| root | `name` | text | display name of the CLI |
| root | `maker` | text | who maintains the CLI |
| root | `site` | URL | main official documentation |
| root | `status` | text, optional | `discontinued` or `migrated` when the vendor ended or replaced the product; absent = active |
| `[detect]` | `executables` | list | names to search for in PATH |
| `[detect]` | `version` | list | args that print the version (`["--version"]`) |
| `[start]` | `command` | text | executable of the interactive mode |
| `[start]` | `args` | list | default args of the interactive mode |
| `[prompt]` | `mode` | `argument` \| `flag` \| `stdin` \| `none` | how the initial prompt enters the interactive mode (section 4) |
| `[prompt]` | `flag` | text | only with `mode = "flag"`: the flag that receives the prompt |
| `[prompt]` | `headless` | list | **complete** args of the non-interactive mode (prints and exits), with `{prompt}` |
| `[prompt]` | `headless_stdin` | boolean | the CLI accepts the prompt on stdin in non-interactive mode |
| `[permissions.<mode>]` | `args`, `env` | list, table | flags and environment variables of the mode (section 3) |
| `[resume]` | `last`, `id` | list | **complete** args to resume the most recent session of the directory and a session by id |
| `[resume.headless]` | `last`, `id` | list | the same, in non-interactive mode, when it differs |
| `[fork]` | `last`, `id`, `adds` | list | **complete** args to fork a session; `adds` is a flag appended to `resume` |
| `[fork.headless]` | `last`, `id`, `adds` | list | the same, in non-interactive mode |
| `[models]` | `model_args` | list | args to choose the model, with `{model}` |
| `[models]` | `effort_args` | list | args for the effort level, with `{effort}` |
| `[models]` | `effort_values` | list | accepted effort values |
| `[models]` | `model_env` | text | environment variable that chooses the model |
| `[models]` | `effort_env` | text | environment variable that sets the effort, when the CLI has no flag for it |
| `[sessions]` | `format` | `jsonl` \| `sqlite` \| `markdown` \| `server` \| `to-confirm` | format of the transcripts |
| `[sessions]` | `path` | text | path or pattern where the CLI writes; `~` is the user folder |
| `[sessions]` | `dir_env` | text | variable that changes the base directory |
| `[sessions]` | `list`, `export`, `create`, `delete` | list | CLI subcommands that list, export, create or delete a session (when they exist) |
| `[sessions]` | `confidence` | `high` \| `medium` \| `low` | confidence in this section |
| `[hooks]` | `supported` | boolean | the CLI calls external hooks |
| `[hooks]` | `mechanism` | `command` \| `plugin` | external command declared in configuration, or a plugin in code |
| `[hooks]` | `config` | list | files where hooks are declared |
| `[hooks]` | `key` | text | key inside those files, when there is one |
| `[hooks]` | `events` | list | event names the source documents |
| `[sources]` | `date` | `YYYY-MM-DD` | day the source was read |
| `[sources]` | `urls` | list | official pages read |
| `[sources]` | `commands` | list | commands run locally (only `--version` and `--help`) |
| `[sources]` | `confidence` | `high` \| `medium` \| `low` | see section 6 |

## 3. Permission modes (our decision)

Forja has four modes. Each CLI maps its own modes onto them, when it can.

| Mode | Forja's promise | Example (Claude Code) |
|---|---|---|
| `ask` | The CLI asks for confirmation before each change. | no flag (the CLI default) |
| `accept-edits` | File edits pass without asking. The rest follows the CLI default. | `--permission-mode acceptEdits` |
| `automatic` | A classifier or a rule list decides. The CLI asks only about what does not pass. | `--permission-mode auto` |
| `full` | No confirmation, including shell commands. Only in an isolated environment. | `--dangerously-skip-permissions` |

- If the CLI has no such mode, the section stays commented. If the mapping is approximate, it also stays commented with `# a confirmar` and an explanation.
- When the mode depends on a rules file of the CLI (for example `opencode.json`), the profile does not write it: the section stays `# a confirmar` and Forja asks for consent before any write.
- When the CLI forbids the mode together with the prompt (Kimi Code: `--yolo`, `--auto` and `--plan` do not combine with `--prompt`), the section stays in the profile with the warning, and Forja does not offer flag-based permissions in non-interactive mode for that CLI.
- Settings-only modes (a settings key with no flag) do not go into `args`. When the CLI only has the key (for example `toolPermission` in Antigravity), the section stays commented with the key named.

## 4. Prompt delivery

- `argument`: the prompt is the last argument of `[start]`. Example: `claude "<prompt>"`.
- `flag`: the flag in `[prompt].flag` followed by the prompt. Example: `gemini -i "<prompt>"`.
- `stdin`: the prompt is written to stdin when the CLI is ready. The sessions module detects that moment.
- `none`: there is no initial prompt. Forja types the text after the CLI opens.
- `headless`: the list is complete. If it contains `{prompt}`, the text replaces it there. If not, the text goes at the end.

## 5. Resume and fork

- The lists in `[resume]` and `[fork]` are **complete**, starting from the executable. They are not appended to `[start].args`. Example: Codex uses `["resume", "--last"]`, and Claude Code uses `["--continue"]`.
- `last` resumes the most recent session of the current directory. `id` uses `{id}`, which is the id or the name that Forja stores in the Archive.
- `fork.adds` is a flag appended to `resume` (Claude Code: `--fork-session`). When forking is a subcommand (Codex: `fork`), use `last` and `id` in `[fork]` itself.

## 6. Confidence levels

- **high**: flags and variables checked in the local `--help` or `--version`, or official documentation read in full.
- **medium**: official documentation read as a summary or in parts, or a section with a marked inference in its comment.
- **low**: only search results, archived or old-version documentation, or third-party material. Almost everything stays commented in that case.

## 7. Archive and hooks

- **Archive.** `[sessions]` says where to read the transcripts. The Archive reads without writing to the CLI's files. When the CLI keeps sessions on a server (Amp), `format = "server"` and the Archive uses `export`.
- **Hooks.** The profile describes where the CLI reads hooks (`supported`, `mechanism`, `config`, `events`). It installs nothing. For `forja-hook` to receive status events from helpers, it must be registered in the CLI's settings, and that requires the recorded consent of section 1.

## 8. Example (excerpt of `content/helpers/claude-code.toml`)

```toml
id = "claude-code"
name = "Claude Code"
maker = "Anthropic"
site = "https://code.claude.com/docs"

[detect]
executables = ["claude"]
version = ["--version"]          # output: "2.1.322 (Claude Code)"

[start]
command = "claude"
args = []

[prompt]
mode = "argument"                # claude "<prompt>" opens the session with the prompt
headless = ["-p", "{prompt}"]    # claude -p "<prompt>": prints and exits
headless_stdin = true

[permissions.accept-edits]
args = ["--permission-mode", "acceptEdits"]
env = {}

[resume]
last = ["--continue"]
id = ["--resume", "{id}"]

[fork]
adds = ["--fork-session"]
```

## 9. Validation

- Each file must be valid TOML. Today the check is done with `tomllib` (Python 3.11 or newer), reading the files in `content/helpers/`.
- The schema check (required fields, `mode` values, `permissions` names, placeholders) enters with the profiles module in phase B. Until then the review is manual, against the sources in `[sources]`.
- `cargo xtask provenance` must pass. It rejects terms and identifiers in the provenance list.
