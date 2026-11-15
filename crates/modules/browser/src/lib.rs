//! Módulo `browser` da Forja: navegador WebView2 embutido, histórico, zoom,
//! downloads confinados e Mermaid offline.

mod auth;
mod download;
mod history;
mod home;
mod home_data;
mod mermaid;
mod profile;
mod state;
mod url;
mod view;
mod view_support;
mod visibility;
mod webview;

use crate::state::{Pending, Shared};
use forja_kernel::{
    Args, Ctx, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot,
};
use serde_json::{json, Value};

#[forja_kernel::module]
static BROWSER: fn() -> Box<dyn Module> = || Box::new(BrowserModule::with_history());

pub struct BrowserModule {
    state: Shared,
}

impl BrowserModule {
    pub fn new() -> Self {
        Self {
            state: Shared::new(history::History::default()),
        }
    }

    fn with_history() -> Self {
        let path = data_path();
        Self {
            state: Shared::new(history::History::load(&path)),
        }
    }
}

impl Default for BrowserModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for BrowserModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("browser", "browser.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);

        let open_state = self.state.clone();
        r.command("browser.open", "browser.command.open", move |cx, args| {
            open_command(cx, &open_state, &args, true)
        });

        let navigate_state = self.state.clone();
        r.command(
            "browser.navigate",
            "browser.command.navigate",
            move |cx, args| open_command(cx, &navigate_state, &args, false),
        );

        let mermaid_state = self.state.clone();
        r.command(
            "browser.mermaid.render",
            "browser.command.mermaid",
            move |_cx, args| {
                let source = args.required_str("source")?.to_owned();
                let blocks = mermaid::extract_blocks(&source);
                let html = if blocks.is_empty() {
                    mermaid::preview_html(&[source])
                } else {
                    mermaid::preview_html(&blocks)
                };
                mermaid_state.open("forja://mermaid", Some(html), true);
                Ok(json!({
                    "opened": true,
                    "version": mermaid::VERSION.trim(),
                    "license": mermaid::license()
                }))
            },
        );

        let external_state = self.state.clone();
        r.command(
            "browser.open_external",
            "browser.command.open_external",
            move |cx, args| {
                let url = match args.str("url").filter(|u| !u.trim().is_empty()) {
                    Some(url) => url.to_owned(),
                    None => match external_state.active().map(|tab| tab.url).filter(|u| !u.is_empty()) {
                        Some(url) => url,
                        None => return Ok(json!({ "pending": false, "reason": "no_active_tab" })),
                    },
                };
                external_state.set_error(None);
                let error_state = external_state.clone();
                cx.spawn(
                    move || open_external(&url),
                    move |_cx, result| {
                        if let Err(error) = result {
                            error_state.set_error(Some(error));
                        }
                    },
                );
                Ok(json!({ "pending": true }))
            },
        );

        r.command(
            "browser.devtools",
            "browser.command.devtools",
            move |cx, _args| {
                let current = cx
                    .setting("browser.devtools.enabled")
                    .and_then(SettingValue::as_bool)
                    .unwrap_or(true);
                let next = !current;
                cx.set_setting("browser.devtools.enabled", SettingValue::Bool(next))
                    .map_err(|error| forja_kernel::KernelError::message(error.to_string()))?;
                Ok(json!({ "enabled": next }))
            },
        );

        let history_state = self.state.clone();
        r.command(
            "browser.history",
            "browser.command.history",
            move |_cx, _args| {
                history_state.toggle_history();
                Ok(json!({ "open": history_state.history_open() }))
            },
        );

        let clear_state = self.state.clone();
        r.command(
            "browser.history.clear",
            "browser.command.clear_history",
            move |cx, _args| {
                clear_state.clear_history();
                save_history(cx, &clear_state);
                Ok(json!({ "cleared": true }))
            },
        );

        register_actions(r, &self.state);
        register_rpc(r, &self.state);

        for key in used_keys() {
            r.uses_i18n(key);
        }

        let view_state = self.state.clone();
        r.view(
            Slot::BenchTab,
            "browser.tab",
            "browser.tab.title",
            bench_tab_order(25),
            move || Box::new(view::BrowserView::new(view_state.clone())),
        );
    }
}

fn register_actions(r: &mut Registrar<'_>, shared: &Shared) {
    let state = shared.clone();
    r.command(
        "browser.address",
        "browser.command.address",
        move |_cx, _args| {
            state.set_address_focus(true);
            Ok(json!({ "focused": true }))
        },
    );
    let state = shared.clone();
    r.command(
        "browser.reload",
        "browser.command.reload",
        move |_cx, _args| {
            state.request(Pending::Reload);
            Ok(json!({}))
        },
    );
    let state = shared.clone();
    r.command(
        "browser.reload_hard",
        "browser.command.reload_hard",
        move |_cx, _args| {
            state.request(Pending::HardReload);
            Ok(json!({}))
        },
    );
    let state = shared.clone();
    r.command("browser.find", "browser.command.find", move |_cx, _args| {
        state.set_find(true, None);
        Ok(json!({ "open": true }))
    });
    let state = shared.clone();
    r.command(
        "browser.new_page",
        "browser.command.new_page",
        move |cx, _args| {
            let home = home_url(cx);
            state.open(&home, None, true);
            Ok(json!({}))
        },
    );
    let state = shared.clone();
    r.command(
        "browser.reopen_page",
        "browser.command.reopen_page",
        move |_cx, _args| {
            state.reopen();
            Ok(json!({}))
        },
    );
    let state = shared.clone();
    r.command(
        "browser.close_page",
        "browser.command.close_page",
        move |_cx, _args| {
            let index = state.with(|state| state.active).unwrap_or(0);
            state.close(index);
            Ok(json!({}))
        },
    );
    for (id, key, delta) in [
        ("browser.zoom.in", "browser.command.zoom_in", 1),
        ("browser.zoom.out", "browser.command.zoom_out", -1),
        ("browser.zoom.reset", "browser.command.zoom_reset", 0),
    ] {
        let state = shared.clone();
        r.command(id, key, move |_cx, _args| {
            state.with(|state| {
                if let Some(tab) = state.tabs.get_mut(state.active) {
                    tab.zoom = if delta == 0 {
                        0
                    } else {
                        (tab.zoom + delta).clamp(-6, 9)
                    };
                }
            });
            state.request(Pending::Zoom);
            Ok(json!({}))
        });
    }
    let state = shared.clone();
    r.command("browser.back", "browser.command.back", move |_cx, _args| {
        state.request(Pending::Back);
        Ok(json!({}))
    });
    let state = shared.clone();
    r.command(
        "browser.forward",
        "browser.command.forward",
        move |_cx, _args| {
            state.request(Pending::Forward);
            Ok(json!({}))
        },
    );
}

fn register_rpc(r: &mut Registrar<'_>, shared: &Shared) {
    use forja_kernel::forja_protocol::{
        Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
    };
    let state = shared.clone();
    r.rpc(
        Method::new("browser.open", "browser.method.open.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("url", ParamKind::String))
            .result(ResultSchema::new("browser.method.open.result")),
        move |_cx, params| {
            let url = params
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_params("missing `url`"))?;
            state.open(url, None, true);
            Ok(json!({}))
        },
    );
    let state = shared.clone();
    r.rpc(
        Method::new("browser.history", "browser.method.history.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .result(ResultSchema::new("browser.method.history.result")),
        move |_cx, _params| {
            let history = state.history();
            let count = history.len();
            Ok(json!({ "entries": history.entries, "count": count }))
        },
    );
}

fn open_command(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    args: &Args,
    new_tab: bool,
) -> forja_kernel::Result<Value> {
    let raw = args.required_str("url")?.to_owned();
    let engine = cx
        .setting("browser.search.engine")
        .and_then(SettingValue::as_str)
        .unwrap_or("duckduckgo");
    let url = url::normalize(&raw, engine).map(|target| target.url().to_owned());
    let Some(url) = url else {
        return Ok(json!({ "opened": false }));
    };
    let external = cx
        .setting("browser.engine.kind")
        .and_then(SettingValue::as_str)
        .unwrap_or("embedded")
        == "system";
    if external {
        let target = url.clone();
        cx.spawn(move || open_external(&target), |_cx, _| {});
        return Ok(json!({ "opened": false, "external": true }));
    }
    shared.open(&url, None, new_tab);
    Ok(json!({ "opened": true, "url": url }))
}

fn register_settings(r: &mut Registrar<'_>) {
    let settings = [
        (
            "browser.engine.kind",
            SettingKind::Choice {
                options: vec!["embedded".into(), "system".into()],
            },
            SettingValue::Text("embedded".into()),
        ),
        (
            "browser.home.url",
            SettingKind::Text {
                max_len: Some(2048),
            },
            SettingValue::Text("forja://inicio".into()),
        ),
        (
            "browser.search.engine",
            SettingKind::Choice {
                options: vec!["duckduckgo".into(), "bing".into(), "google".into()],
            },
            SettingValue::Text("duckduckgo".into()),
        ),
        (
            "browser.privacy.do_not_track",
            SettingKind::Bool,
            SettingValue::Bool(true),
        ),
        (
            "browser.devtools.enabled",
            SettingKind::Bool,
            SettingValue::Bool(false),
        ),
        (
            "browser.download.dir",
            SettingKind::Path,
            SettingValue::Text("%USERPROFILE%\\Downloads".into()),
        ),
        (
            "browser.download.ask_each",
            SettingKind::Bool,
            SettingValue::Bool(true),
        ),
        (
            "browser.zoom.default",
            SettingKind::Number {
                min: Some(0.5),
                max: Some(2.0),
            },
            SettingValue::Number(1.0),
        ),
        (
            "browser.tabs.restore",
            SettingKind::Bool,
            SettingValue::Bool(true),
        ),
        (
            "browser.auth.open_external",
            SettingKind::Bool,
            SettingValue::Bool(false),
        ),
    ];
    for (key, kind, default) in settings {
        let slug = key.replace('.', "-");
        r.setting(SettingSpec::new(
            key,
            kind,
            default,
            format!("setting-{slug}-label"),
            format!("setting-{slug}-help"),
        ));
    }
}

fn home_url(cx: &Ctx<'_>) -> String {
    cx.setting("browser.home.url")
        .and_then(SettingValue::as_str)
        .unwrap_or("forja://inicio")
        .to_owned()
}

fn data_path() -> std::path::PathBuf {
    forja_kernel::paths::default_data_dir().join("browser-history.json")
}

fn save_history(cx: &mut Ctx<'_>, shared: &Shared) {
    let history = shared.history();
    let path = data_path();
    cx.spawn(move || history.save(&path), |_cx, _| {});
}

pub(crate) fn open_external(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

fn bench_tab_order(default: i32) -> i32 {
    match std::env::var("FORJA_BENCH_TAB") {
        Ok(module) if module == "browser" => 1,
        _ => default,
    }
}

fn used_keys() -> Vec<&'static str> {
    vec![
        "browser.name",
        "browser.tab.title",
        "browser.command.open",
        "browser.command.navigate",
        "browser.command.mermaid",
        "browser.command.open_external",
        "browser.command.devtools",
        "browser.command.history",
        "browser.command.clear_history",
        "browser.command.address",
        "browser.command.reload",
        "browser.command.reload_hard",
        "browser.command.find",
        "browser.command.new_page",
        "browser.command.reopen_page",
        "browser.command.close_page",
        "browser.command.zoom_in",
        "browser.command.zoom_out",
        "browser.command.zoom_reset",
        "browser.command.back",
        "browser.command.forward",
        "browser.view.empty",
        "browser.view.address",
        "browser.view.go",
        "browser.view.back",
        "browser.view.forward",
        "browser.view.reload",
        "browser.view.new-tab",
        "browser.view.close-tab",
        "browser.view.history",
        "browser.view.downloads",
        "browser.window.close",
        "browser.view.downloaded",
        "browser.view.error",
        "browser.view.loading",
        "browser.view.find",
        "browser.view.find-hint",
        "browser.view.zoom",
        "browser.view.home-title",
        "browser.view.home-body",
        "browser.view.hidden",
        "browser.view.unfocused",
        "browser.view.no-window",
        "browser.history.title",
        "browser.history.empty",
        "browser.history.clear",
        "browser.home.servers-title",
        "browser.home.servers-refresh",
        "browser.home.servers-filter-hint",
        "browser.home.servers-active-only",
        "browser.home.servers-empty",
        "browser.home.servers-empty-bench",
        "browser.home.servers-forja",
        "browser.home.docs-title",
        "browser.home.docs-hint",
        "browser.home.docs-no-project",
        "browser.home.docs-empty",
        "browser.home.docs-loading",
        "browser.home.docs-open-failed",
        "browser.home.recent-title",
        "browser.home.recent-empty",
        "browser.home.offline-note",
        "browser.method.open.description",
        "browser.method.open.result",
        "browser.method.history.description",
        "browser.method.history.result",
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};

    struct TempGuard(std::path::PathBuf);

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_dir() -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-browser-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir)
    }

    fn runtime(dir: &std::path::Path) -> Runtime {
        Runtime::new(
            vec![Box::new(BrowserModule::new())],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("browser registers cleanly")
    }

    #[test]
    fn contract_holds_and_settings_match_the_catalog() {
        let dir = temp_dir();
        let runtime = runtime(&dir.0);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let keys: Vec<&str> = runtime
            .registry()
            .settings()
            .iter()
            .filter(|spec| spec.key.starts_with("browser."))
            .map(|spec| spec.key.as_str())
            .collect();
        assert_eq!(keys.len(), 10);
        assert!(keys.contains(&"browser.engine.kind"));
        assert!(keys.contains(&"browser.download.dir"));
    }

    #[test]
    fn open_and_mermaid_commands_create_tabs() {
        let dir = temp_dir();
        let module = BrowserModule::new();
        let shared = module.state.clone();
        let mut runtime = Runtime::new(
            vec![Box::new(module)],
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .unwrap();
        let mut args = Args::new();
        args.insert("url", json!("https://example.test"));
        runtime.run_command("browser.open", args).unwrap();
        assert_eq!(shared.tabs().len(), 1);

        let mut args = Args::new();
        args.insert("source", json!("graph TD\nA-->B"));
        runtime.run_command("browser.mermaid.render", args).unwrap();
        let tabs = shared.tabs();
        assert_eq!(tabs.len(), 2);
        assert!(tabs[1]
            .html
            .as_deref()
            .unwrap_or_default()
            .contains("graph TD"));
    }

    #[test]
    fn tab_cap_keeps_four_and_remembers_closed_pages() {
        let shared = Shared::new(history::History::default());
        for index in 0..6 {
            shared.open(&format!("https://example.test/{index}"), None, true);
        }
        assert_eq!(shared.tabs().len(), 4);
        shared.with(|state| assert_eq!(state.closed.len(), 2));
    }

    #[test]
    fn devtools_and_open_external_commands_work() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir.0);
        let toggled = runtime.run_command("browser.devtools", Args::new()).unwrap();
        assert_eq!(toggled["enabled"], true);
        let toggled_back = runtime.run_command("browser.devtools", Args::new()).unwrap();
        assert_eq!(toggled_back["enabled"], false);

        let no_tab = runtime.run_command("browser.open_external", Args::new()).unwrap();
        assert_eq!(no_tab["pending"], false);
        assert_eq!(no_tab["reason"], "no_active_tab");
    }
}
