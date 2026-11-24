//! Comandos, chaves i18n e contrato do seletor de arranjos.
//!
//! O namespace `layout` é servido pelo shell para que os ids sigam o contrato
//! sem criar um módulo novo. Toda a fiação vive aqui, fora do `shell_module`.

use crate::actions::{self, Action};
use forja_kernel::Registrar;
use serde_json::json;

/// Chaves de interface usadas pelo seletor e pela edição de chrome.
pub(crate) const I18N_KEYS: &[&str] = &[
    "shell.command.layout-switch",
    "shell.command.layout-focus",
    "shell.command.layout-save",
    "shell.command.layout-reset",
    "shell.command.layout-manage",
    "shell.layout.label",
    "shell.layout.fallback",
    "shell.layout.save",
    "shell.layout.restore",
    "shell.layout.manage",
    "shell.layout.picker-title",
    "shell.layout.with-origin",
    "shell.layout.origin-builtin",
    "shell.layout.origin-user",
    "shell.layout.origin-project",
    "shell.layout.save-title",
    "shell.layout.save-hint",
    "shell.layout.save-name",
    "shell.layout.save-default",
    "shell.layout.save-confirm",
    "shell.layout.save-cancel",
    "shell.layout.save-invalid-name",
    "shell.layout.save-no-project",
    "shell.layout.settings-title",
    "shell.layout.settings-current",
    "shell.layout.settings-rail",
    "shell.layout.settings-status",
    "shell.layout.settings-rail-order",
    "shell.layout.focus-mode",
    "shell.layout.focus-exit",
    "shell.layout.focus-exit-tip",
    "shell.layout.status-items",
    "shell.layout.status-item",
    "shell.layout.status-items-menu",
    "shell.layout.up",
    "shell.layout.down",
    "shell.layout.hide",
    "shell.layout.show",
    "shell.layout.unavailable-ref-title",
    "shell.layout.unavailable-ref",
    "setting-appearance-layout-default-label",
    "setting-appearance-layout-default-help",
    "setting-appearance-focus-hide_bars-label",
    "setting-appearance-focus-hide_bars-help",
    "setting.appearance.focus.hide_bars.option.side_panels",
    "setting.appearance.focus.hide_bars.option.chrome",
    "setting.appearance.focus.hide_bars.option.all",
    "setting-appearance-status_bar-items-label",
    "setting-appearance-status_bar-items-help",
];

pub(crate) fn register_i18n(registrar: &mut Registrar<'_>) {
    for key in I18N_KEYS {
        registrar.uses_i18n(key);
    }
}

/// Comandos do seletor de arranjos e a tecla global `Ctrl+Alt+L`.
pub(crate) fn register(registrar: &mut Registrar<'_>) {
    registrar.contract_namespace("layout");
    registrar.command("layout.switch", "shell.command.layout-switch", |_, args| {
        if let Some(id) = args.str("id") {
            let layer = args
                .str("layer")
                .and_then(crate::layout_settings::LayoutLayer::parse)
                .unwrap_or_default();
            let name = args.str("name").unwrap_or(id).to_owned();
            actions::push(Action::SwitchLayout {
                id: id.to_owned(),
                layer,
                name,
            });
        } else {
            actions::push(Action::OpenLayoutPicker);
        }
        Ok(json!({ "opening": true }))
    });
    registrar.command(
        "layout.focus_mode",
        "shell.command.layout-focus",
        |_, _| {
            actions::dispatch("layout.focus_mode");
            Ok(json!({}))
        },
    );
    for (id, title) in [
        ("layout.save", "shell.command.layout-save"),
        ("layout.reset", "shell.command.layout-reset"),
        ("layout.manage", "shell.command.layout-manage"),
    ] {
        registrar.command(id, title, move |_, _| {
            actions::dispatch(id);
            Ok(json!({}))
        });
    }
    registrar.keymap(
        "[[binding]]\nid = \"layout.switch\"\nkey = \"Ctrl+Alt+L\"\nscope = \"global\"\n",
    );
    registrar.keymap(
        "[[binding]]\nid = \"layout.focus_mode\"\nkey = \"Ctrl+F11\"\nscope = \"global\"\n",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell_module::ShellModule;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use serde_json::Map;

    #[test]
    fn layout_commands_register_and_queue_selector_actions() {
        let dir = std::env::temp_dir().join(format!(
            "forja-shell-layout-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut runtime = Runtime::new(
            vec![Box::new(ShellModule)],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("shell module registers");
        contract::assert_ok(runtime.registry(), runtime.i18n());
        for id in [
            "layout.switch",
            "layout.focus_mode",
            "layout.save",
            "layout.reset",
            "layout.manage",
        ] {
            assert!(runtime.registry().command(id).is_some(), "{id}");
        }
        assert!(runtime
            .keymap()
            .bindings()
            .iter()
            .any(|binding| binding.binding.command.as_str() == "layout.switch"
                && binding.binding.chord.to_string() == "Ctrl+Alt+L"));
        assert!(runtime
            .keymap()
            .bindings()
            .iter()
            .any(|binding| binding.binding.command.as_str() == "layout.focus_mode"
                && binding.binding.chord.to_string() == "Ctrl+F11"));
        let _ = actions::drain();
        runtime
            .run_command(
                "layout.switch",
                forja_kernel::Args::from_map(Map::from_iter([
                    ("id".to_owned(), json!("foco")),
                    ("layer".to_owned(), json!("user")),
                    ("name".to_owned(), json!("Foco")),
                ])),
            )
            .expect("layout switch command");
        runtime
            .run_command("layout.save", forja_kernel::Args::new())
            .expect("layout save command");
        assert_eq!(
            actions::drain(),
            vec![
                Action::SwitchLayout {
                    id: "foco".to_owned(),
                    layer: crate::layout_settings::LayoutLayer::User,
                    name: "Foco".to_owned(),
                },
                Action::SaveLayout,
            ]
        );
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }
}
