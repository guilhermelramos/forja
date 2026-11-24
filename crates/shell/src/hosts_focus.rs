//! Ajustes novos da primeira fatia do layout: Modo Foco e itens da barra.
//! Separados de `hosts.rs` para não engordar o catálogo já existente.

use forja_kernel::{SettingKind, SettingSpec, SettingValue};

/// Duas chaves de `appearance` registradas na mesma ordem do catálogo.
pub(crate) fn specs() -> Vec<SettingSpec> {
    vec![
        SettingSpec::new(
            "appearance.focus.hide_bars",
            SettingKind::Choice {
                options: vec![
                    "side_panels".to_owned(),
                    "chrome".to_owned(),
                    "all".to_owned(),
                ],
            },
            SettingValue::Text("chrome".to_owned()),
            "setting-appearance-focus-hide_bars-label",
            "setting-appearance-focus-hide_bars-help",
        )
        .order(106),
        SettingSpec::new(
            "appearance.status_bar.items",
            SettingKind::List {
                item: "id da contribuição".to_owned(),
            },
            SettingValue::List(
                crate::status_bar_items::DEFAULT_ITEMS
                    .iter()
                    .map(|id| (*id).to_owned())
                    .collect(),
            ),
            "setting-appearance-status_bar-items-label",
            "setting-appearance-status_bar-items-help",
        )
        .order(107),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::hosts;
    use forja_kernel::{Runtime, RuntimeOptions};

    #[test]
    fn focus_and_status_item_settings_are_registered_with_the_design_defaults() {
        let dir = std::env::temp_dir().join(format!(
            "forja-hosts-focus-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let runtime = Runtime::new(
            hosts(),
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: None,
                wake: None,
            },
        )
        .expect("hosts register");
        let hide = runtime
            .registry()
            .settings()
            .iter()
            .find(|spec| spec.key == "appearance.focus.hide_bars")
            .expect("hide_bars is registered");
        assert_eq!(hide.default, SettingValue::Text("chrome".to_owned()));
        assert!(matches!(hide.kind, SettingKind::Choice { .. }));
        let items = runtime
            .registry()
            .settings()
            .iter()
            .find(|spec| spec.key == "appearance.status_bar.items")
            .expect("status items are registered");
        assert_eq!(
            items.default,
            SettingValue::List(
                crate::status_bar_items::DEFAULT_ITEMS
                    .iter()
                    .map(|id| (*id).to_owned())
                    .collect()
            )
        );
        assert!(matches!(items.kind, SettingKind::List { .. }));
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }
}
