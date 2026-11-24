//! Catalog hosts and the commands the shell chrome owns.
//!
//! Setting keys must start with a registered module id. Appearance, language,
//! the tray and the notification prefix therefore exist as modules even though
//! they are part of the shell. `notify` registers the `notifications.*` values.

use crate::actions;
use forja_kernel::{Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue};
use serde_json::json;

pub struct Host {
    pub id: &'static str,
    pub name_key: &'static str,
}

impl Module for Host {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new(self.id, self.name_key, env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, registrar: &mut Registrar<'_>) {
        match self.id {
            "appearance" => register_appearance(registrar),
            "app" => register_app(registrar),
            "i18n" => register_i18n(registrar),
            "keymap" => register_keymap(registrar),
            "anvil" => {
                for (id, title) in crate::anvil_keys::COMMANDS {
                    registrar.command(id, title, |_cx, _args| Ok(json!({})));
                }
                registrar.keymap(&crate::anvil_keys::keymap_text());
                bind(registrar, "anvil.open", "anvil.command.open", "Ctrl+K");
                bind(
                    registrar,
                    "anvil.command_mode",
                    "anvil.command.mode",
                    "Ctrl+Shift+P",
                );
                registrar.setting(number(
                    "anvil.history.limit",
                    100.0,
                    10.0,
                    1000.0,
                    "setting-anvil-history-limit-label",
                    "setting-anvil-history-limit-help",
                    10,
                ));
                registrar.setting(flag(
                    "anvil.history.show_recent",
                    true,
                    "setting-anvil-history-show_recent-label",
                    "setting-anvil-history-show_recent-help",
                    20,
                ));
            }
            "view" => {
                bind(
                    registrar,
                    "view.zoom.in",
                    "view.command.zoom_in",
                    "Ctrl+Shift+Plus",
                );
                bind(
                    registrar,
                    "view.zoom.out",
                    "view.command.zoom_out",
                    "Ctrl+Shift+Minus",
                );
                bind(
                    registrar,
                    "view.zoom.reset",
                    "view.command.zoom_reset",
                    "Ctrl+Shift+0",
                );
            }
            "window" => bind(
                registrar,
                "window.fullscreen",
                "window.command.fullscreen",
                "F11",
            ),
            "find" => {
                registrar.i18n(
                    "find-name = Find\nfind-command-file = Find file\n",
                    "find-name = Achar\nfind-command-file = Achar arquivo\n",
                );
                bind(registrar, "find.file", "find.command.file", "Ctrl+P");
            }
            "settings" => {
                bind(
                    registrar,
                    "settings.open",
                    "settings.command.open",
                    "Ctrl+Comma",
                );
                bind(
                    registrar,
                    "settings.keymap",
                    "settings.command.keymap",
                    "Ctrl+Shift+Comma",
                );
            }
            "help" => bind(
                registrar,
                "help.keymap",
                "help.command.keymap",
                "Ctrl+Shift+Slash",
            ),
            "tab" => register_tabs(registrar),
            "sidebar" => bind(
                registrar,
                "sidebar.toggle",
                "sidebar.command.toggle",
                "Ctrl+B",
            ),
            "panel" => bind(
                registrar,
                "panel.toggle",
                "panel.command.toggle",
                "Ctrl+Shift+B",
            ),
            _ => {}
        }
    }
}

pub fn hosts() -> Vec<Box<dyn Module>> {
    let specs = [
        ("appearance", "appearance.name"),
        ("app", "app.name"),
        ("i18n", "i18n.name"),
        ("keymap", "keymap.name"),
        ("notifications", "notifications.name"),
        ("anvil", "anvil.name"),
        ("find", "find.name"),
        ("settings", "settings.name"),
        ("sidebar", "sidebar.name"),
        ("panel", "panel.name"),
        ("view", "view.name"),
        ("window", "window.name"),
        ("tab", "tab.name"),
        ("help", "help.name"),
    ];
    specs
        .into_iter()
        .map(|(id, name_key)| Box::new(Host { id, name_key }) as Box<dyn Module>)
        .collect()
}

fn register_tabs(registrar: &mut Registrar<'_>) {
    let bindings = [
        ("tab.focus", "tab.command.focus", "F6"),
        ("tab.next", "tab.command.next", "Ctrl+Tab"),
        ("tab.previous", "tab.command.previous", "Ctrl+Shift+Tab"),
        ("tab.close", "tab.command.close", "Ctrl+W"),
        ("tab.close_force", "tab.command.close_force", "Ctrl+Shift+W"),
        ("tab.reopen", "tab.command.reopen", "Ctrl+Shift+T"),
        ("tab.switch_1", "tab.command.switch_1", "Ctrl+1"),
        ("tab.switch_2", "tab.command.switch_2", "Ctrl+2"),
        ("tab.switch_3", "tab.command.switch_3", "Ctrl+3"),
        ("tab.switch_4", "tab.command.switch_4", "Ctrl+4"),
        ("tab.switch_5", "tab.command.switch_5", "Ctrl+5"),
        ("tab.switch_6", "tab.command.switch_6", "Ctrl+6"),
        ("tab.switch_7", "tab.command.switch_7", "Ctrl+7"),
        ("tab.switch_8", "tab.command.switch_8", "Ctrl+8"),
        ("tab.switch_9", "tab.command.switch_9", "Ctrl+9"),
        ("tab.switch_10", "tab.command.switch_10", "Ctrl+0"),
    ];
    for (id, title, key) in bindings {
        bind(registrar, id, title, key);
    }
}

fn bind(registrar: &mut Registrar<'_>, id: &'static str, title: &'static str, key: &str) {
    registrar.command(id, title, move |_cx, _args| {
        actions::dispatch(id);
        Ok(json!({}))
    });
    registrar.keymap(&format!(
        "[[binding]]\nid = \"{id}\"\nkey = \"{key}\"\nscope = \"global\"\n"
    ));
}

fn register_appearance(registrar: &mut Registrar<'_>) {
    for (id, title, mode) in [
        (
            "appearance.theme.light",
            "appearance.command.theme-light",
            "light",
        ),
        (
            "appearance.theme.dark",
            "appearance.command.theme-dark",
            "dark",
        ),
    ] {
        registrar.command(id, title, move |cx, _| {
            cx.set_setting("appearance.theme.mode", SettingValue::Text(mode.to_owned()))?;
            Ok(json!({ "mode": mode }))
        });
    }
    let mut specs = vec![
        choice(
            "appearance.theme.mode",
            &["system", "dark", "light"],
            "system",
            "setting-appearance-theme-mode-label",
            "setting-appearance-theme-mode-help",
            10,
        ),
        choice(
            "appearance.theme.dark_name",
            &["brasa_escuro", "carvao", "cinza_quente", "aco"],
            "brasa_escuro",
            "setting-appearance-theme-dark_name-label",
            "setting-appearance-theme-dark_name-help",
            20,
        ),
        choice(
            "appearance.theme.light_name",
            &["brasa_claro"],
            "brasa_claro",
            "setting-appearance-theme-light_name-label",
            "setting-appearance-theme-light_name-help",
            30,
        ),
        choice(
            "appearance.ui.density",
            &["comfortable", "compact"],
            "comfortable",
            "setting-appearance-ui-density-label",
            "setting-appearance-ui-density-help",
            40,
        ),
        number(
            "appearance.ui.scale",
            1.0,
            0.8,
            1.5,
            "setting-appearance-ui-scale-label",
            "setting-appearance-ui-scale-help",
            50,
        ),
        choice(
            "appearance.motion.reduce",
            &["auto", "always", "never"],
            "auto",
            "setting-appearance-motion-reduce-label",
            "setting-appearance-motion-reduce-help",
            60,
        ),
    ];
    specs.extend(crate::hosts_motion::specs());
    specs.extend(crate::hosts_focus::specs());
    specs.extend([
        flag(
            "appearance.sidebar.visible",
            true,
            "setting-appearance-sidebar-visible-label",
            "setting-appearance-sidebar-visible-help",
            70,
        ),
        number(
            "appearance.sidebar.width",
            260.0,
            180.0,
            480.0,
            "setting-appearance-sidebar-width-label",
            "setting-appearance-sidebar-width-help",
            80,
        ),
        flag(
            "appearance.rail.visible",
            true,
            "setting-appearance-rail-visible-label",
            "setting-appearance-rail-visible-help",
            90,
        ),
        flag(
            "appearance.status_bar.visible",
            true,
            "setting-appearance-status_bar-visible-label",
            "setting-appearance-status_bar-visible-help",
            100,
        ),
        SettingSpec::new(
            "appearance.layout.default",
            SettingKind::Text { max_len: Some(64) },
            SettingValue::Text("oficina".to_owned()),
            "setting-appearance-layout-default-label",
            "setting-appearance-layout-default-help",
        )
        .order(105),
    ]);
    for spec in specs {
        registrar.setting(spec);
    }
    registrar.setting(flag(
        "appearance.window.start_maximized",
        false,
        "setting-appearance-window-start_maximized-label",
        "setting-appearance-window-start_maximized-help",
        110,
    ));
    registrar.setting(flag(
        "appearance.window.remember_geometry",
        true,
        "setting-appearance-window-remember_geometry-label",
        "setting-appearance-window-remember_geometry-help",
        105,
    ));
    registrar.setting(SettingSpec::new(
        "appearance.colors.accent",
        SettingKind::Color,
        SettingValue::Text("#F2762E".to_owned()),
        "setting-appearance-colors-accent-label",
        "setting-appearance-colors-accent-help",
    ));
}

fn register_app(registrar: &mut Registrar<'_>) {
    registrar.setting(flag(
        "app.startup.open_last",
        true,
        "setting-app-startup-open_last-label",
        "setting-app-startup-open_last-help",
        2,
    ));
    registrar.setting(flag(
        "app.close.confirm",
        true,
        "setting-app-close-confirm-label",
        "setting-app-close-confirm-help",
        6,
    ));
    registrar.setting(flag(
        "app.tray.enabled",
        false,
        "setting-app-tray-enabled-label",
        "setting-app-tray-enabled-help",
        10,
    ));
    registrar.setting(flag(
        "app.cli.enabled",
        true,
        "setting-app-cli-enabled-label",
        "setting-app-cli-enabled-help",
        11,
    ));
    registrar.setting(
        SettingSpec::new(
            "app.storage.data_dir",
            SettingKind::Path,
            SettingValue::Text("%APPDATA%\\Forja".to_owned()),
            "setting-app-storage-data_dir-label",
            "setting-app-storage-data_dir-help",
        )
        .order(12),
    );
    registrar.setting(
        SettingSpec::new(
            "app.storage.content_dir",
            SettingKind::Path,
            SettingValue::Text("~/.forja".to_owned()),
            "setting-app-storage-content_dir-label",
            "setting-app-storage-content_dir-help",
        )
        .order(14),
    );
    registrar.setting(
        SettingSpec::new(
            "app.storage.cleanup_days",
            SettingKind::Integer {
                min: Some(0),
                max: Some(365),
            },
            SettingValue::Integer(30),
            "setting-app-storage-cleanup_days-label",
            "setting-app-storage-cleanup_days-help",
        )
        .order(16),
    );
    registrar.setting(choice(
        "app.log.level",
        &["error", "warn", "info", "debug", "trace"],
        "info",
        "setting-app-log-level-label",
        "setting-app-log-level-help",
        20,
    ));
    registrar.setting(number(
        "app.log.max_mb",
        20.0,
        1.0,
        100.0,
        "setting-app-log-max_mb-label",
        "setting-app-log-max_mb-help",
        30,
    ));
    registrar.setting(number(
        "app.log.keep",
        5.0,
        1.0,
        20.0,
        "setting-app-log-keep-label",
        "setting-app-log-keep-help",
        40,
    ));
    registrar.setting(flag(
        "app.log.include_terminal_output",
        false,
        "setting-app-log-include_terminal_output-label",
        "setting-app-log-include_terminal_output-help",
        50,
    ));
    registrar.setting(number(
        "app.performance.max_fps",
        60.0,
        0.0,
        240.0,
        "setting-app-performance-max_fps-label",
        "setting-app-performance-max_fps-help",
        55,
    ));
    registrar.setting(number(
        "app.performance.unfocused_fps",
        10.0,
        0.0,
        120.0,
        "setting-app-performance-unfocused_fps-label",
        "setting-app-performance-unfocused_fps-help",
        58,
    ));
    registrar.setting(flag(
        "app.diagnostics.overlay",
        false,
        "setting-app-diagnostics-overlay-label",
        "setting-app-diagnostics-overlay-help",
        70,
    ));
    registrar.setting(number(
        "app.performance.worker_threads",
        0.0,
        0.0,
        32.0,
        "setting-app-performance-worker_threads-label",
        "setting-app-performance-worker_threads-help",
        60,
    ));
}

fn register_i18n(registrar: &mut Registrar<'_>) {
    registrar.setting(choice(
        "i18n.language.mode",
        &["auto", "pt_BR", "en"],
        "auto",
        "setting-i18n-language-mode-label",
        "setting-i18n-language-mode-help",
        10,
    ));
    registrar.setting(choice(
        "i18n.language.fallback",
        &["en", "pt_BR"],
        "en",
        "setting-i18n-language-fallback-label",
        "setting-i18n-language-fallback-help",
        20,
    ));
    registrar.setting(choice(
        "i18n.time.format",
        &["auto", "24h", "12h"],
        "auto",
        "setting-i18n-time-format-label",
        "setting-i18n-time-format-help",
        30,
    ));
    registrar.setting(choice(
        "i18n.date.week_start",
        &["auto", "sunday", "monday"],
        "auto",
        "setting-i18n-date-week_start-label",
        "setting-i18n-date-week_start-help",
        40,
    ));
}

fn register_keymap(registrar: &mut Registrar<'_>) {
    registrar.setting(
        SettingSpec::new(
            "keymap.file.custom_path",
            SettingKind::Path,
            SettingValue::Text("~/.forja/atalhos.toml".to_owned()),
            "setting-keymap-file-custom_path-label",
            "setting-keymap-file-custom_path-help",
        )
        .order(10),
    );
    registrar.setting(choice(
        "keymap.conflict.policy",
        &["warn", "block"],
        "warn",
        "setting-keymap-conflict-policy-label",
        "setting-keymap-conflict-policy-help",
        20,
    ));
}

pub(crate) fn flag(key: &str, default: bool, label: &str, help: &str, order: i32) -> SettingSpec {
    SettingSpec::new(
        key,
        SettingKind::Bool,
        SettingValue::Bool(default),
        label,
        help,
    )
    .order(order)
}

pub(crate) fn number(
    key: &str,
    default: f64,
    min: f64,
    max: f64,
    label: &str,
    help: &str,
    order: i32,
) -> SettingSpec {
    SettingSpec::new(
        key,
        SettingKind::Number {
            min: Some(min),
            max: Some(max),
        },
        SettingValue::Number(default),
        label,
        help,
    )
    .order(order)
}

fn choice(
    key: &str,
    options: &[&str],
    default: &str,
    label: &str,
    help: &str,
    order: i32,
) -> SettingSpec {
    SettingSpec::new(
        key,
        SettingKind::Choice {
            options: options.iter().map(|option| (*option).to_owned()).collect(),
        },
        SettingValue::Text(default.to_owned()),
        label,
        help,
    )
    .order(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{Args, Runtime, RuntimeOptions};

    #[test]
    fn theme_commands_change_the_mode() {
        let dir = std::env::temp_dir().join(format!("forja-theme-test-{}", std::process::id()));
        let mut runtime = Runtime::new(
            hosts(),
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: None,
                wake: None,
            },
        )
        .unwrap();
        for mode in ["light", "dark"] {
            runtime
                .run_command(&format!("appearance.theme.{mode}"), Args::new())
                .unwrap();
            assert_eq!(
                runtime
                    .settings()
                    .values()
                    .get("appearance.theme.mode")
                    .and_then(SettingValue::as_str),
                Some(mode)
            );
        }
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

}
