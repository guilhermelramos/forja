---
name: forja-new-module
description: How to build a new Rust feature module in Forja using cargo xtask new-module, including registration, i18n, slots, settings, and RPC.
---

# Building a Rust Module in Forja

In Forja, every distinct feature is an independent Rust crate living under `crates/modules/<name>/`. Modules register themselves into the kernel at runtime without the shell hardcoding any module names.

---

## 1. Scaffolding a Module (`cargo xtask new-module`)

Generate the crate structure using the repository xtask:
```sh
cargo xtask new-module my_feature
```

This creates:
```
crates/modules/my_feature/
├── Cargo.toml          # Preconfigured with kernel dependencies
├── MODULE.md           # Purpose, registration list, contracts, test guide
├── i18n/
│   ├── en.ftl          # English translations
│   └── pt-BR.ftl       # Portuguese (Brazil) translations
├── keymap.toml         # Default keyboard chords
└── src/lib.rs          # Registration and trait implementation
```
It also automatically adds `forja-module-my_feature = { path = "../modules/my_feature" }` between `# forja:modules:start` and `# forja:modules:end` in `crates/shell/Cargo.toml`. Never edit other files outside your module directory.

---

## 2. Implementing the `Module` Trait

Each module implements `forja_kernel::Module` and exports its factory:

```rust
use forja_kernel::{Module, ModuleManifest, Registrar};

pub struct MyFeatureModule;

impl Module for MyFeatureModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            id: "my_feature".into(),
            name_key: "my_feature-name".into(),
            version: "0.1.0".into(),
            depends_on: vec![],
        }
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        // Register resources, views, commands, settings, RPC
    }
}

#[forja_kernel::module]
static FACTORY: fn() -> Box<dyn Module> = || Box::new(MyFeatureModule);
```

---

## 3. What to Register in `register()`

### Internationalization & Keymaps
Always bundle both English and Brazilian Portuguese Fluent files:
```rust
r.i18n(include_str!("../i18n/en.ftl"), include_str!("../i18n/pt-BR.ftl"));
r.keymap(include_str!("../keymap.toml"));
```

### Settings
Settings follow the three-part key pattern `<module>.<group>.<name>`:
```rust
r.setting(SettingSpec::new(
    "my_feature.general.enabled",
    SettingKind::Bool,
    SettingValue::Bool(true),
    "setting-my_feature-general-enabled-label",
    "setting-my_feature-general-enabled-help",
));
```

### Commands
Commands are identified as `<module>.<action>` in English:
```rust
r.command("my_feature.run", "my_feature-command-run", |cx, args| {
    let target = args.str("target").unwrap_or("all");
    Ok(serde_json::json!({ "status": "ok", "target": target }))
});
```

### Views and Layout Slots
Mount views into available UI slots: `Slot::Rail` (left navigation page), `Slot::BenchTab` (central tab), `Slot::RightPanel`, `Slot::Sidebar`, or `Slot::StatusBar`:
```rust
r.view(Slot::BenchTab, "my_feature.tab", "my_feature-tab-title", 50, move || {
    Box::new(MyFeatureView::new())
});
```

### RPC and CLI Methods
Expose methods to the `forja` CLI and JSON-RPC pipe:
```rust
r.rpc(
    Method::new("my_feature.status", "my_feature-method-status-description")
        .effect(MethodEffect::Read)
        .idempotent(true)
        .param(ParamSchema::optional("bench_path", ParamKind::String))
        .result(ResultSchema::new("my_feature-method-status-result")),
    move |cx, params| {
        Ok(serde_json::json!({ "active": true }))
    },
);
```

---

## 4. Architectural Rules and Best Practices

- **Never block the UI thread:** File I/O, network requests, and subprocess execution must run inside background tasks using `cx.spawn(work, on_done)`.
- **Atomic state storage:** Use `cx.store()` to persist module JSON data into `%APPDATA%\Forja\modules\<id>.json`.
- **Language conventions:** All identifiers, configuration keys, function names, and file paths must be in English. Portuguese is strictly reserved for user-facing i18n text.
- **Size constraints:** Source files should not exceed ~600 lines, and a module crate should remain under ~5,000 lines.

---

## 5. Verification Checklist

Before finalizing your module, run the standard validation suite:
```sh
cargo test -p forja-module-my_feature
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask provenance
```
All four checks must pass cleanly.
