use forja_kernel::{Registrar, SettingKind, SettingSpec, SettingValue};
pub(crate) fn settings(r: &mut Registrar<'_>) {
    let mut add = |key: &str, kind: SettingKind, default: SettingValue, order: i32| {
        r.setting(
            SettingSpec::new(
                key,
                kind,
                default,
                format!("setting-{}-label", key.replace('.', "-")),
                format!("setting-{}-help", key.replace('.', "-")),
            )
            .order(order),
        );
    };
    add(
        "git.identity.name",
        SettingKind::Text { max_len: Some(120) },
        SettingValue::Text(String::new()),
        10,
    );
    add(
        "git.identity.email",
        SettingKind::Text { max_len: Some(200) },
        SettingValue::Text(String::new()),
        20,
    );
    add(
        "git.commit.sign",
        SettingKind::Bool,
        SettingValue::Bool(false),
        30,
    );
    add(
        "git.commit.gpg_key",
        SettingKind::Text { max_len: Some(120) },
        SettingValue::Text(String::new()),
        40,
    );
    add(
        "git.commit.sign_off",
        SettingKind::Bool,
        SettingValue::Bool(false),
        50,
    );
    add(
        "git.sync.fetch_interval_min",
        SettingKind::Integer {
            min: Some(0),
            max: Some(60),
        },
        SettingValue::Integer(10),
        60,
    );
    add(
        "git.sync.auto_pull",
        SettingKind::Choice {
            options: ["never", "clean_only", "always"]
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        },
        SettingValue::Text("never".to_owned()),
        70,
    );
    add(
        "git.push.auto_set_upstream",
        SettingKind::Bool,
        SettingValue::Bool(true),
        80,
    );
    add(
        "git.protected.branches",
        SettingKind::List {
            item: "nome de branch".to_owned(),
        },
        SettingValue::List(vec!["main".to_owned(), "master".to_owned()]),
        90,
    );
    add(
        "git.status.interval_s",
        SettingKind::Integer {
            min: Some(1),
            max: Some(60),
        },
        SettingValue::Integer(5),
        100,
    );
}
