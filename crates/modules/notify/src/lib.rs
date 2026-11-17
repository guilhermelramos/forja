//! Módulo `notify`: toasts, central do Windows e o centro na barra de status.
//!
//! Atende o comando `notify.show` e o evento `notify.requested`.

mod os;
mod rules;
mod sound;
mod view;

use forja_contracts::names::NOTIFY_REQUESTED;
use forja_contracts::{NotifyLevel, NotifyRequested};
use forja_kernel::forja_protocol::{Method, MethodEffect, ParamKind, ParamSchema, ResultSchema};
use forja_kernel::ui_contract::View;
use forja_kernel::{
    Args, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot,
};
use rules::{decide, local_hm, parse_hhmm, Policy};
use serde_json::{json, Value};
use std::time::Instant;
use view::{notices_json, CenterView, Notice, Shared, ToastView};

#[forja_kernel::module]
static NOTIFY: fn() -> Box<dyn Module> = || {
    Box::new(NotifyModule {
        state: Shared::new(),
    })
};

pub struct NotifyModule {
    state: Shared,
}

impl Module for NotifyModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("notify", "notify.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);

        let command_state = self.state.clone();
        r.command("notify.show", "notify.command.show", move |cx, args| {
            show_args(cx, &command_state, &args)
        });

        let recent_state = self.state.clone();
        r.command(
            "notify.recent",
            "notify.command.recent",
            move |_cx, _args| Ok(notices_json(&recent_state)),
        );

        let rpc_state = self.state.clone();
        r.rpc(
            Method::new("notify.show", "notify.method.show.description")
                .effect(MethodEffect::Write)
                .param(ParamSchema::required("title", ParamKind::String))
                .param(ParamSchema::optional("body", ParamKind::String))
                .param(ParamSchema::optional("level", ParamKind::String))
                .param(ParamSchema::optional("bench_path", ParamKind::String))
                .result(
                    ResultSchema::new("notify.method.show.result").schema(json!({
                        "type": "object",
                        "properties": {
                            "toast": { "type": "boolean" },
                            "os": { "type": "boolean" },
                            "sound": { "type": "boolean" }
                        },
                        "required": ["toast", "os", "sound"]
                    })),
                ),
            move |cx, params| {
                let args = Args::from_map(params);
                show_args(cx, &rpc_state, &args)
                    .map_err(|error| forja_kernel::forja_protocol::RpcError::app(error.to_string()))
            },
        );

        r.contract_event::<NotifyRequested>(NOTIFY_REQUESTED);
        let event_state = self.state.clone();
        r.subscribe(move |cx, event: &NotifyRequested| {
            let _ = deliver(
                cx,
                &event_state,
                event.title.clone(),
                event.body.clone(),
                event.level,
                event.bench_path.clone(),
            );
        });

        for key in [
            "notify.command.fallback-title",
            "notify.center.count",
            "notify.center.empty",
            "notify.toast.dismiss",
        ] {
            r.uses_i18n(key);
        }

        let toasts = self.state.clone();
        r.view(
            Slot::Modal,
            "notify.toasts",
            "notify.toasts.title",
            10,
            move || {
                Box::new(ToastView {
                    state: toasts.clone(),
                }) as Box<dyn View>
            },
        );
        let center = self.state.clone();
        r.view(
            Slot::StatusBar,
            "notify.center",
            "notify.center.title",
            50,
            move || {
                Box::new(CenterView {
                    state: center.clone(),
                }) as Box<dyn View>
            },
        );
    }
}

fn register_settings(r: &mut Registrar<'_>) {
    r.setting(toggle(
        "notifications.toast.enabled",
        true,
        "setting-notifications-toast-enabled-label",
        "setting-notifications-toast-enabled-help",
        10,
    ));
    r.setting(choice(
        "notifications.toast.position",
        &["bottom_right", "top_right", "bottom_left", "top_left"],
        "bottom_right",
        "setting-notifications-toast-position-label",
        "setting-notifications-toast-position-help",
        20,
    ));
    r.setting(number(
        "notifications.toast.duration_s",
        5.0,
        2.0,
        15.0,
        "setting-notifications-toast-duration_s-label",
        "setting-notifications-toast-duration_s-help",
        30,
    ));
    r.setting(number(
        "notifications.toast.max",
        3.0,
        1.0,
        5.0,
        "setting-notifications-toast-max-label",
        "setting-notifications-toast-max-help",
        40,
    ));
    r.setting(toggle(
        "notifications.os.enabled",
        true,
        "setting-notifications-os-enabled-label",
        "setting-notifications-os-enabled-help",
        50,
    ));
    r.setting(toggle(
        "notifications.os.only_unfocused",
        true,
        "setting-notifications-os-only_unfocused-label",
        "setting-notifications-os-only_unfocused-help",
        60,
    ));
    r.setting(toggle(
        "notifications.os.sound",
        false,
        "setting-notifications-os-sound-label",
        "setting-notifications-os-sound-help",
        70,
    ));
    r.setting(toggle(
        "notifications.quiet.enabled",
        false,
        "setting-notifications-quiet-enabled-label",
        "setting-notifications-quiet-enabled-help",
        80,
    ));
    r.setting(text_setting(
        "notifications.quiet.start",
        "22:00",
        "setting-notifications-quiet-start-label",
        "setting-notifications-quiet-start-help",
        90,
    ));
    r.setting(text_setting(
        "notifications.quiet.end",
        "07:00",
        "setting-notifications-quiet-end-label",
        "setting-notifications-quiet-end-help",
        100,
    ));
}

fn toggle(key: &str, default: bool, label: &str, help: &str, order: i32) -> SettingSpec {
    SettingSpec::new(
        key,
        SettingKind::Bool,
        SettingValue::Bool(default),
        label,
        help,
    )
    .order(order)
}

fn text_setting(key: &str, default: &str, label: &str, help: &str, order: i32) -> SettingSpec {
    SettingSpec::new(
        key,
        SettingKind::Text { max_len: Some(8) },
        SettingValue::Text(default.to_owned()),
        label,
        help,
    )
    .order(order)
}

fn number(
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

fn show_args(
    cx: &mut forja_kernel::Ctx<'_>,
    state: &Shared,
    args: &Args,
) -> forja_kernel::Result<Value> {
    let title = args
        .str("title")
        .map(str::to_owned)
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| cx.t("notify.command.fallback-title"));
    let body = args.str("body").unwrap_or("").to_owned();
    let level = parse_level(args.str("level"));
    let bench_path = args
        .str("bench_path")
        .map(str::to_owned)
        .filter(|path| !path.is_empty());
    deliver(cx, state, title, body, level, bench_path)
}

fn deliver(
    cx: &mut forja_kernel::Ctx<'_>,
    state: &Shared,
    title: String,
    body: String,
    level: NotifyLevel,
    bench_path: Option<String>,
) -> forja_kernel::Result<Value> {
    let decision = decide(&policy_from(cx, state.focused()));
    if decision.show_os {
        let title = title.clone();
        let body = body.clone();
        cx.spawn(
            move || {
                if let Err(error) = os::show(&title, &body) {
                    eprintln!("forja notify: {error}");
                }
            },
            |_, _| {},
        );
    }
    if decision.play_sound {
        cx.spawn(sound::play_beep, |_, _| {});
    }
    if decision.show_toast || decision.show_os {
        state.push(Notice {
            id: 0,
            title,
            body,
            level,
            bench_path,
            at: Instant::now(),
            read: false,
        });
    }
    Ok(json!({
        "toast": decision.show_toast,
        "os": decision.show_os,
        "sound": decision.play_sound,
    }))
}

fn policy_from(cx: &forja_kernel::Ctx<'_>, focused: bool) -> Policy {
    Policy {
        toast_enabled: flag(cx, "notifications.toast.enabled", true),
        os_enabled: flag(cx, "notifications.os.enabled", true),
        only_unfocused: flag(cx, "notifications.os.only_unfocused", true),
        sound: flag(cx, "notifications.os.sound", false),
        quiet: flag(cx, "notifications.quiet.enabled", false),
        quiet_start: parse_hhmm(text_setting_value(cx, "notifications.quiet.start", "22:00"))
            .unwrap_or((22, 0)),
        quiet_end: parse_hhmm(text_setting_value(cx, "notifications.quiet.end", "07:00"))
            .unwrap_or((7, 0)),
        focused,
        now: local_hm(),
    }
}

fn flag(cx: &forja_kernel::Ctx<'_>, key: &str, default: bool) -> bool {
    cx.setting(key)
        .and_then(SettingValue::as_bool)
        .unwrap_or(default)
}

fn text_setting_value<'a>(cx: &'a forja_kernel::Ctx<'_>, key: &str, default: &'a str) -> &'a str {
    cx.setting(key)
        .and_then(SettingValue::as_str)
        .unwrap_or(default)
}

fn parse_level(text: Option<&str>) -> NotifyLevel {
    match text.unwrap_or("info") {
        "success" | "ok" => NotifyLevel::Success,
        "attention" | "warning" | "warn" => NotifyLevel::Attention,
        "error" => NotifyLevel::Error,
        _ => NotifyLevel::Info,
    }
}

/// Âncora só para os testes: o prefixo `notifications` precisa ser um módulo.
pub struct NotificationsAnchor;

impl Module for NotificationsAnchor {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("notifications", "notifications.name", "0.1.0")
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            "notifications-name = Notifications\n",
            "notifications-name = Notificações\n",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use serde_json::Map;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "forja-notify-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn runtime(dir: &std::path::Path) -> Runtime {
        Runtime::new(
            vec![
                Box::new(NotificationsAnchor),
                Box::new(NotifyModule {
                    state: Shared::new(),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("notify registers")
    }

    #[test]
    fn notification_icon_action_has_a_translated_accessible_name() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let ctx = forja_ui_kit::egui::Context::default();
        ctx.enable_accesskit();
        let mut view = CenterView {
            state: Shared::new(),
        };
        let output = ctx.run(Default::default(), |ctx| {
            forja_ui_kit::egui::CentralPanel::default().show(ctx, |ui| {
                view.ui(ui, &mut runtime.ctx("notify"));
            });
        });
        let update = output.platform_output.accesskit_update.unwrap();
        assert!(update
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("0 avisos")));
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn contract_show_and_event() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let value = runtime
            .run_command(
                "notify.show",
                Args::from_map(Map::from_iter([
                    ("title".to_owned(), json!("Olá")),
                    ("body".to_owned(), json!("da forja")),
                    ("level".to_owned(), json!("success")),
                ])),
            )
            .unwrap();
        assert_eq!(value["toast"], true);
        assert_eq!(value["os"], false);
        assert_eq!(value["sound"], false);
        let recent = runtime.run_command("notify.recent", Args::new()).unwrap();
        assert_eq!(recent["items"][0]["title"], "Olá");

        let emitted = runtime.emit(&NotifyRequested {
            title: "Evento".into(),
            body: "corpo".into(),
            level: NotifyLevel::Info,
            bench_path: None,
        });
        assert!(emitted);
        let recent = runtime.run_command("notify.recent", Args::new()).unwrap();
        assert_eq!(recent["items"][0]["title"], "Evento");
        let _ = std::fs::remove_dir_all(dir);
    }
}
