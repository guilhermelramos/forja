# Helpers: profiles of the coding-agent CLIs

> Summary of the profiles in `content/helpers/*.toml`. Format of each file: `design/helper-profile.md`. Sources read on **2026-11-12**, from each tool's official documentation (each profile cites its URLs and commands in `[sources]`).

## 1. Summary table

Legend: **permissions** = how many of the four modes (`ask`, `accept-edits`, `automatic`, `full`) are confirmed. **Resume** = continue the last session or one by id. **Sessions** = path or subcommand that the Archive uses. **Confidence** = `high` (flags checked in the local `--help`, with documentation read in full), `medium` (official documentation read as a summary or in parts) or `low` (search results or incomplete documentation).

| Name | File (id) | Prompt (interactive / non-interactive) | Permissions | Resume and fork | Sessions | Hooks | Confidence | Status |
|---|---|---|---|---|---|---|---|---|
| Claude Code | `claude-code.toml` (`claude-code`) | argument / `-p` | 4 of 4 | yes (`--continue`, `--resume`) and fork (`--fork-session`) | JSONL in `~/.claude/projects/` | yes, 33 events | high | installed here (2.1.322) |
| OpenAI Codex CLI | `codex.toml` (`codex`) | argument / `exec` | 3 of 4 (`ask` to confirm) | yes (`resume --last`, `resume <id>`) and fork (`fork`) | `rollout-*.jsonl` in `$CODEX_HOME/sessions` (medium) | yes, 14 events | high | installed here (0.167.1) |
| OpenCode | `opencode.toml` (`opencode`) | `run` (the TUI has no documented prompt) | 2 of 4 (`ask`, `full`); `accept-edits` and `automatic` depend on rules in `opencode.json` | yes (`--continue`, `--session`) and fork (`--fork`, in `run`) | SQLite `~/.local/share/opencode/opencode.db`; `session list` and `session export` | plugins in JS/TS | medium | installed here (v2.2.6) |
| Gemini CLI | `gemini.toml` (`gemini`) | argument / `-p` | 3 of 4 (`automatic` to confirm) | yes (`--resume latest`, index, UUID); no fork | `~/.gemini/tmp/<hash>/chats/` (summary) | yes, 11 events | medium | **discontinued** on 2026-06-18, replaced by the Antigravity CLI; installed here (0.67.0) |
| Antigravity CLI | `antigravity.toml` (`antigravity`) | `-i` / `-p` | 4 of 4 (`--mode accept-edits` and `--mode auto`; bypass with `--dangerously-skip-permissions`) | yes (`--continue`, `--conversation <id>`); works with `-p`; fork only in the TUI (`/fork`) | JSONL in `~/.gemini/antigravity-cli/brain/` (example from the hooks page) | yes, 6 events (command) | medium | not installed; **replaces the Gemini CLI** |
| GitHub Copilot CLI | `copilot.toml` (`copilot`) | `-i` / `-p` | 4 of 4 (`accept-edits` with `--allow-tool=write`; `automatic` only non-interactive) | yes (`--continue`, `--resume`); fork only through `/fork` | `events.jsonl` in `~/.copilot/session-state/` (summary) | yes, 14 events | medium | not installed |
| Aider | `aider.toml` (`aider`) | NOT FOUND / `--message` | 1 of 4 (`full`, `--yes-always`) | partial (`--restore-chat-history`, no id) | `.aider.chat.history.md` (markdown) | not found (`--notifications-command` covers notifications only) | medium | not installed |
| Goose | `goose.toml` (`goose`) | `goose session` without prompt / `goose run -t` | 3 of 4 through `GOOSE_MODE` (`ask`, `automatic`, `full`) | yes (`session --resume`, `--session-id`) and fork (`--fork`) | SQLite `sessions.db` | yes, plugins (partial events) | medium | not installed |
| Amp | `amp.toml` (`amp`) | NOT FOUND / `-x` | 0 of 4 by flag (approval is by plugin) | by id (`threads continue T-...`); no fork | server; `threads export` | yes, plugins in JS/TS (6 events) | medium (permissions: low) | not installed |
| Qwen Code | `qwen-code.toml` (`qwen-code`) | `-i` / `-p` | 4 of 4 through `--approval-mode` (`ask` with a default conflict) | yes (`--continue`, `--resume <id>`); no fork | JSONL in `~/.qwen/projects/<cwd>/chats/` | yes, 23 events | medium | not installed |
| Kimi Code CLI | `kimi-code.toml` (`kimi-code`) | NOT FOUND / `-p` | 3 of 4 (`ask`, `automatic` `--yolo`, `full` `--auto`); they do not combine with `-p` | yes (`--continue`, `--session <id>`); fork only in the TUI | JSONL in `~/.kimi-code/sessions/`; `kimi export` | yes, 20 events (in `config.toml`) | medium | active; succeeds the Kimi CLI (archived) |
| Crush | `crush.toml` (`crush`) | NOT FOUND / NOT FOUND | 2 of 4 (`ask`, `full` `--yolo`); allowlist is config | NOT FOUND | data in `$XDG_DATA_HOME/crush`; format NOT FOUND | yes, only `PreToolUse` (preliminary) | medium | not installed |
| Cline CLI | `cline.toml` (`cline`) | argument / `--json` | 1 of 4 (`full`, `--auto-approve true`) | by id (`--id`); `cline history` lists | `~/.cline/data/sessions/` (one folder per session) | to confirm | medium | not installed |
| Continue CLI | `continue.toml` (`continue`, executable `cn`) | NOT FOUND / `-p` | 3 of 4 (`ask`, `accept-edits` with `--allow`, `full` with `--auto`) | yes, only the last (`--resume`); no fork | NOT FOUND | NOT FOUND | medium | not installed; CLI in alpha |
| Kiro CLI | `kiro.toml` (`kiro`, executable `kiro-cli`) | argument (`chat`) / `chat --no-interactive` | 1 of 4 (`full`, `--trust-all-tools`) | yes (`--resume`, `--resume-id`); no fork by flag | `~/.kiro/sessions/` (SQLite, medium) | yes (V3), 7 events | medium | not installed |

### How to read the table

- **Installed here** means it was checked with `--version` and `--help` on this machine, on 2026-11-12, without login and without a session. None of the commands changed state.
- **Gemini CLI, discontinued.** The vendor ended individual use on 2026-06-18. The profile stays for people who still have access through an organization or an API key. The replacement is the Antigravity CLI.
- **Kimi CLI, replaced.** The old profile was replaced by the Kimi Code CLI (`kimi-code.toml`), which is the current product. The executable `kimi` is the same.
- Fields without an official source stay commented with `# a confirmar` in the file itself, and Forja treats a missing field as "not offered".

## 2. Default selection order (our decision)

When the user has not chosen a helper, Forja uses this order:

1. **Status first.** Profiles with status `discontinued` are never the default. They come last and are used only by manual choice.
2. **Installed first.** Forja searches each executable from `[detect].executables` in PATH (on Windows also `.exe`, `.cmd` and `.ps1`). Found executables come before the others.
3. **Most complete next.** Within each group, count what the profile confirms: the 4 permission modes; `resume.last`; `resume.id`; `fork`; `models.model_args`; `sessions.path`; and `hooks.supported`. Each item present counts 1, so the sum goes from 0 to 10. The highest sum comes first.
4. **Tie-break.** Source confidence (`high`, then `medium`, then `low`) and, last, the `id` in alphabetical order.
5. **Project default.** The first on the list. The user changes it at any time, and the change is stored in Forja's configuration. Nothing is written to another app's files without recorded consent.

Applying the rule to this machine on 2026-11-12:

| Order | Profile | Reason |
|---|---|---|
| 1 | Claude Code | installed, 10 of 10 |
| 2 | OpenAI Codex CLI | installed, 9 of 10 |
| 3 | OpenCode | installed, 8 of 10 |
| 4 | Cursor Agent CLI | installed, 5 of 10 |
| 5 | Antigravity CLI | **in place of the Gemini CLI**: not installed, 9 of 10, first of the not installed (tie with Copilot, Goose and Qwen Code, broken by `id`) |
| 6 | GitHub Copilot CLI | not installed, 9 of 10 |
| 7 | Goose | not installed, 9 of 10 |
| 8 | Qwen Code | not installed, 9 of 10 |
| 9 | Kimi Code CLI | not installed, 8 of 10 |
| 10 | Kiro CLI | not installed, 6 of 10 |
| 11 | Continue CLI | not installed, 5 of 10 |
| 12 | Aider | not installed, 4 of 10 |
| 13 | Cline CLI | not installed, 4 of 10 |
| 14 | Crush | not installed, 3 of 10 |
| 15 | Amp | not installed, 1 of 10 |
| out | Gemini CLI | discontinued (rule 1) |

Default helper on this machine: **Claude Code**. The Antigravity CLI takes the place of the Gemini CLI: it becomes the default only on a machine that has none of the first four installed.

## 3. Pending items for the coordinator

1. **Fields still without an official source** (marked `# a confirmar` in the file): Cursor (`ask` and hooks in the CLI), Codex (`ask` and the name of the transcript file), OpenCode (who maintains it and the effort), Aider (base directory of the history), Amp (flag-based permissions: there are none), Cline (`--continue` and hook execution), Continue (session id, npm package and Windows installation), Kiro (names of permission modes as flags), Goose (path of `sessions.db` and of the hooks on Windows) and Crush (version and resume).
2. **Effort values of Antigravity and Kimi**: the list differs between pages ("low through max" in the reference; "low, medium, high" in the headless page, for Antigravity). Confirm with `agy --help` and `kimi --help` once they are installed.
3. **Conflicts recorded in the profiles**: Cline's `--continue` (the search cites it, the reference does not), Qwen Code's default mode, Codex's transcript file name (`rollout-*.jsonl` or `history.jsonl`), and Copilot's stdin.
4. **Crush**: the pages read are on `next-docs.charm.land` (preview) and in the README. The current configuration is `crushrc`, and `crush.json` is legacy. Re-read when the documentation settles.
5. **Gemini CLI**: remove the profile when access through organizations and API keys ends. There is no official date for that.
6. **Tests**: the schema check enters with the profiles module (phase B). Today validation is valid TOML, the set of required fields, and review against `[sources]`.
