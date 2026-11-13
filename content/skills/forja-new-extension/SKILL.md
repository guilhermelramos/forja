---
name: forja-new-extension
description: Guide to extending Forja without recompilation, using declarative file content and sandboxed Lua extensions.
---

# Creating Extensions and Custom Content in Forja

Forja offers two levels of extensibility that require zero Rust compilation:
1. **File-based Content:** Declarative text files defining commands, recipes, themes, skills, and templates.
2. **Lua Extensions:** Lightweight scripts executing inside a capability-secured sandbox.

---

## 1. Extension Roots and Layers

Content is discovered across three hierarchical layers (lowest to highest precedence):
- **Built-in (`content/`):** Bundled with the application.
- **User (`~/.forja/`):** Personal preferences and extensions shared across all your projects.
- **Project (`<project>/.forja/`):** Version-controlled team settings and recipes.

```
~/.forja/ or <project>/.forja/
├── commands/*.md          # Anvil and chat slash-commands
├── recipes/*.toml         # Terminal recipes
├── helpers/*.toml         # Custom agent CLI profiles
├── skills/<name>/SKILL.md # Reusable agent skills
├── themes/*.toml          # Visual themes
├── keymaps/*.toml         # Keymap packs
├── templates/*.md         # Text templates (PR, handoff, note)
└── extensions/<id>/       # Lua extension package
    ├── extension.toml     # Manifest and capabilities
    ├── init.lua           # Script entry point
    └── i18n/              # Localized strings
```

> [!NOTE]
> Project-level content (`<project>/.forja/`) is untrusted by default when opening a new repository. Dangerous actions (running shell recipes or broad-permission helper commands) require explicit trust approval.

---

## 2. Declarative Content Types

### Anvil & Chat Commands (`commands/<name>.md`)
Commands use TOML frontmatter enclosed in `+++` followed by a prompt body:
```markdown
+++
title = { pt-BR = "Explicar erro", en = "Explain error" }
description = "Analyzes terminal output to diagnose build or test errors."
scope = ["anvil", "chat"]
run = "helper"
permissions = "ask"
context = ["diff", "bench"]
arguments = [ { name = "details", required = false, default = "" } ]
+++
Analyze the following error output: {{details}}.
Explain the root cause and provide exact code corrections.
```

### Terminal Recipes (`recipes/<name>.toml`)
Save reusable shell commands for quick terminal execution:
```toml
[[recipe]]
id = "workspace-lint"
title = { pt-BR = "Executar lint", en = "Run workspace lint" }
command = "cargo clippy --workspace --all-targets -- -D warnings"
scope = "project"
enter = true
directory = "."
```

### Skills (`skills/<name>/SKILL.md`)
Agent skills use YAML frontmatter (`---`) containing `name` and `description`:
```markdown
---
name: my-team-conventions
description: Coding and architectural guidelines for our service layer.
---
# Service Layer Guidelines
All services must implement idempotency checks on mutating requests.
```

---

## 3. Sandboxed Lua Extensions

### Scaffolding an extension
Generate a new extension using the xtask runner:
```sh
cargo xtask new-extension my_tool
```

### The Manifest (`extension.toml`)
Declare your extension's metadata and requested capabilities:
```toml
id = "my_tool"
name = "My Helper Tool"
version = "0.1.0"
author = "Your Name"
description = "Automates code analysis workflows."

[capabilities]
storage = true           # Access to extension-scoped key-value store
read_bench = true        # Read file list and current bench metadata
run_command = true       # Invoke registered Forja commands
```

### Script Entry Point (`init.lua`)
Implement extension lifecycle logic:
```lua
local forja = require("forja")

function forja.init(ctx)
    ctx.register_command("my_tool.greet", {
        title = { en = "Greet developer", ["pt-BR"] = "Cumprimentar desenvolvedor" },
        callback = function(args)
            ctx.notify("Hello from your custom Lua extension!")
        end
    })
end
```

### Security & Hash Approvals
- Forja computes a SHA-256 hash over all files in an extension folder.
- The user is prompted to inspect and approve requested capabilities.
- If any script or manifest file is modified, Forja invalidates the approval and requires re-confirmation before running.
