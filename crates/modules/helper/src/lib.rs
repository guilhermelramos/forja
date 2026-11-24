//! Módulo `helper` da Forja: perfis, detecção, início e status dos ajudantes.
//!
//! O módulo não importa o crate do terminal: fala com ele por `terminal.open`,
//! `terminal.send`, `terminal.read` e pelos eventos de contrato. Sem o módulo
//! terminal, o launch falha com uma mensagem clara e o resto continua de pé.

mod commands;
mod context;
mod launch_context;
mod launch_flow;
mod persistence;
mod ready;
mod resume;
pub mod demo;
mod detect;
mod hooks;
mod launch;
mod picker;
mod profile;
mod profile_cards;
mod profile_data;
mod rpc;
mod status;
mod status_view;
mod store;
mod views;

#[cfg(test)]
mod conpty_test;

use forja_contracts::names;
use forja_contracts::{
    HelperStatus, HelperStatusChanged, TerminalBell, TerminalExited, TerminalTitleChanged,
};
use forja_kernel::{
    Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use views::{HelperPage, HelperSettings};
use windows_sys::Win32::System::Power::{
    SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED,
};

/// Ponteiro compartilhado para o estado do módulo.
pub(crate) type Shared = Arc<Core>;

/// Estado global do módulo.
pub struct Core {
    /// Estado protegido por mutex.
    pub state: Mutex<HelperState>,
}

impl Core {
    fn new() -> Self {
        Self {
            state: Mutex::new(HelperState::default()),
        }
    }
}

/// Estado em memória.
#[derive(Default)]
pub struct HelperState {
    /// Perfis carregados.
    pub profiles: Vec<profile::Profile>,
    /// Detecção por id.
    pub detection: BTreeMap<String, detect::Detection>,
    /// Probe rápido de executável em cache antes ou na ausência da detecção completa.
    pub probed: BTreeMap<String, Option<std::path::PathBuf>>,
    /// Runs ativos por id de terminal.
    pub runs: BTreeMap<String, status::Run>,
    /// Última bancada usada por perfil.
    pub last_benches: BTreeMap<String, String>,
    /// Preferências e consentimentos persistidos.
    pub file: store::HelperFile,
    /// Bancada da última carga.
    pub loaded_for: Option<std::path::PathBuf>,
    /// Detecção em andamento.
    pub detecting: bool,
    /// Quantas vezes a detecção terminou nesta sessão.
    pub detect_runs: u64,
    /// O computador está sendo mantido acordado.
    pub hook_results: BTreeMap<String, Result<hooks::HookStatus, String>>,
    pub hook_busy: bool,
    pub awake: bool,
    /// A prévia de captura já foi semeada.
    pub preview_seeded: bool,
    /// Contexto de launch, prontidão e retomada.
    pub launch: launch_context::State,
    /// Erro cru do último lançamento que falhou, exibido humanizado na página.
    pub last_launch_error: Option<String>,
}

impl HelperState {
    /// Ajustes de um perfil.
    pub fn overrides(&self, id: &str) -> launch::ProfileOverride {
        self.file.profiles.get(id).cloned().unwrap_or_default()
    }

    /// Caminho resolvido do executável para o perfil pela fonte única de verdade.
    pub fn executable_for(&self, id: &str) -> Option<&std::path::Path> {
        if let Some(detection) = self.detection.get(id) {
            return detection.executable.as_deref();
        }
        self.probed.get(id).and_then(|path| path.as_deref())
    }

    /// Indica se o perfil está instalado segundo a fonte única de verdade do módulo.
    pub fn is_installed(&self, id: &str) -> bool {
        self.executable_for(id).is_some()
    }

    /// Indica se já existe um veredito de detecção ou probe rápido para os perfis carregados.
    pub fn has_detection_verdict(&self) -> bool {
        self.detect_runs > 0
            || !self.detection.is_empty()
            || (!self.profiles.is_empty() && self.probed.len() >= self.profiles.len())
    }

    /// Atualiza o estado de energia conforme existem runs ativos.
    pub fn refresh_awake(&mut self) {
        let active = self
            .runs
            .values()
            .any(|run| !matches!(run.status, HelperStatus::Finished | HelperStatus::Error));
        if active != self.awake {
            self.awake = active;
            set_awake(active);
        }
    }
}

impl Drop for HelperState {
    fn drop(&mut self) {
        if self.awake {
            set_awake(false);
        }
    }
}

fn set_awake(enabled: bool) {
    #[cfg(windows)]
    unsafe {
        SetThreadExecutionState(if enabled {
            ES_CONTINUOUS | ES_SYSTEM_REQUIRED
        } else {
            ES_CONTINUOUS
        });
    }
}

#[forja_kernel::module]
static HELPER: fn() -> Box<dyn Module> = || {
    Box::new(HelperModule {
        core: Arc::new(Core::new()),
    })
};

/// Módulo helper.
pub struct HelperModule {
    core: Shared,
}

impl Module for HelperModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("helper", "helper.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);
        commands::register(r, &self.core);
        launch_context::register(r, &self.core);
        picker::register(r);
        r.view(
            forja_kernel::Slot::Modal,
            picker::PICKER_VIEW,
            "helper.profile-picker.title",
            10,
            {
                let core = std::sync::Arc::clone(&self.core);
                move || Box::new(picker::ProfilePicker::new(std::sync::Arc::clone(&core)))
            },
        );
        rpc::register(r, &self.core);
        register_events(r, &self.core);

        for key in [
            "helper.view.title",
            "helper.view.empty",
            "helper.view.active",
            "helper.view.profiles",
            "helper.view.launch",
            "helper.view.select",
            "helper.view.working",
            "helper.view.needs-you",
            "helper.view.finished",
            "helper.view.error",
            "helper.view.failed",
            "helper.view.idle",
            "helper.view.unknown",
            "helper.view.installed",
            "helper.view.not-installed",
            "helper.view.detect",
            "helper.view.detecting",
            "helper.view.detect-summary",
            "helper.view.installed-section",
            "helper.view.installed-empty",
            "helper.view.not-installed-section",
            "helper.view.show-not-installed",
            "helper.view.hide-not-installed",
            "helper.view.search-hint",
            "helper.view.clear-filter",
            "helper.view.filter-empty",
            "helper.view.how-to-install",
            "helper.view.copy-command",
            "helper.view.copied",
            "helper.view.open-docs",
            "helper.view.last-bench",
            "helper.view.chat-action",
            "helper.view.default-badge",
            "helper.view.set-default",
            "helper.view.focus",
            "helper.view.bench",
            "helper.view.duration",
            "helper.launch.not-installed",
            "helper.settings.title",
            "helper.settings.profiles",
            "helper.settings.enabled",
            "helper.settings.command",
            "helper.settings.extra-args",
            "helper.settings.extra-args-help",
            "helper.settings.environment",
            "helper.settings.environment-help",
            "helper.settings.hooks",
            "helper.settings.hook-target",
            "helper.settings.consent",
            "helper.settings.install",
            "helper.settings.remove",
            "helper.settings.save",
            "helper.settings.status.installed",
            "helper.settings.status.partial",
            "helper.settings.status.not-installed",
            "helper.settings.status.unsupported",
            "helper.settings.status.error",
            "helper.settings.status.loading",
            "helper.settings.preview-title",
            "helper.settings.preview-body",
            "helper.notice.terminal-missing",
            "helper.notify.waiting-title",
            "helper.notify.waiting-body",
            "helper.notify.done-title",
            "helper.notify.done-body",
            "helper.profile-picker.empty",
            "helper.profile-picker.cancel",
            "helper.notify.error-title",
            "helper.notify.error-body",
        ] {
            r.uses_i18n(key);
        }

        let status_core = Arc::clone(&self.core);
        r.view(
            Slot::StatusBar,
            "helper.status-bar",
            "helper.view.title",
            10,
            move || Box::new(status_view::HelperStatusView(status_core.clone())),
        );
        let page_core = Arc::clone(&self.core);
        let page_order = if std::env::var_os("FORJA_HELPER_PREVIEW").is_some() {
            1
        } else {
            30
        };
        r.view(
            Slot::Rail,
            "helper.tab",
            "helper.view.title",
            page_order,
            move || Box::new(HelperPage::new(Arc::clone(&page_core))),
        );
        r.view_glyph("helper.tab", forja_ui_kit::HELPERS.id());
        let settings_core = Arc::clone(&self.core);
        r.view(
            Slot::SettingsSection,
            "helper.settings",
            "helper.settings.title",
            20,
            move || Box::new(HelperSettings::new(Arc::clone(&settings_core))),
        );
    }
}

fn register_settings(r: &mut Registrar<'_>) {
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
        "helper.default.profile",
        SettingKind::Choice {
            options: [
                "auto",
                "aider",
                "amp",
                "antigravity",
                "claude-code",
                "cline",
                "codex",
                "continue",
                "copilot",
                "crush",
                "cursor-agent",
                "gemini",
                "goose",
                "kimi-code",
                "kiro",
                "opencode",
                "qwen-code",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        },
        SettingValue::Text("auto".to_owned()),
    );
    add(
        "helper.default.permissions",
        SettingKind::Choice {
            options: ["ask", "accept_edits", "auto", "total"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("ask".to_owned()),
    );
    add(
        "helper.launch.auto_start",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "helper.launch.mode",
        SettingKind::Choice {
            options: ["tui", "chat"].into_iter().map(str::to_owned).collect(),
        },
        SettingValue::Text("tui".to_owned()),
    );
    add(
        "helper.monitor.idle_minutes",
        SettingKind::Integer {
            min: Some(1),
            max: Some(120),
        },
        SettingValue::Integer(10),
    );
    add(
        "helper.notifications.waiting",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "helper.notifications.done",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "helper.notifications.error",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "helper.sessions.keep_transcripts",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "helper.hooks.enabled",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "helper.hooks.timeout_s",
        SettingKind::Integer {
            min: Some(1),
            max: Some(60),
        },
        SettingValue::Integer(10),
    );
    add(
        "helper.close.confirm_running",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
}

fn register_events(r: &mut Registrar<'_>, core: &Shared) {
    r.contract_event::<HelperStatusChanged>(names::HELPER_STATUS_CHANGED);
    r.contract_event::<TerminalExited>(names::TERMINAL_EXITED);
    r.contract_event::<TerminalBell>(names::TERMINAL_BELL);
    r.contract_event::<TerminalTitleChanged>(names::TERMINAL_TITLE_CHANGED);

    let exited_core = Arc::clone(core);
    r.subscribe(move |cx, event: &TerminalExited| {
        let _ = commands::ensure_current(cx, &exited_core);
        status::update_exited_run(cx, &exited_core, &event.terminal, event.exit_code);
    });
    let bell_core = Arc::clone(core);
    r.subscribe(move |cx, event: &TerminalBell| {
        let _ = commands::ensure_current(cx, &bell_core);
        status::update_run(
            cx,
            &bell_core,
            &event.terminal,
            HelperStatus::NeedsYou,
            Some("bell".to_owned()),
        );
    });
    let title_core = Arc::clone(core);
    r.subscribe(move |cx, event: &TerminalTitleChanged| {
        let _ = commands::ensure_current(cx, &title_core);
        status::infer_from_terminal(cx, &title_core, &event.terminal);
    });
}

/// Detecta o status de um terminal por título e saída.
pub(crate) fn read_output(cx: &mut forja_kernel::Ctx<'_>, terminal_id: &str) -> Option<String> {
    let mut args = forja_kernel::Args::new();
    args.insert("terminal_id", serde_json::json!(terminal_id));
    args.insert("lines", serde_json::json!(24));
    cx.run_command("terminal.read", args)
        .ok()
        .and_then(|value| {
            value
                .get("text")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use serde_json::{json, Map};
    use std::path::PathBuf;

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            for attempt in 0..20 {
                if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                    return;
                }
                if attempt < 19 {
                    std::thread::sleep(std::time::Duration::from_millis(15));
                }
            }
        }
    }

    fn temp_guard(label: &str) -> TempDirGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-helper-module-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempDirGuard(dir)
    }

    fn runtime(dir: &std::path::Path, core: Shared) -> Runtime {
        Runtime::new(
            vec![Box::new(HelperModule { core })],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("helper module registers")
    }

    #[test]
    fn failed_exit_respects_error_notification_setting() {
        struct NotifyStub(Arc<std::sync::Mutex<Vec<String>>>);
        impl Module for NotifyStub {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("notify", "helper.name", "0")
            }
            fn register(&mut self, r: &mut Registrar<'_>) {
                let calls = Arc::clone(&self.0);
                r.command("notify.show", "helper.name", move |_, args| {
                    calls
                        .lock()
                        .unwrap()
                        .push(args.str("title").unwrap_or("").to_owned());
                    Ok(json!({}))
                });
            }
        }
        let dir = temp_guard("exit-notification");
        let dir = &dir.0;
        let core = Arc::new(Core::new());
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(HelperModule {
                    core: Arc::clone(&core),
                }),
                Box::new(NotifyStub(Arc::clone(&calls))),
            ],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("en".into()),
                wake: None,
            },
        )
        .unwrap();
        core.state.lock().unwrap().runs.insert(
            "1".into(),
            status::Run {
                terminal_id: "1".into(),
                profile_id: "test".into(),
                bench_path: dir.display().to_string(),
                started_at: String::new(),
                hook_token: String::new(),
                status: HelperStatus::Working,
                last_event: None,
            },
        );
        let event = TerminalExited {
            terminal: forja_contracts::TerminalRef {
                terminal_id: "1".into(),
                bench_path: dir.display().to_string(),
                title: String::new(),
            },
            exit_code: Some(1),
        };
        runtime
            .set_setting("helper.notifications.error", SettingValue::Bool(false))
            .unwrap();
        runtime.emit(&event);
        assert!(calls.lock().unwrap().is_empty());
        runtime
            .set_setting("helper.notifications.error", SettingValue::Bool(true))
            .unwrap();
        runtime.emit(&event);
        assert_eq!(calls.lock().unwrap().as_slice(), ["Helper failed"]);
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn failed_launch_leaves_run_in_error_status() {
        let dir = temp_guard("failed-launch-error-status");
        let dir = &dir.0;
        let core = Arc::new(Core::new());
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(HelperModule {
                    core: Arc::clone(&core),
                }),
                Box::new(StubTerminal {
                    calls: Arc::clone(&calls),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("helper and stub terminal register");
        let mut profile = crate::profile::Profile::parse(
            "id = \"ghost\"\nname = \"Ghost\"\nmaker = \"Test\"\nsite = \"https://example.test\"\n[detect]\nexecutables=[\"definitely-not-a-real-helper-xyz\"]\n[start]\ncommand=\"definitely-not-a-real-helper-xyz\"\n",
            std::path::PathBuf::from("ghost.toml"),
            crate::profile::Layer::User,
        )
        .unwrap();
        profile.trusted = true;
        core.with_state(|state| {
            state.profiles = vec![profile];
            state.detection.clear();
            state.probed.clear();
            state.loaded_for = Some(dir.clone());
        })
        .unwrap();

        let mut args = forja_kernel::Args::new();
        args.insert("profile_id", serde_json::json!("ghost"));
        args.insert("bench_path", serde_json::json!(dir.display().to_string()));
        let error = runtime
            .run_command("helper.launch", args)
            .expect_err("missing executable must fail launch");
        assert!(error.to_string().contains("executable-not-found:"), "{error}");

        let status = core
            .with_state(|state| {
                state
                    .runs
                    .values()
                    .find(|run| run.profile_id == "ghost")
                    .map(|run| run.status)
            })
            .unwrap();
        assert_eq!(status, Some(HelperStatus::Error));
        let raw = core
            .with_state(|state| state.last_launch_error.clone())
            .unwrap();
        assert!(
            raw.as_deref().is_some_and(|raw| raw.contains("executable-not-found:")),
            "{raw:?}"
        );
    }

    #[test]
    fn successful_relaunch_clears_the_launch_error_run() {
        let dir = temp_guard("relaunch-clears-error");
        let dir = &dir.0;
        let core = Arc::new(Core::new());
        let mut runtime = Runtime::new(
            vec![
                Box::new(HelperModule {
                    core: Arc::clone(&core),
                }),
                Box::new(StubTerminal {
                    calls: Arc::new(std::sync::Mutex::new(Vec::new())),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("helper and stub terminal register");
        let mut profile = crate::profile::Profile::parse(
            "id = \"ghost\"\nname = \"Ghost\"\nmaker = \"Test\"\nsite = \"https://example.test\"\n[detect]\nexecutables=[\"definitely-not-a-real-helper-xyz\"]\n[start]\ncommand=\"definitely-not-a-real-helper-xyz\"\n",
            std::path::PathBuf::from("ghost.toml"),
            crate::profile::Layer::User,
        )
        .unwrap();
        profile.trusted = true;
        core.with_state(|state| {
            state.profiles = vec![profile];
            state.detection.clear();
            state.probed.clear();
            state.loaded_for = Some(dir.clone());
        })
        .unwrap();

        let mut args = forja_kernel::Args::new();
        args.insert("profile_id", serde_json::json!("ghost"));
        args.insert("bench_path", serde_json::json!(dir.display().to_string()));
        runtime
            .run_command("helper.launch", args)
            .expect_err("missing executable must fail launch");
        let status = runtime
            .call_method("helper.status", Map::new())
            .expect("helper.status");
        assert_eq!(status["runs"].as_array().map(Vec::len), Some(1));
        assert!(status["runs"][0]["terminal_id"]
            .as_str()
            .unwrap()
            .starts_with("launch-error:"));

        // O executável "aparece" e o mesmo perfil/bancada relança com sucesso.
        core.with_state(|state| {
            state.detection.insert(
                "ghost".to_owned(),
                detect::Detection {
                    executable: Some(dir.join("ghost.exe")),
                    ..detect::Detection::default()
                },
            );
        })
        .unwrap();
        let mut args = forja_kernel::Args::new();
        args.insert("profile_id", serde_json::json!("ghost"));
        args.insert("bench_path", serde_json::json!(dir.display().to_string()));
        runtime
            .run_command("helper.launch", args)
            .expect("launch now succeeds");

        let status = runtime
            .call_method("helper.status", Map::new())
            .expect("helper.status");
        let runs = status["runs"].as_array().unwrap();
        assert!(
            runs.iter()
                .all(|run| run["status"] != json!("error")),
            "{runs:?}"
        );
        assert!(
            runs.iter().all(|run| !run["terminal_id"]
                .as_str()
                .unwrap()
                .starts_with("launch-error:")),
            "{runs:?}"
        );
    }

    #[test]
    fn contract_rpc_list_and_missing_terminal() {
        let dir = temp_guard("rpc");
        let dir = &dir.0;
        let core = Arc::new(Core::new());
        let mut runtime = runtime(dir, Arc::clone(&core));
        contract::assert_ok(runtime.registry(), runtime.i18n());
        assert!(runtime.registry().method("helper.hooks.consent").is_none());
        assert!(runtime.registry().command("helper.hooks.consent").is_some());
        let list = runtime
            .call_method("helper.list", Map::new())
            .expect("list profiles");
        assert_eq!(
            list["profiles"].as_array().map(Vec::len),
            Some(16),
            "{list}"
        );
        let launch = runtime.call_method(
            "helper.launch",
            Map::from_iter([("profile_id".to_owned(), json!("claude-code"))]),
        );
        let error = launch.expect_err("terminal module is absent");
        assert!(error.message.contains("terminal"), "{}", error.message);
        let hook = runtime
            .call_method(
                "helper.hook",
                Map::from_iter([("payload".to_owned(), json!({}))]),
            )
            .expect("hook without runs");
        assert_eq!(hook, json!({}));
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn hook_status_returns_pending_then_delivers_background_result() {
        let dir = temp_guard("hook-async");
        let dir = &dir.0;
        let core = Arc::new(Core::new());
        let mut runtime = runtime(dir, Arc::clone(&core));
        let mut profile = profile::Profile::parse(
            r#"id = "claude-code"
name = "Test"
maker = "Test"
site = "https://example.test"
[detect]
executables = ["test"]
[start]
command = "test"
[prompt]
mode = "argument"
[hooks]
supported = true
mechanism = "command"
events = ["Stop"]
"#,
            dir.join("profile.toml"),
            profile::Layer::User,
        )
        .unwrap();
        profile.trusted = true;
        profile.hooks.config = vec![dir.join("settings.json").display().to_string()];
        {
            let mut state = core.state.lock().unwrap();
            state.profiles = vec![profile];
            state.loaded_for = std::env::current_dir().ok();
        }
        let params = || Map::from_iter([("profile_id".to_owned(), json!("claude-code"))]);
        let (sender, receiver) = std::sync::mpsc::channel();
        runtime.set_wake(Arc::new(move || {
            let _ = sender.send(());
        }));
        let first = runtime
            .call_method("helper.hooks.status", params())
            .unwrap();
        assert_eq!(first["pending"], true);
        while core.state.lock().unwrap().hook_busy {
            runtime.poll_tasks();
            if core.state.lock().unwrap().hook_busy {
                receiver
                    .recv_timeout(std::time::Duration::from_secs(60))
                    .expect("hook task did not signal completion");
            }
        }
        let result = runtime
            .call_method("helper.hooks.status", params())
            .unwrap();
        assert!(result["state"].is_string(), "{result}");
        assert!(!core.state.lock().unwrap().hook_busy);
        for method in ["helper.hooks.install", "helper.hooks.remove"] {
            let error = runtime.call_method(method, params()).unwrap_err();
            assert!(error.message.contains("consent"), "{}", error.message);
        }
        drop(runtime);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn hook_payload_requires_the_per_run_token_before_updating_status() {
        let dir = temp_guard("hook");
        let dir = &dir.0;
        let core = Arc::new(Core::new());
        let mut runtime = runtime(dir, Arc::clone(&core));
        core.with_state(|state| {
            state.runs.insert(
                "terminal-1".to_owned(),
                status::Run {
                    terminal_id: "terminal-1".to_owned(),
                    profile_id: "claude-code".to_owned(),
                    bench_path: std::env::current_dir().unwrap().display().to_string(),
                    started_at: chrono::Utc::now().to_rfc3339(),
                    hook_token: "run-secret".to_owned(),
                    status: HelperStatus::Working,
                    last_event: None,
                },
            );
        })
        .unwrap();
        runtime
            .call_method(
                "helper.hook",
                Map::from_iter([(
                    "payload".to_owned(),
                    json!({
                        "terminal_id": "terminal-1",
                        "hook_event_name": "Stop",
                        "cwd": std::env::current_dir().unwrap().display().to_string()
                    }),
                )]),
            )
            .expect("hook event");
        let status = core
            .with_state(|state| state.runs["terminal-1"].status)
            .unwrap();
        assert_eq!(status, HelperStatus::Working, "forged payload is ignored");

        runtime
            .call_method(
                "helper.hook",
                Map::from_iter([(
                    "payload".to_owned(),
                    json!({
                        "forja_profile_id": "claude-code",
                        "forja_token": "run-secret",
                        "forja_event": "Notification",
                        "payload": {
                            "cwd": std::env::current_dir().unwrap().display().to_string()
                        }
                    }),
                )]),
            )
            .expect("authorized hook event");
        let status = core
            .with_state(|state| state.runs["terminal-1"].status)
            .unwrap();
        assert_eq!(status, HelperStatus::NeedsYou);
        let rpc_status = runtime
            .call_method(
                "helper.status",
                Map::from_iter([("terminal_id".to_owned(), json!("terminal-1"))]),
            )
            .expect("helper.status rpc");
        assert_eq!(rpc_status["runs"].as_array().unwrap().len(), 1);
        assert_eq!(rpc_status["runs"][0]["profile_id"], "claude-code");
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    struct StubTerminal {
        calls: Arc<std::sync::Mutex<Vec<(String, serde_json::Value)>>>,
    }

    impl Module for StubTerminal {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("terminal", "terminal.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            r.i18n(
                "terminal-name = Terminal\nterminal-open = Open\nterminal-focus = Focus\n",
                "terminal-name = Terminal\nterminal-open = Abrir\nterminal-focus = Focar\n",
            );
            let open_calls = Arc::clone(&self.calls);
            r.command("terminal.open", "terminal.open", move |_, args| {
                open_calls
                    .lock()
                    .unwrap()
                    .push(("terminal.open".to_owned(), args.into_json()));
                Ok(json!({ "terminal_id": "term-default" }))
            });
            let focus_calls = Arc::clone(&self.calls);
            r.command("terminal.focus", "terminal.focus", move |_, args| {
                focus_calls
                    .lock()
                    .unwrap()
                    .push(("terminal.focus".to_owned(), args.into_json()));
                Ok(json!({ "focused": true }))
            });
        }
    }

    #[test]
    fn launch_default_and_focus_without_terminal_id_work() {
        let dir = temp_guard("launch-default-focus");
        let dir = &dir.0;
        let core = Arc::new(Core::new());
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(HelperModule {
                    core: Arc::clone(&core),
                }),
                Box::new(StubTerminal {
                    calls: Arc::clone(&calls),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("helper and stub terminal register");

        core.with_state(|state| {
            state.detection.insert(
                "claude-code".to_owned(),
                detect::Detection {
                    executable: Some(dir.join("claude.exe")),
                    ..detect::Detection::default()
                },
            );
        })
        .unwrap();

        let launched = runtime
            .run_command("helper.launch.default", forja_kernel::Args::new())
            .expect("helper.launch.default runs");
        assert_eq!(launched["terminal_id"], "term-default");

        let focused = runtime
            .run_command("helper.focus", forja_kernel::Args::new())
            .expect("helper.focus selects active run");
        assert_eq!(focused["focused"], true);
        let calls = calls.lock().unwrap();
        assert_eq!(calls.last().unwrap().0, "terminal.focus");
        assert_eq!(calls.last().unwrap().1["terminal_id"], "term-default");
        drop(calls);
        drop(runtime);
    }

    #[test]
    fn failed_exit_marks_run_failed_in_status() {
        let dir = temp_guard("failed-status");
        let core = Arc::new(Core::new());
        let mut runtime = runtime(&dir.0, Arc::clone(&core));
        core.state.lock().unwrap().runs.insert(
            "t-fail".into(),
            status::Run {
                terminal_id: "t-fail".into(),
                profile_id: "claude-code".into(),
                bench_path: dir.0.display().to_string(),
                started_at: chrono::Utc::now().to_rfc3339(),
                hook_token: String::new(),
                status: HelperStatus::Working,
                last_event: None,
            },
        );
        runtime.emit(&TerminalExited {
            terminal: forja_contracts::TerminalRef {
                terminal_id: "t-fail".into(),
                bench_path: dir.0.display().to_string(),
                title: String::new(),
            },
            exit_code: Some(1),
        });
        let status = runtime
            .call_method("helper.status", Map::new())
            .expect("helper.status");
        assert_eq!(status["runs"][0]["status"], json!("error"));
        assert_eq!(status["runs"][0]["failed"], true);
    }

    #[test]
    fn launch_rejects_uninstalled_helper_before_opening_terminal() {
        let dir = temp_guard("launch-missing");
        let core = Arc::new(Core::new());
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(HelperModule {
                    core: Arc::clone(&core),
                }),
                Box::new(StubTerminal {
                    calls: Arc::clone(&calls),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .unwrap();
        core.with_state(|state| {
            state
                .detection
                .insert("claude-code".to_owned(), detect::Detection::default());
        })
        .unwrap();
        let error = runtime
            .call_method(
                "helper.launch",
                Map::from_iter([("profile_id".to_owned(), json!("claude-code"))]),
            )
            .expect_err("uninstalled helper must fail");
        assert!(
            error.message.contains("executable-not-found:"),
            "{}",
            error.message
        );
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn focus_opens_terminal_tab_in_shell() {
        struct StubShell(Arc<std::sync::Mutex<Vec<(String, serde_json::Value)>>>);
        impl Module for StubShell {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("shell", "helper.name", "0.0.0")
            }
            fn register(&mut self, r: &mut Registrar<'_>) {
                let calls = Arc::clone(&self.0);
                r.command("shell.tab.open", "helper.name", move |_, args| {
                    calls
                        .lock()
                        .unwrap()
                        .push(("shell.tab.open".to_owned(), args.into_json()));
                    Ok(json!({ "opening": true }))
                });
            }
        }
        let dir = temp_guard("focus-tab");
        let core = Arc::new(Core::new());
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(HelperModule {
                    core: Arc::clone(&core),
                }),
                Box::new(StubTerminal {
                    calls: Arc::clone(&calls),
                }),
                Box::new(StubShell(Arc::clone(&calls))),
            ],
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .unwrap();
        core.state.lock().unwrap().runs.insert(
            "term-focus".into(),
            status::Run {
                terminal_id: "term-focus".into(),
                profile_id: "claude-code".into(),
                bench_path: dir.0.display().to_string(),
                started_at: chrono::Utc::now().to_rfc3339(),
                hook_token: String::new(),
                status: HelperStatus::Working,
                last_event: None,
            },
        );
        let focused = runtime
            .call_method(
                "helper.focus",
                Map::from_iter([("terminal_id".to_owned(), json!("term-focus"))]),
            )
            .expect("helper.focus");
        assert_eq!(focused["focused"], true);
        let calls = calls.lock().unwrap();
        assert_eq!(calls[0].0, "shell.tab.open");
        assert_eq!(calls[0].1["view_id"], "terminal.tab");
        assert_eq!(calls[1].0, "terminal.focus");
    }

    #[test]
    fn installed_truth_matches_list_launch_and_detection_precedence() {
        let dir = temp_guard("truth-source");
        let core = Arc::new(Core::new());
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(HelperModule {
                    core: Arc::clone(&core),
                }),
                Box::new(StubTerminal {
                    calls: Arc::clone(&calls),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .unwrap();

        let listed = runtime
            .call_method("helper.list", Map::new())
            .expect("helper.list");
        let listed_profiles = listed["profiles"].as_array().expect("profiles array");
        assert!(!listed_profiles.is_empty());

        core.with_state(|state| {
            assert!(state.has_detection_verdict());
            for item in listed_profiles {
                let id = item["id"].as_str().unwrap();
                assert_eq!(
                    item["installed"].as_bool().unwrap(),
                    state.is_installed(id),
                    "helper.list and HelperState must match for {id}"
                );
            }
            state.probed.insert(
                "claude-code".to_owned(),
                Some(std::path::PathBuf::from("claude")),
            );
            state
                .detection
                .insert("claude-code".to_owned(), detect::Detection::default());
            assert!(
                !state.is_installed("claude-code"),
                "explicit detection verdict must override probed cache"
            );
        })
        .unwrap();

        let updated = runtime
            .call_method("helper.list", Map::new())
            .expect("helper.list after detection override");
        let claude = updated["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == "claude-code")
            .unwrap();
        assert_eq!(claude["installed"], false);
        let error = runtime
            .call_method(
                "helper.launch",
                Map::from_iter([("profile_id".to_owned(), json!("claude-code"))]),
            )
            .expect_err("launch must obey the same installed verdict");
        assert!(
            error.message.contains("executable-not-found:"),
            "{}",
            error.message
        );
    }
}
