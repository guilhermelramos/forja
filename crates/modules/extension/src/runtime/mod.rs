//! Runtime Lua de uma extensão: um estado e uma thread por extensão.
//!
//! A interface nunca executa Lua. Ela envia comandos e recebe árvores de painel
//! prontas; os pedidos que dependem de outros módulos (terminal, notificação,
//! navegador) voltam como [`HostRequest`] e são atendidos na thread da interface.

mod api;
mod io;
mod panel;

pub use panel::PanelSnapshot;

use crate::manifest::{Capabilities, Manifest};
use api::is_localhost_url;
use forja_kernel::Ctx;
use mlua::{HookTriggers, Lua, LuaOptions, StdLib, VmState};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Estado de uma extensão na tela.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostStatus {
    Loading,
    Active,
    Suspended,
    Failed,
}

/// Metadado de um comando registrado por Lua.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExtCommandInfo {
    pub id: String,
    pub extension: String,
    pub local_id: String,
    pub title: crate::manifest::Localized,
    pub description: crate::manifest::Localized,
    pub shortcut: Option<String>,
    pub arguments: Vec<CommandArgumentInfo>,
}

/// Argumento de comando.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandArgumentInfo {
    pub name: String,
    pub kind: String,
    pub required: bool,
    pub default: Value,
}

/// Metadado de uma configuração da extensão.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingInfo {
    pub name: String,
    pub kind: String,
    pub default: Value,
    pub label: crate::manifest::Localized,
    pub help: crate::manifest::Localized,
    pub options: Vec<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

/// Estado compartilhado entre a thread Lua e a interface.
pub struct HostShared {
    pub id: String,
    pub version: String,
    pub data_dir: PathBuf,
    pub capabilities: Capabilities,
    pub project: Option<PathBuf>,
    pub bench: Option<Value>,
    pub locale: String,
    pub panels: BTreeMap<String, PanelSnapshot>,
    pub commands: Vec<ExtCommandInfo>,
    pub settings_schema: Vec<SettingInfo>,
    pub settings: BTreeMap<String, Value>,
    pub store: BTreeMap<String, Value>,
    pub i18n: BTreeMap<String, String>,
    pub errors: Vec<String>,
    pub status: HostStatus,
    pub loading: bool,
    pub log_level: String,
    pub sandbox_mode: String,
    pub user_action: bool,
    pub http_times: Vec<Instant>,
    pub terminal_times: Vec<Instant>,
    pub notify_times: Vec<Instant>,
    pub error_times: Vec<Instant>,
}

impl HostShared {
    pub fn log(&mut self, level: &str, text: &str) {
        if !level_enabled(&self.log_level, level) {
            return;
        }
        let path = log_path(&self.data_dir, &self.id);
        let _ = append_log(&path, level, text);
    }

    pub fn schema(&self, name: &str) -> Option<&SettingInfo> {
        self.settings_schema.iter().find(|item| item.name == name)
    }
}

fn level_enabled(minimum: &str, level: &str) -> bool {
    let rank = |value: &str| match value {
        "error" => 0,
        "warn" => 1,
        "info" => 2,
        "debug" => 3,
        _ => 2,
    };
    rank(level) <= rank(minimum)
}

fn log_path(dir: &std::path::Path, id: &str) -> PathBuf {
    dir.join("logs")
        .join("extensions")
        .join(format!("{id}.log"))
}

fn append_log(path: &std::path::Path, level: &str, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::metadata(path).map(|meta| meta.len() > 1_048_576).unwrap_or(false) {
        let _ = std::fs::rename(path, path.with_extension("log.1"));
    }
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(
        file,
        "{} [{}] {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        level,
        text
    )
}

/// Pedido que só a interface pode atender.
pub enum HostRequest {
    TerminalList {
        reply: Sender<Result<Value, String>>,
    },
    TerminalSend {
        terminal: String,
        text: String,
        enter: bool,
        reply: Sender<Result<Value, String>>,
    },
    OpenUrl {
        url: String,
        reply: Sender<Result<Value, String>>,
    },
}

/// Mensagem da extensão para a interface.
pub enum HostMessage {
    Loaded,
    Panel(PanelSnapshot),
    Notice {
        title: String,
        text: String,
        level: String,
    },
    Error(String),
    Request(HostRequest),
}

/// Comando da interface para a extensão.
pub enum HostCommand {
    Run {
        command: String,
        args: Value,
        reply: Sender<Result<Value, String>>,
    },
    Action {
        action: String,
        event: Value,
        reply: Sender<Result<Value, String>>,
    },
    Event {
        name: String,
        payload: Value,
    },
    SettingSet {
        name: String,
        value: Value,
    },
    Context {
        project: Option<PathBuf>,
        bench: Option<Value>,
    },
    Shutdown,
}

/// Opções de inicialização.
pub struct StartOptions {
    pub manifest: Manifest,
    pub project: Option<PathBuf>,
    pub bench: Option<Value>,
    pub locale: String,
    pub data_dir: PathBuf,
    pub memory_mb: usize,
    pub timeout_ms: u64,
    pub log_level: String,
    pub sandbox_mode: String,
    pub network_allowed: bool,
    pub fs_outside_allowed: bool,
}

/// Uma extensão viva.
pub struct Host {
    pub tx: Sender<HostCommand>,
    rx: Arc<Mutex<Receiver<HostMessage>>>,
    pub shared: Arc<Mutex<HostShared>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Host {
    /// Cria o estado Lua e a thread da extensão.
    pub fn start(options: StartOptions) -> Self {
        let (command_tx, command_rx) = channel();
        let (message_tx, message_rx) = channel();
        let id = options.manifest.id.clone();
        let dir = PathBuf::from(&options.manifest.dir);
        let store_path = options
            .data_dir
            .join("extensions")
            .join(&id)
            .join("store.json");
        let settings_path = options
            .data_dir
            .join("extensions")
            .join(&id)
            .join("settings.json");
        let settings = read_json_map(&settings_path);
        let store = read_json_map(&store_path);
        let i18n = crate::manifest::read_i18n(&dir, &options.locale);
        let shared = Arc::new(Mutex::new(HostShared {
            id: id.clone(),
            version: options.manifest.version.clone(),
            data_dir: options.data_dir.clone(),
            capabilities: options.manifest.capabilities.clone(),
            project: options.project.clone(),
            bench: options.bench.clone(),
            locale: options.locale.clone(),
            panels: BTreeMap::new(),
            commands: Vec::new(),
            settings_schema: Vec::new(),
            settings,
            store,
            i18n,
            errors: Vec::new(),
            status: HostStatus::Loading,
            loading: true,
            log_level: options.log_level.clone(),
            sandbox_mode: options.sandbox_mode.clone(),
            user_action: false,
            http_times: Vec::new(),
            terminal_times: Vec::new(),
            notify_times: Vec::new(),
            error_times: Vec::new(),
        }));
        let thread_shared = Arc::clone(&shared);
        let io = io::HostIo {
            dir,
            project: options.project,
            capabilities: options.manifest.capabilities.clone(),
            network_allowed: options.network_allowed,
            fs_outside_allowed: options.fs_outside_allowed,
            store_path,
            settings_path,
        };
        let entry = options.manifest.entry.clone();
        let thread = std::thread::Builder::new()
            .name(format!("forja-ext-{id}"))
            .spawn(move || {
                run_thread(
                    entry,
                    thread_shared,
                    io,
                    command_rx,
                    message_tx,
                    options.memory_mb,
                    options.timeout_ms,
                );
            })
            .ok();
        Self {
            tx: command_tx,
            rx: Arc::new(Mutex::new(message_rx)),
            shared,
            thread: Mutex::new(thread),
        }
    }

    /// Envia um comando sem esperar.
    pub fn send(&self, command: HostCommand) -> bool {
        self.tx.send(command).is_ok()
    }

    /// Atende as mensagens pendentes.
    pub fn tick(&self, cx: &mut Ctx<'_>) {
        loop {
            let message = self.recv_timeout(Duration::from_millis(0));
            match message {
                Some(HostMessage::Request(request)) => service(cx, request),
                Some(other) => self.handle_message(cx, other),
                None => break,
            }
        }
    }

    fn recv_timeout(&self, timeout: Duration) -> Option<HostMessage> {
        self.rx
            .lock()
            .ok()
            .and_then(|rx| rx.recv_timeout(timeout).ok())
    }

    fn handle_message(&self, _cx: &mut Ctx<'_>, message: HostMessage) {
        match message {
            HostMessage::Loaded => {
                if let Ok(mut shared) = self.shared.lock() {
                    shared.loading = false;
                    if shared.status == HostStatus::Loading {
                        shared.status = HostStatus::Active;
                    }
                }
            }
            HostMessage::Panel(panel) => {
                if let Ok(mut shared) = self.shared.lock() {
                    shared.panels.insert(panel.id.clone(), panel);
                }
            }
            HostMessage::Notice { title, text, level } => {
                if let Ok(mut shared) = self.shared.lock() {
                    shared.log("info", &format!("{title}: {text}"));
                }
                let level = match level.as_str() {
                    "error" => forja_contracts::NotifyLevel::Error,
                    "warning" => forja_contracts::NotifyLevel::Attention,
                    _ => forja_contracts::NotifyLevel::Info,
                };
                let _ = _cx.emit(&forja_contracts::NotifyRequested {
                    title,
                    body: text,
                    level,
                    bench_path: None,
                });
            }
            HostMessage::Error(error) => {
                if let Ok(mut shared) = self.shared.lock() {
                    shared.log("error", &error);
                    shared.errors.push(error);
                    if shared.errors.len() > 200 {
                        let overflow = shared.errors.len() - 200;
                        shared.errors.drain(0..overflow);
                    }
                    let now = Instant::now();
                    shared
                        .error_times
                        .retain(|at| now.duration_since(*at) < Duration::from_secs(60));
                    shared.error_times.push(now);
                    if shared.error_times.len() >= 3 {
                        shared.status = HostStatus::Suspended;
                        shared.panels.clear();
                    }
                }
            }
            HostMessage::Request(request) => {
                // Pedido sem Ctx fica pendente; a próxima passagem com `call`
                // ou `tick` o atende.
                let _ = request;
            }
        }
    }

    /// Para a thread e descarta o estado Lua.
    pub fn stop(&self) {
        let _ = self.tx.send(HostCommand::Shutdown);
        if let Ok(mut thread) = self.thread.lock() {
            if let Some(thread) = thread.take() {
                let _ = thread.join();
            }
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.tx.send(HostCommand::Shutdown);
        if let Ok(thread) = self.thread.get_mut() {
            if let Some(thread) = thread.take() {
                let _ = thread.join();
            }
        }
    }
}

fn read_json_map(path: &std::path::Path) -> BTreeMap<String, Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<BTreeMap<String, Value>>(&text).ok())
        .unwrap_or_default()
}

fn service(cx: &mut Ctx<'_>, request: HostRequest) {
    match request {
        HostRequest::TerminalList { reply } => {
            let _ = reply.send(terminal_list(cx));
        }
        HostRequest::TerminalSend {
            terminal,
            text,
            enter,
            reply,
        } => {
            let mut args = forja_kernel::Args::new();
            args.insert("terminal_id", serde_json::json!(terminal));
            args.insert("text", serde_json::json!(text));
            args.insert("enter", serde_json::json!(enter));
            let result = cx
                .run_command("terminal.send", args)
                .map_err(|error| error.to_string());
            let _ = reply.send(result);
        }
        HostRequest::OpenUrl { url, reply } => {
            let _ = reply.send(open_external(&url));
        }
    }
}

fn terminal_list(cx: &mut Ctx<'_>) -> Result<Value, String> {
    let bench = cx
        .run_command("bench.current", forja_kernel::Args::new())
        .ok()
        .and_then(|value| value.get("path").and_then(Value::as_str).map(str::to_owned));
    let mut args = forja_kernel::Args::new();
    if let Some(bench) = bench {
        args.insert("bench_path", serde_json::json!(bench));
    }
    cx.run_command("terminal.list", args)
        .map_err(|error| error.to_string())
}

fn open_external(url: &str) -> Result<Value, String> {
    if !url.starts_with("https://") && !is_localhost_url(url) {
        return Err("[argument] open_url accepts https or http://localhost".to_owned());
    }
    let target = url.to_owned();
    std::thread::spawn(move || {
        #[cfg(target_os = "windows")]
        {
            let _ = forja_kernel::process::command("cmd")
                .args(["/C", "start", "", &target])
                .spawn();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = forja_kernel::process::command("xdg-open").arg(&target).spawn();
        }
    });
    Ok(serde_json::json!({ "opened": true }))
}

/// In `ask` sandbox mode, a denied capability becomes a visible request.
fn ask_notice(shared: &Arc<Mutex<HostShared>>, messages: &Sender<HostMessage>, error: &str) {
    if !error.contains("[capability_denied]") {
        return;
    }
    let Ok(state) = shared.lock() else {
        return;
    };
    if state.sandbox_mode != "ask" {
        return;
    }
    let title = format!("{}: capability", state.id);
    drop(state);
    let _ = messages.send(HostMessage::Notice {
        title,
        text: format!("{error}. Edit the manifest and approve the extension again to grant it."),
        level: "warning".to_owned(),
    });
}

fn run_thread(
    entry: String,
    shared: Arc<Mutex<HostShared>>,
    io: io::HostIo,
    commands: Receiver<HostCommand>,
    messages: Sender<HostMessage>,
    memory_mb: usize,
    timeout_ms: u64,
) {
    let lua = match Lua::new_with(
        StdLib::COROUTINE | StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::MATH,
        LuaOptions::default(),
    ) {
        Ok(lua) => lua,
        Err(error) => {
            let _ = messages.send(HostMessage::Error(format!("[internal] {error}")));
            return;
        }
    };
    let _ = lua.set_memory_limit(memory_mb.saturating_mul(1024 * 1024));
    let deadline = Arc::new(Mutex::new(Instant::now() + Duration::from_millis(timeout_ms)));
    let hook_deadline = Arc::clone(&deadline);
    let _ = lua.set_hook(
        HookTriggers::new().every_nth_instruction(10_000),
        move |_, _| {
            let expired = hook_deadline
                .lock()
                .map(|value| Instant::now() > *value)
                .unwrap_or(false);
            if expired {
                Err(mlua::Error::RuntimeError(
                    "[time] Lua call exceeded the CPU budget".to_owned(),
                ))
            } else {
                Ok(VmState::Continue)
            }
        },
    );
    if let Err(error) = api::install(&lua, Arc::clone(&shared), io.clone(), messages.clone()) {
        let _ = messages.send(HostMessage::Error(format!("[internal] {error}")));
        return;
    }
    let entry_path = io.dir.join(&entry);
    let result = std::fs::read_to_string(&entry_path)
        .map_err(|error| format!("{}: {error}", entry_path.display()))
        .and_then(|text| {
            if let Ok(mut deadline) = deadline.lock() {
                *deadline = Instant::now() + Duration::from_millis(timeout_ms);
            }
            lua.load(&text)
                .set_name(entry_path.display().to_string())
                .set_mode(mlua::chunk::ChunkMode::Text)
                .exec()
                .map_err(|error| error.to_string())
        });
    if let Err(error) = result {
        let _ = messages.send(HostMessage::Error(format!("[invalid] {error}")));
        if let Ok(mut shared) = shared.lock() {
            shared.status = HostStatus::Failed;
            shared.errors.push(error);
        }
        let _ = messages.send(HostMessage::Loaded);
        return;
    }
    if let Ok(mut shared) = shared.lock() {
        shared.loading = false;
        shared.status = HostStatus::Active;
    }
    let _ = messages.send(HostMessage::Loaded);

    while let Ok(command) = commands.recv() {
        match command {
            HostCommand::Run {
                command,
                args,
                reply,
            } => {
                if let Ok(mut shared) = shared.lock() {
                    shared.user_action = true;
                }
                let result = api::call_command(&lua, &shared, &deadline, &command, &args, timeout_ms);
                if let Ok(mut shared) = shared.lock() {
                    shared.user_action = false;
                }
                if let Err(error) = &result {
                    ask_notice(&shared, &messages, error);
                }
                let _ = reply.send(result);
            }
            HostCommand::Action {
                action,
                event,
                reply,
            } => {
                if let Ok(mut shared) = shared.lock() {
                    shared.user_action = true;
                }
                let result = api::call_action(&lua, &shared, &deadline, &action, &event, timeout_ms);
                if let Ok(mut shared) = shared.lock() {
                    shared.user_action = false;
                }
                if let Err(error) = &result {
                    ask_notice(&shared, &messages, error);
                }
                let _ = reply.send(result);
            }
            HostCommand::Event { name, payload } => {
                api::dispatch_event(&lua, &shared, &deadline, &name, &payload, timeout_ms);
            }
            HostCommand::SettingSet { name, value } => {
                api::set_setting(&lua, &shared, &deadline, &name, &value, timeout_ms);
            }
            HostCommand::Context { project, bench } => {
                if let Ok(mut shared) = shared.lock() {
                    shared.project = project;
                    shared.bench = bench;
                }
            }
            HostCommand::Shutdown => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Capabilities, Layer, Localized};
    use std::path::Path;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-ext-runtime-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn manifest(dir: &Path, id: &str, capabilities: Capabilities) -> Manifest {
        Manifest {
            id: id.to_owned(),
            name: Localized {
                pt_br: id.to_owned(),
                en: id.to_owned(),
            },
            version: "1.0.0".to_owned(),
            author: "Forja".to_owned(),
            description: Localized::default(),
            api: "1".to_owned(),
            entry: "init.lua".to_owned(),
            capabilities,
            layer: Layer::User,
            dir: dir.display().to_string(),
            hash: "hash".to_owned(),
            fingerprint: "fingerprint".to_owned(),
        }
    }

    fn start(dir: &Path, id: &str, capabilities: Capabilities) -> Host {
        Host::start(StartOptions {
            manifest: manifest(dir, id, capabilities),
            project: Some(dir.to_path_buf()),
            bench: None,
            locale: "pt-BR".to_owned(),
            data_dir: dir.join("data"),
            memory_mb: 64,
            timeout_ms: 500,
            log_level: "debug".to_owned(),
            sandbox_mode: "strict".to_owned(),
            network_allowed: false,
            fs_outside_allowed: false,
        })
    }

    #[test]
    fn sandbox_removes_os_io_and_require() {
        let dir = temp_dir("sandbox");
        std::fs::write(
            dir.join("init.lua"),
            r#"
local results = {}
results.os = type(os)
results.io = type(io)
results.require = type(require)
results.loadfile = type(loadfile)
results.dofile = type(dofile)
results.debug = type(debug)
results.package = type(package)
results.dump = type(string.dump)
forja.log.info(results.os .. "," .. results.io .. "," .. results.require .. "," .. results.package)
"#,
        )
        .unwrap();
        let host = start(&dir, "sandbox", Capabilities::default());
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut log = String::new();
        while Instant::now() < deadline {
            log = std::fs::read_to_string(dir.join("data/logs/extensions/sandbox.log"))
                .unwrap_or_default();
            if log.contains("nil,nil,nil") {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(log.contains("nil,nil,nil,nil"), "{log}");
        drop(host);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sandbox_blocks_os_io_and_require_at_runtime() {
        let dir = temp_dir("sandbox-calls");
        std::fs::write(
            dir.join("init.lua"),
            r#"
forja.commands.register({
  id = "probe",
  title = "Probe",
  run = function()
    local os_ok = pcall(function() return os.execute("echo x") end)
    local io_ok = pcall(function() return io.open("x", "r") end)
    local req_ok = pcall(function() return require("x") end)
    local bytecode = "\27Lua" .. string.rep("\0", 40)
    local byte_ok = pcall(load, bytecode, "chunk", "t")
    error(("os=%s io=%s require=%s bytecode=%s"):format(tostring(os_ok), tostring(io_ok), tostring(req_ok), tostring(byte_ok)))
  end,
})
"#,
        )
        .unwrap();
        let host = start(
            &dir,
            "sandbox-calls",
            Capabilities {
                commands: true,
                ..Capabilities::default()
            },
        );
        std::thread::sleep(Duration::from_millis(300));
        let (reply_tx, reply_rx) = channel();
        let _ = host.tx.send(HostCommand::Run {
            command: "ext.sandbox-calls.probe".to_owned(),
            args: Value::Null,
            reply: reply_tx,
        });
        let result = reply_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let error = result.expect_err("sandbox must block");
        assert!(error.contains("os=false"), "{error}");
        assert!(error.contains("io=false"), "{error}");
        assert!(error.contains("require=false"), "{error}");
        assert!(error.contains("bytecode=false"), "{error}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_denied_capability_fails_with_a_clear_code() {
        let dir = temp_dir("denied");
        std::fs::write(
            dir.join("init.lua"),
            r#"
forja.commands.register({
  id = "panel",
  title = "Panel",
  run = function()
    forja.ui.panel({ id = "p", slot = "right_panel", tree = {} })
  end,
})
"#,
        )
        .unwrap();
        let host = start(
            &dir,
            "denied",
            Capabilities {
                commands: true,
                ..Capabilities::default()
            },
        );
        std::thread::sleep(Duration::from_millis(300));
        let (reply_tx, reply_rx) = channel();
        let _ = host.tx.send(HostCommand::Run {
            command: "ext.denied.panel".to_owned(),
            args: Value::Null,
            reply: reply_tx,
        });
        let result = reply_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let error = result.expect_err("ui.panel was not granted");
        assert!(error.contains("[capability_denied] ui.panel"), "{error}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn every_subscription_for_the_same_event_fires() {
        let dir = temp_dir("subscriptions");
        std::fs::write(
            dir.join("init.lua"),
            r#"
forja.on("project.opened", function() forja.store.set("first", 1) end)
forja.on("project.opened", function() forja.store.set("second", 2) end)
"#,
        )
        .unwrap();
        let host = start(
            &dir,
            "subscriptions",
            Capabilities {
                events: vec!["project.opened".to_owned()],
                ..Capabilities::default()
            },
        );
        std::thread::sleep(Duration::from_millis(300));
        let _ = host.tx.send(HostCommand::Event {
            name: "project.opened".to_owned(),
            payload: serde_json::json!({}),
        });
        let mut found = false;
        for _ in 0..80 {
            if let Ok(shared) = host.shared.lock() {
                if shared.store.contains_key("first") && shared.store.contains_key("second") {
                    found = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(found, "both subscriptions must receive the event");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn infinite_loop_is_stopped_by_the_instruction_budget() {
        let dir = temp_dir("loop");
        std::fs::write(
            dir.join("init.lua"),
            "forja.commands.register({ id = 'spin', title = 'Spin', run = function() while true do end end })\n",
        )
        .unwrap();
        let host = start(
            &dir,
            "loop",
            Capabilities {
                commands: true,
                ..Capabilities::default()
            },
        );
        std::thread::sleep(Duration::from_millis(300));
        let (reply_tx, reply_rx) = channel();
        let _ = host.tx.send(HostCommand::Run {
            command: "ext.loop.spin".to_owned(),
            args: Value::Null,
            reply: reply_tx,
        });
        let result = reply_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("[time]"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
