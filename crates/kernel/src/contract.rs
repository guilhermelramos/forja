//! Contract checks that run over every linked module.
//!
//! The shell's integration test instantiates all modules discovered through
//! `linkme` and calls [`check`]. A violation fails the test suite, so a module
//! cannot ship with duplicate ids, missing translations, invalid defaults or
//! conflicting shortcuts.

use crate::i18n::{I18n, Locale};
use crate::registry::Registry;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub rule: &'static str,
    pub detail: String,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.rule, self.detail)
    }
}

/// Checks ids, translations, setting defaults and shortcut conflicts.
pub fn check(registry: &Registry, i18n: &I18n) -> Vec<Violation> {
    let mut violations = Vec::new();

    let mut modules = BTreeSet::new();
    for module in registry.modules() {
        if !modules.insert(module.id.clone()) {
            violations.push(violation("ids", format!("duplicate module `{}`", module.id)));
        }
    }

    let mut commands = BTreeSet::new();
    for command in registry.commands() {
        if !commands.insert(command.id.as_str().to_owned()) {
            violations.push(violation(
                "ids",
                format!("duplicate command `{}`", command.id),
            ));
        }
    }

    let mut views = BTreeSet::new();
    for view in registry.views() {
        if !views.insert((view.slot, view.id.clone())) {
            violations.push(violation(
                "ids",
                format!("duplicate view `{}` in `{}`", view.id, view.slot.id()),
            ));
        }
    }

    let mut settings = BTreeSet::new();
    for setting in registry.settings() {
        if !settings.insert(setting.key.clone()) {
            violations.push(violation(
                "ids",
                format!("duplicate setting `{}`", setting.key),
            ));
        }
        if let Err(reason) = setting.validate_value(&setting.default) {
            violations.push(violation(
                "settings",
                format!("`{}` has an invalid default: {reason}", setting.key),
            ));
        }
    }

    let mut methods = BTreeSet::new();
    for method in registry.methods() {
        if !methods.insert(method.schema.name.clone()) {
            violations.push(violation(
                "ids",
                format!("duplicate RPC method `{}`", method.schema.name),
            ));
        }
        let mut params = BTreeSet::new();
        for param in &method.schema.params {
            if !params.insert(param.name.clone()) {
                violations.push(violation(
                    "ids",
                    format!(
                        "RPC method `{}` repeats parameter `{}`",
                        method.schema.name, param.name
                    ),
                ));
            }
        }
    }

    for key in registry.i18n_keys() {
        if !i18n.has(Locale::En, &key) {
            violations.push(violation(
                "i18n",
                format!("key `{key}` is missing from en"),
            ));
        }
        if !i18n.has(Locale::PtBr, &key) {
            violations.push(violation(
                "i18n",
                format!("key `{key}` is missing from pt-BR"),
            ));
        }
    }

    for conflict in registry.keymap_conflicts() {
        violations.push(violation(
            "keymap",
            format!(
                "`{}` is bound to `{}` and `{}` in scope `{}`",
                conflict.chord, conflict.first, conflict.second, conflict.scope
            ),
        ));
    }

    for binding in registry.keybinds() {
        if !commands.contains(binding.command.as_str()) {
            violations.push(violation(
                "keymap",
                format!(
                    "`{}` points to unknown command `{}`",
                    binding.chord, binding.command
                ),
            ));
        }
    }

    violations
}

/// Panics with every violation; used by integration tests.
pub fn assert_ok(registry: &Registry, i18n: &I18n) {
    let violations = check(registry, i18n);
    assert!(
        violations.is_empty(),
        "module contract violations:\n{}",
        violations
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn violation(rule: &'static str, detail: String) -> Violation {
    Violation { rule, detail }
}
