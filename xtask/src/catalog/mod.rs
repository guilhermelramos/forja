//! `cargo xtask catalog`: verifies `design/settings.toml` and `design/keymap.toml`
//! against the linked runtime and generates `design/coverage.md`.

mod compare;
mod report;
mod scan;

use crate::util::{read, repo_root, write};
use compare::{KeymapCatalog, RuntimeCatalog, SettingsCatalog};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

pub fn run() -> Result<(), String> {
    let root = repo_root();
    let settings_text = read(&root.join("design/settings.toml"))?;
    let keymap_text = read(&root.join("design/keymap.toml"))?;

    let settings_cat: SettingsCatalog = toml::from_str(&settings_text)
        .map_err(|error| format!("design/settings.toml: {error}"))?;
    let keymap_cat: KeymapCatalog =
        toml::from_str(&keymap_text).map_err(|error| format!("design/keymap.toml: {error}"))?;

    let runtime = dump_runtime_catalog(&root)?;
    let keys: BTreeSet<String> = runtime.settings.iter().map(|s| s.key.clone()).collect();
    let usage = scan::find_used_settings(&root, &keys)?;

    let coverage = compare::evaluate(settings_cat, keymap_cat, runtime, &usage);
    let invalid_scopes = coverage.invalid_scopes();
    if !invalid_scopes.is_empty() {
        return Err(format!(
            "invalid setting scopes in design/settings.toml: {}",
            invalid_scopes
                .iter()
                .map(|item| format!(
                    "{} ({})",
                    item.key,
                    item.scope_error.as_deref().unwrap_or("invalid")
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let invalid_deferred = coverage.invalid_deferred();
    if !invalid_deferred.is_empty() {
        return Err(format!("deferred settings must have a reason and must not be registered: {}",
            invalid_deferred.iter().map(|item| item.key.as_str()).collect::<Vec<_>>().join(", ")));
    }
    let markdown = report::render(&coverage);
    let coverage_path = root.join("design/coverage.md");
    write(&coverage_path, &markdown)?;

    let eff = coverage
        .settings
        .iter()
        .filter(|s| s.registered && s.has_effect)
        .count()
        + (coverage.extra_settings.len() - coverage.no_effect_extra_settings.len());
    let missing = coverage.settings.iter().filter(|s| !s.registered && s.deferred.is_none()).count();
    let key_reg = coverage.keybinds.iter().filter(|k| k.registered).count();
    let key_missing = coverage.keybinds.len() - key_reg;
    println!(
        "catalog: settings registradas {}/{} (+{} extras), com efeito {}, ausentes {}; atalhos {}/{} (+{} extras), ausentes {}; {}",
        coverage.runtime_settings_total,
        coverage.settings.len(),
        coverage.extra_settings.len(),
        eff,
        missing,
        coverage.runtime_keybinds_total,
        coverage.keybinds.len(),
        coverage.extra_keybinds.len(),
        key_missing,
        coverage_path.display()
    );

    let mismatches = coverage.type_or_default_mismatches();
    if !mismatches.is_empty() {
        for item in &mismatches {
            eprintln!(
                "catalog mismatch `{}`: tipo={:?}, padrão={:?}",
                item.key, item.type_mismatch, item.default_mismatch
            );
        }
        return Err(format!(
            "catalog check failed: {} registered setting(s) disagree with design/settings.toml on type or default",
            mismatches.len()
        ));
    }
    Ok(())
}

fn dump_runtime_catalog(root: &Path) -> Result<RuntimeCatalog, String> {
    let build = Command::new("cargo")
        .args(["build", "-p", "forja-app"])
        .current_dir(root)
        .status()
        .map_err(|error| format!("could not run cargo build -p forja-app: {error}"))?;
    if !build.success() {
        return Err(format!("cargo build -p forja-app failed with {build}"));
    }

    let output = Command::new("cargo")
        .args(["run", "-p", "forja-app", "--quiet", "--", "--dump-catalog"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("could not run forja-app --dump-catalog: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "forja-app --dump-catalog failed ({}): {stderr}",
            output.status
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("forja-app --dump-catalog emitted invalid UTF-8: {error}"))?;
    serde_json::from_str(&stdout)
        .map_err(|error| format!("invalid JSON from forja-app --dump-catalog: {error}"))
}
