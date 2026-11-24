//! Módulo `chat` da Forja: aba de conversa com um ajudante.
//!
//! O chat lê a transcrição do CLI ao vivo e envia turnos pelo modo headless
//! (`claude -p --output-format stream-json` e `codex exec --json`). O perfil
//! informa os caminhos, os argumentos e o ambiente; nada é fixo no módulo.

mod clipboard;
mod diff_mode;
mod helper_picker;
mod model;
mod profile;
mod runner;
mod state;
mod stream;
mod transcript;
mod view;

use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::{
    Args, Ctx, KernelError, Module, ModuleManifest, Registrar, SettingKind, SettingSpec,
    SettingValue, Slot,
};
use serde_json::{json, Map, Value};
use state::ChatState;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use view::ChatView;

/// Ponteiro compartilhado para o estado do módulo.
pub(crate) type Shared = Arc<Core>;

/// Estado global do módulo.
pub struct Core {
    /// Sessão e perfis.
    pub state: Mutex<ChatState>,
}

impl Core {
    fn new() -> Self {
        Self {
            state: Mutex::new(ChatState::default()),
        }
    }
}

#[forja_kernel::module]
static CHAT: fn() -> Box<dyn Module> = || {
    Box::new(ChatModule {
        core: Arc::new(Core::new()),
    })
};

/// Módulo chat.
pub struct ChatModule {
    core: Shared,
}

impl Module for ChatModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("chat", "chat.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);
        commands::register(r, &self.core);
        rpc::register(r, &self.core);

        for key in [
            "chat.view.title",
            "chat.view.no-session",
            "chat.view.idle",
            "chat.view.working",
            "chat.view.cancelled",
            "chat.view.error",
            "chat.view.queued",
            "chat.view.terminal",
            "chat.view.effort",
            "chat.view.default",
            "chat.view.attach",
            "chat.view.attachment-too-large",
            "chat.view.send",
            "chat.view.send-no-helper",
            "chat.view.cancel",
            "chat.view.composer-hint",
            "chat.view.files",
            "chat.view.loading-files",
            "chat.view.commands",
            "chat.view.command-hint",
            "chat.view.you",
            "chat.view.assistant",
            "chat.view.tool",
            "chat.view.system",
            "chat.view.running",
            "chat.view.ok",
            "chat.view.failed",
            "chat.view.input",
            "chat.view.output",
            "chat.view.diff",
            "chat.view.paste-failed",
            "chat.view.diff-review",
            "chat.view.diff-applied",
            "chat.view.todos",
            "chat.view.thinking",
            "chat.view.helper-picker",
            "chat.view.not-installed",
            "chat.view.none-installed",
            "chat.view.install-guide",
            "chat.view.open-helpers",
            "chat.view.send-no-helper",
            "chat.view.copy-install",
            "chat.view.copied",
            "chat.view.error-not-found",
            "chat.view.error-launch",
            "chat.view.error-provider",
            "chat.view.error-internal",
        ] {
            r.uses_i18n(key);
        }

        let view_core = Arc::clone(&self.core);
        let order = if std::env::var_os("FORJA_CHAT_PREVIEW").is_some() {
            1
        } else {
            12
        };
        r.view(
            Slot::BenchTab,
            "chat.tab",
            "chat.view.title",
            order,
            move || Box::new(ChatView::new(Arc::clone(&view_core))),
        );
    }
}

fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "chat.font.size",
        SettingKind::Integer {
            min: Some(10),
            max: Some(24),
        },
        SettingValue::Integer(14),
        "setting-chat-font-size-label",
        "setting-chat-font-size-help",
    ));
    r.setting(SettingSpec::new(
        "chat.message.max_width",
        SettingKind::Integer {
            min: Some(400),
            max: Some(1200),
        },
        SettingValue::Integer(760),
        "setting-chat-message-max_width-label",
        "setting-chat-message-max_width-help",
    ));
    r.setting(SettingSpec::new(
        "chat.input.enter_sends",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-chat-input-enter_sends-label",
        "setting-chat-input-enter_sends-help",
    ));
    r.setting(SettingSpec::new(
        "chat.attachments.max_mb",
        SettingKind::Integer {
            min: Some(1),
            max: Some(100),
        },
        SettingValue::Integer(20),
        "setting-chat-attachments-max_mb-label",
        "setting-chat-attachments-max_mb-help",
    ));
    r.setting(SettingSpec::new(
        "chat.attachments.paste_images",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-chat-attachments-paste_images-label",
        "setting-chat-attachments-paste_images-help",
    ));
    r.setting(SettingSpec::new(
        "chat.tools.show_calls",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-chat-tools-show_calls-label",
        "setting-chat-tools-show_calls-help",
    ));
    r.setting(SettingSpec::new(
        "chat.tools.collapse_output",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-chat-tools-collapse_output-label",
        "setting-chat-tools-collapse_output-help",
    ));
    r.setting(SettingSpec::new(
        "chat.diff.apply_mode",
        SettingKind::Choice {
            options: ["review", "inline", "auto"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("review".to_owned()),
        "setting-chat-diff-apply_mode-label",
        "setting-chat-diff-apply_mode-help",
    ));
}

mod commands {
    use super::*;

    pub(super) fn register(r: &mut Registrar<'_>, core: &Shared) {
        let open_core = Arc::clone(core);
        r.command("chat.open", "chat.command.open", move |cx, args| {
            let profile_id = args.required_str("profile_id")?.to_owned();
            let bench = args
                .str("bench_path")
                .map(PathBuf::from)
                .or_else(|| current_bench(cx))
                .unwrap_or_else(|| PathBuf::from("."));
            let session_id = args.str("session_id").map(str::to_owned);
            let permission = args
                .str("permission")
                .map(str::to_owned)
                .or_else(|| {
                    cx.setting("helper.default.permissions")
                        .and_then(|value| value.as_str())
                        .map(str::to_owned)
                })
                .unwrap_or_else(|| "ask".to_owned());
            let selected = {
                let mut state = open_core.state.lock().map_err(poisoned)?;
                state.ensure_profiles(Some(&bench));
                state.profile(&profile_id).cloned()
            };
            let Some(selected) = selected else {
                return Err(KernelError::Message(format!(
                    "helper profile `{profile_id}` was not found"
                )));
            };
            if selected.requires_trust {
                let Some(source_path) = selected.source_path.as_ref() else {
                    return Err(KernelError::Message(
                        "project helper profile has no trusted source".to_owned(),
                    ));
                };
                let mut trust_args = Args::new();
                trust_args.insert("kind", json!("helper_profile"));
                trust_args.insert("source_path", json!(source_path.display().to_string()));
                trust_args.insert("content_hash", json!(selected.content_hash));
                let approved = cx
                    .run_command("trust.check", trust_args)
                    .ok()
                    .and_then(|value| value.get("approved").and_then(Value::as_bool))
                    .unwrap_or(false);
                if !approved {
                    return Err(KernelError::Message(
                        "project helper profile needs trust approval".to_owned(),
                    ));
                }
            }
            let mut state = open_core.state.lock().map_err(poisoned)?;
            state
                .open(&bench, &profile_id, session_id, permission)
                .map_err(KernelError::Message)?;
            let model = args.str("model").map(str::to_owned);
            let effort = args.str("effort").map(str::to_owned);
            if let Some(session) = state.active.as_mut() {
                session.model = model;
                session.effort = effort;
            }
            let session = state.active.as_ref().expect("opened session");
            Ok(json!({
                "profile_id": session.profile.id,
                "bench_path": session.bench_path.display().to_string(),
                "session_id": session.session_id,
                "transcript": session.transcript.as_ref().map(|path| path.display().to_string()),
                "messages": session.messages.len()
            }))
        });

        let send_core = Arc::clone(core);
        r.command("chat.send", "chat.command.send", move |_, args| {
            let prompt = args.required_str("prompt")?.to_owned();
            let mut state = send_core.state.lock().map_err(poisoned)?;
            let session = state
                .active
                .as_mut()
                .ok_or_else(|| KernelError::Message("no chat session is open".to_owned()))?;
            let queued = session.send(prompt).map_err(KernelError::Message)?;
            Ok(json!({ "queued": queued, "pending": session.pending() }))
        });

        let cancel_core = Arc::clone(core);
        r.command("chat.cancel", "chat.command.cancel", move |_, _| {
            let mut state = cancel_core.state.lock().map_err(poisoned)?;
            let session = state
                .active
                .as_mut()
                .ok_or_else(|| KernelError::Message("no chat session is open".to_owned()))?;
            session.cancel();
            Ok(json!({}))
        });

        let status_core = Arc::clone(core);
        r.command("chat.status", "chat.command.status", move |_, _| {
            let state = status_core.state.lock().map_err(poisoned)?;
            let session = state
                .active
                .as_ref()
                .ok_or_else(|| KernelError::Message("no chat session is open".to_owned()))?;
            Ok(status_json(session))
        });
    }

    fn status_json(session: &crate::state::ChatSession) -> Value {
        json!({
            "profile_id": session.profile.id,
            "session_id": session.session_id,
            "bench_path": session.bench_path.display().to_string(),
            "status": session.status_label(),
            "messages": session.messages.len(),
            "pending": session.pending(),
            "usage": {
                "input_tokens": session.usage.input_tokens,
                "output_tokens": session.usage.output_tokens,
                "total_tokens": session.usage.total_tokens
            }
        })
    }

    fn current_bench(cx: &mut Ctx<'_>) -> Option<PathBuf> {
        cx.run_command("bench.current", Args::new())
            .ok()
            .and_then(|value| value.get("path").and_then(Value::as_str).map(PathBuf::from))
    }
}

fn poisoned<T>(_: T) -> KernelError {
    KernelError::Message("chat state is poisoned".to_owned())
}

mod rpc {
    use super::*;

    pub(super) fn register(r: &mut Registrar<'_>, core: &Shared) {
        let open_core = Arc::clone(core);
        r.rpc(
            Method::new("chat.open", "chat.method.open.description")
                .effect(MethodEffect::Write)
                .param(ParamSchema::required("profile_id", ParamKind::String).alias("profile"))
                .param(ParamSchema::optional("bench_path", ParamKind::String).alias("bench"))
                .param(ParamSchema::optional("session_id", ParamKind::String))
                .param(ParamSchema::optional("permission", ParamKind::String))
                .param(ParamSchema::optional("model", ParamKind::String))
                .param(ParamSchema::optional("effort", ParamKind::String))
                .result(ResultSchema::new("chat.method.open.result").schema(json!({
                    "type": "object",
                    "properties": {
                        "profile_id": { "type": "string" },
                        "bench_path": { "type": "string" },
                        "session_id": { "type": ["string", "null"] },
                        "transcript": { "type": ["string", "null"] },
                        "messages": { "type": "integer" }
                    },
                    "required": ["profile_id", "bench_path"]
                }))),
            move |cx, params| run(cx, &open_core, "chat.open", params),
        );

        let send_core = Arc::clone(core);
        r.rpc(
            Method::new("chat.send", "chat.method.send.description")
                .effect(MethodEffect::Write)
                .param(ParamSchema::required("prompt", ParamKind::String))
                .result(ResultSchema::new("chat.method.send.result").schema(json!({
                    "type": "object",
                    "properties": {
                        "queued": { "type": "boolean" },
                        "pending": { "type": "integer" }
                    },
                    "required": ["queued"]
                }))),
            move |cx, params| run(cx, &send_core, "chat.send", params),
        );

        let cancel_core = Arc::clone(core);
        r.rpc(
            Method::new("chat.cancel", "chat.method.cancel.description")
                .effect(MethodEffect::Write)
                .result(ResultSchema::new("chat.method.cancel.result")),
            move |cx, params| run(cx, &cancel_core, "chat.cancel", params),
        );

        let status_core = Arc::clone(core);
        r.rpc(
            Method::new("chat.status", "chat.method.status.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("chat.method.status.result")),
            move |cx, params| run(cx, &status_core, "chat.status", params),
        );
    }

    fn run(
        cx: &mut Ctx<'_>,
        core: &Shared,
        command: &str,
        params: Map<String, Value>,
    ) -> Result<Value, RpcError> {
        let _ = core;
        cx.run_command(command, Args::from_map(params))
            .map_err(|error| RpcError::app(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use std::path::PathBuf;

    struct TempGuard(PathBuf);

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_guard(label: &str) -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-chat-{label}-{}-{}",
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
            vec![Box::new(ChatModule {
                core: Arc::new(Core::new()),
            })],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("chat module registers")
    }

    #[test]
    fn contract_and_open_send_degrade_without_runner() {
        let guard = temp_guard("rpc");
        let dir = &guard.0;
        let mut runtime = runtime(dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let opened = runtime
            .call_method(
                "chat.open",
                Map::from_iter([
                    ("profile_id".to_owned(), json!("claude-code")),
                    ("bench_path".to_owned(), json!(dir.display().to_string())),
                ]),
            )
            .expect("open");
        assert_eq!(opened["profile_id"], "claude-code");
        assert_eq!(opened["messages"], 0);
        let status = runtime
            .call_method("chat.status", Map::new())
            .expect("status");
        assert_eq!(status["status"], "idle");
        let opened = runtime
            .call_method(
                "chat.open",
                Map::from_iter([
                    ("profile_id".to_owned(), json!("gemini")),
                    ("bench_path".to_owned(), json!(dir.display().to_string())),
                ]),
            )
            .expect("open unsupported viewer");
        assert_eq!(opened["profile_id"], "gemini");
        let error = runtime
            .call_method(
                "chat.send",
                Map::from_iter([("prompt".to_owned(), json!("hi"))]),
            )
            .expect_err("gemini has no structured runner");
        assert!(error.message.contains("structured chat runner"));
        let cancel = runtime
            .call_method("chat.cancel", Map::new())
            .expect("cancel");
        assert_eq!(cancel, json!({}));
    }

    #[test]
    fn chat_open_rejects_an_untrusted_project_profile_with_full_permissions() {
        let guard = temp_guard("untrusted-profile");
        let dir = &guard.0;
        let bench = dir.join("bench");
        let profiles = bench.join(".forja").join("helpers");
        std::fs::create_dir_all(&profiles).unwrap();
        std::fs::write(
            profiles.join("claude-code.toml"),
            "id = \"claude-code\"\nname = \"Project profile\"\n[start]\ncommand = \"claude\"\n[permissions.full]\nargs = [\"--unsafe-mode\"]\n",
        )
        .unwrap();
        let mut runtime = runtime(dir);
        let error = runtime
            .call_method(
                "chat.open",
                Map::from_iter([
                    ("profile_id".to_owned(), json!("claude-code")),
                    ("bench_path".to_owned(), json!(bench.display().to_string())),
                    ("permission".to_owned(), json!("full")),
                ]),
            )
            .expect_err("project profile needs trust approval");
        assert!(error.message.contains("trust approval"));
    }

    #[test]
    fn failed_start_prompt_preserves_the_prompt_and_sets_error_status() {
        let guard = temp_guard("failed-start");
        let dir = &guard.0;
        let mut profile = crate::profile::ChatProfile::parse(
            crate::profile::builtin_text("claude-code").unwrap(),
            None,
            false,
        )
        .unwrap();
        profile.start.command = "definitely-missing-forja-chat-binary".to_owned();
        let mut session = state::ChatSession::new(profile, dir.clone(), "ask".to_owned());
        let error = session
            .start_prompt("olá".to_owned())
            .expect_err("missing binary should fail");
        assert!(error.starts_with("executable-not-found:"), "{error}");
        assert_eq!(session.status, crate::model::SessionStatus::Error);
        assert!(session.messages.is_empty());
        assert!(session.error.is_some());
    }

    #[test]
    fn opencode_stream_parses_session_text_and_tool_use() {
        let events: Vec<_> = [
            r#"{"type":"step_start","timestamp":100,"sessionID":"ses_abc","part":{"type":"step-start"}}"#,
            r#"{"type":"text","timestamp":101,"sessionID":"ses_abc","part":{"type":"text","text":"oi tudo bem"}}"#,
            r#"{"type":"tool_use","timestamp":102,"sessionID":"ses_abc","part":{"tool":"bash","state":{"status":"completed","input":{"cmd":"ls"},"output":"ok"}}}"#,
        ]
        .into_iter()
        .flat_map(|line| stream::parse_stream_line("opencode", line))
        .collect();
        assert!(events.iter().any(|e| matches!(e, stream::StreamEvent::SessionId(id) if id == "ses_abc")));
        assert!(events.iter().any(|e| matches!(e, stream::StreamEvent::Message(m) if m.summary() == "oi tudo bem")));
        assert!(events.iter().any(|e| matches!(e, stream::StreamEvent::Message(m) if m.role == crate::model::ChatRole::Tool)));
    }

    #[test]
    fn humanizes_helper_launch_and_not_installed_errors() {
        let guard = temp_guard("humanize-chat-err");
        let mut runtime = runtime(&guard.0);
        let cx = runtime.ctx("chat");
        let raw = "command `helper.launch` failed: Helper OpenAI Codex CLI is not installed on this machine.";
        let human = helper_picker::humanize_error(&cx, "OpenAI Codex CLI", raw);
        assert!(!human.summary.contains("command `"));
        assert!(!human.summary.contains("failed:"));
        assert!(!human.summary.contains("is not installed"));
        let detail = human.detail.as_deref().unwrap();
        assert!(!detail.contains("helper.launch"), "{detail}");
        assert!(!detail.contains('`'), "{detail}");
    }
}
