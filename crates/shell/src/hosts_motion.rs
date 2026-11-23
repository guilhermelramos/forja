//! Appearance keys for motion, tooltips and tab middle-click (#202, #204).
//!
//! Kept apart from the rest of the catalog hosts so `hosts.rs` stays within
//! the module size budget. Registration order is unchanged.

use crate::hosts::{flag, number};
use forja_kernel::SettingSpec;
#[cfg(test)]
use forja_kernel::{SettingKind, SettingValue};

/// The three order-5 appearance settings from item 15, in catalog order.
pub(crate) fn specs() -> Vec<SettingSpec> {
    vec![
        number(
            "appearance.motion.duration_ms",
            150.0,
            0.0,
            250.0,
            "setting-appearance-motion-duration_ms-label",
            "setting-appearance-motion-duration_ms-help",
            61,
        ),
        flag(
            "appearance.tooltips.enabled",
            true,
            "setting-appearance-tooltips-enabled-label",
            "setting-appearance-tooltips-enabled-help",
            62,
        ),
        flag(
            "appearance.tabs.middle_click",
            true,
            "setting-appearance-tabs-middle_click-label",
            "setting-appearance-tabs-middle_click-help",
            65,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::hosts;
    use forja_kernel::{Runtime, RuntimeOptions};

    fn test_runtime(tag: &str) -> (Runtime, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "forja-hosts-motion-test-{}-{tag}",
            std::process::id()
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
        .unwrap();
        (runtime, dir)
    }

    #[test]
    fn motion_duration_setting_is_wired_with_a_250ms_ceiling() {
        let (mut runtime, dir) = test_runtime("motion");
        let spec = runtime
            .registry()
            .settings()
            .iter()
            .find(|spec| spec.key == "appearance.motion.duration_ms")
            .expect("duration setting is registered");
        assert_eq!(spec.default, SettingValue::Number(150.0));
        assert!(matches!(
            spec.kind,
            SettingKind::Number {
                min: Some(0.0),
                max: Some(250.0)
            }
        ));
        runtime
            .set_setting("appearance.motion.duration_ms", SettingValue::Number(240.0))
            .unwrap();
        assert_eq!(
            runtime
                .settings()
                .values()
                .get("appearance.motion.duration_ms"),
            Some(&SettingValue::Number(240.0))
        );
        assert!(runtime
            .set_setting("appearance.motion.duration_ms", SettingValue::Number(400.0))
            .is_err());
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn tooltip_setting_is_wired_into_the_shared_ui_policy() {
        let (mut runtime, dir) = test_runtime("tooltips");
        let spec = runtime
            .registry()
            .settings()
            .iter()
            .find(|spec| spec.key == "appearance.tooltips.enabled")
            .expect("tooltip setting is registered");
        assert_eq!(spec.kind, SettingKind::Bool);
        assert_eq!(spec.default, SettingValue::Bool(true));
        runtime
            .set_setting("appearance.tooltips.enabled", SettingValue::Bool(false))
            .unwrap();
        assert_eq!(
            runtime
                .settings()
                .values()
                .get("appearance.tooltips.enabled"),
            Some(&SettingValue::Bool(false))
        );
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn middle_click_setting_is_wired_for_the_tab_strip() {
        let (mut runtime, dir) = test_runtime("middle");
        let spec = runtime
            .registry()
            .settings()
            .iter()
            .find(|spec| spec.key == "appearance.tabs.middle_click")
            .expect("middle-click setting is registered");
        assert_eq!(spec.kind, SettingKind::Bool);
        assert_eq!(spec.default, SettingValue::Bool(true));
        runtime
            .set_setting("appearance.tabs.middle_click", SettingValue::Bool(false))
            .unwrap();
        assert_eq!(
            runtime
                .settings()
                .values()
                .get("appearance.tabs.middle_click"),
            Some(&SettingValue::Bool(false))
        );
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }
}
