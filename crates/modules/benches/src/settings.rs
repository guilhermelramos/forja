use forja_kernel::{Registrar, SettingKind, SettingSpec, SettingValue};
pub(crate) fn settings(r: &mut Registrar<'_>) {
    let mut add = |key: &str, kind: SettingKind, default: SettingValue, order: i32| {
        let (label, help) = match key {
            "bench.stage.creation_default" => (
                "setting-bench-stage-creation-default-label",
                "setting-bench-stage-creation-default-help",
            ),
            "bench.stage.wip_limit" => (
                "setting-bench-stage-limit-wip-label",
                "setting-bench-stage-limit-wip-help",
            ),
            "bench.stage.sync_request" => (
                "setting-bench-stage-sync-request-label",
                "setting-bench-stage-sync-request-help",
            ),
            "bench.stage.pr_open" => (
                "setting-bench-stage-pr-open-label",
                "setting-bench-stage-pr-open-help",
            ),
            "bench.stage.pr_merge" => (
                "setting-bench-stage-pr-merge-label",
                "setting-bench-stage-pr-merge-help",
            ),
            "bench.stage.archive_on_ready" => (
                "setting-bench-stage-archive-on_ready-label",
                "setting-bench-stage-archive-on_ready-help",
            ),
            other => {
                return r.setting(
                    SettingSpec::new(
                        key,
                        kind,
                        default,
                        format!("setting-{}-label", other.replace('.', "-")),
                        format!("setting-{}-help", other.replace('.', "-")),
                    )
                    .order(order),
                )
            }
        };
        r.setting(SettingSpec::new(key, kind, default, label, help).order(order));
    };
    add(
        "bench.worktree.location",
        SettingKind::Choice {
            options: ["data_dir", "beside_project", "custom"]
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        },
        SettingValue::Text("data_dir".to_owned()),
        10,
    );
    add(
        "bench.worktree.path",
        SettingKind::Path,
        SettingValue::Text("%LOCALAPPDATA%\\Forja\\bancadas".to_owned()),
        20,
    );
    add(
        "bench.worktree.name_template",
        SettingKind::Text { max_len: Some(120) },
        SettingValue::Text("{pedido}-{slug}".to_owned()),
        30,
    );
    add(
        "bench.worktree.branch_prefix",
        SettingKind::Text { max_len: Some(80) },
        SettingValue::Text("forja/".to_owned()),
        40,
    );
    add(
        "bench.worktree.base_branch",
        SettingKind::Choice {
            options: ["auto", "current", "main", "master"]
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        },
        SettingValue::Text("auto".to_owned()),
        50,
    );
    add(
        "bench.worktree.fetch_before",
        SettingKind::Bool,
        SettingValue::Bool(true),
        60,
    );
    add(
        "bench.worktree.auto_prune_days",
        SettingKind::Integer {
            min: Some(0),
            max: Some(90),
        },
        SettingValue::Integer(0),
        70,
    );
    add(
        "bench.layout.tabs",
        SettingKind::List {
            item: "id de aba".to_owned(),
        },
        SettingValue::List(vec!["terminal".to_owned(), "helper".to_owned()]),
        80,
    );
    add(
        "bench.layout.tool_tabs",
        SettingKind::List {
            item: "id de aba".to_owned(),
        },
        SettingValue::List(vec![
            "terminal".to_owned(),
            "chat".to_owned(),
            "diff".to_owned(),
            "editor".to_owned(),
        ]),
        85,
    );
    add(
        "bench.layout.restore",
        SettingKind::Bool,
        SettingValue::Bool(true),
        90,
    );
    add(
        "bench.setup.policy",
        SettingKind::Choice {
            options: ["ask", "always", "never"]
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        },
        SettingValue::Text("ask".to_owned()),
        100,
    );
    add(
        "bench.setup.timeout_s",
        SettingKind::Integer {
            min: Some(30),
            max: Some(1800),
        },
        SettingValue::Integer(300),
        110,
    );
    add(
        "bench.teardown.policy",
        SettingKind::Choice {
            options: ["ask", "always", "never"]
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        },
        SettingValue::Text("ask".to_owned()),
        120,
    );
    add(
        "bench.teardown.branch",
        SettingKind::Choice {
            options: ["ask", "keep", "delete"]
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        },
        SettingValue::Text("ask".to_owned()),
        130,
    );
    add(
        "bench.close.confirm_dirty",
        SettingKind::Bool,
        SettingValue::Bool(true),
        140,
    );
    add(
        "bench.close.stop_helpers",
        SettingKind::Bool,
        SettingValue::Bool(true),
        150,
    );
    add(
        "bench.stage.creation_default",
        SettingKind::Choice {
            options: ["draft", "forging", "review", "ready"]
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        },
        SettingValue::Text("draft".to_owned()),
        160,
    );
    add(
        "bench.stage.wip_limit",
        SettingKind::Integer {
            min: Some(0),
            max: Some(50),
        },
        SettingValue::Integer(0),
        170,
    );
    add(
        "bench.stage.sync_request",
        SettingKind::Bool,
        SettingValue::Bool(true),
        180,
    );
    add(
        "bench.stage.pr_open",
        SettingKind::Choice {
            options: ["always", "ask", "never"]
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        },
        SettingValue::Text("always".to_owned()),
        190,
    );
    add(
        "bench.stage.pr_merge",
        SettingKind::Choice {
            options: ["always", "ask", "never"]
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        },
        SettingValue::Text("always".to_owned()),
        200,
    );
    add(
        "bench.stage.archive_on_ready",
        SettingKind::Bool,
        SettingValue::Bool(false),
        210,
    );
}
