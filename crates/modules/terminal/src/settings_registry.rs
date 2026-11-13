//! Esquema das configurações do grupo Terminal (inclui `terminal.recipes.*`).

use forja_kernel::{Registrar, SettingKind, SettingSpec, SettingValue};

pub(super) fn register_settings(r: &mut Registrar<'_>) {
    let mut add = |key: &str, kind: SettingKind, default: SettingValue| {
        r.setting(SettingSpec::new(
            key,
            kind,
            default,
            format!("setting-{}-label", key.replace('.', "-")),
            format!("setting-{}-help", key.replace('.', "-")),
        ));
    };
    add(
        "terminal.shell.default",
        SettingKind::Choice {
            options: ["auto", "pwsh", "powershell", "cmd", "custom"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("auto".to_owned()),
    );
    add(
        "terminal.shell.path",
        SettingKind::Path,
        SettingValue::Text(String::new()),
    );
    add(
        "terminal.shell.start_dir",
        SettingKind::Choice {
            options: ["bench", "project", "home"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("bench".to_owned()),
    );
    add(
        "terminal.shell.env",
        SettingKind::List {
            item: "NOME=valor".to_owned(),
        },
        SettingValue::List(Vec::new()),
    );
    add(
        "terminal.scroll.history_lines",
        SettingKind::Integer {
            min: Some(1_000),
            max: Some(200_000),
        },
        SettingValue::Integer(10_000),
    );
    add(
        "terminal.scroll.wheel_lines",
        SettingKind::Integer {
            min: Some(1),
            max: Some(10),
        },
        SettingValue::Integer(3),
    );
    add(
        "terminal.scroll.follow_output",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "terminal.cursor.style",
        SettingKind::Choice {
            options: ["block", "bar", "underline"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("block".to_owned()),
    );
    add(
        "terminal.font.family",
        SettingKind::Choice {
            options: ["inherit", "cascadia_code", "cascadia_mono", "consolas"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("inherit".to_owned()),
    );
    add(
        "terminal.font.size",
        SettingKind::Number {
            min: Some(8.0),
            max: Some(32.0),
        },
        SettingValue::Number(13.0),
    );
    add(
        "terminal.theme.name",
        SettingKind::Choice {
            options: ["brasa_escuro", "brasa_claro", "carvao", "cinza_quente", "aco"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("brasa_escuro".to_owned()),
    );
    add(
        "terminal.theme.follow_app",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "terminal.color.opacity",
        SettingKind::Number {
            min: Some(0.6),
            max: Some(1.0),
        },
        SettingValue::Number(1.0),
    );
    add(
        "terminal.bell.mode",
        SettingKind::Choice {
            options: ["none", "visual", "audible", "both"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("visual".to_owned()),
    );
    add(
        "terminal.copy.on_select",
        SettingKind::Bool,
        SettingValue::Bool(false),
    );
    add(
        "terminal.recipes.press_enter",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "terminal.recipes.confirm",
        SettingKind::Bool,
        SettingValue::Bool(false),
    );
    add(
        "terminal.recipes.show_scope",
        SettingKind::Choice {
            options: ["all", "global", "project"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("all".to_owned()),
    );
    add(
        "terminal.recipes.shell",
        SettingKind::Choice {
            options: ["inherit", "pwsh", "powershell", "cmd"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("inherit".to_owned()),
    );
    add(
        "terminal.copy.trim",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "terminal.paste.warn_multiline",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "terminal.paste.bracketed",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "terminal.links.confirm",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "terminal.links.open_with",
        SettingKind::Choice {
            options: ["system", "app"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("system".to_owned()),
    );
    add(
        "terminal.shell.marks",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "terminal.close.confirm_running",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "terminal.gpu.mode",
        SettingKind::Choice {
            options: ["auto", "on", "off"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("auto".to_owned()),
    );
    add(
        "terminal.osc52.enabled",
        SettingKind::Bool,
        SettingValue::Bool(false),
    );
}


