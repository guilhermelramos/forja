//! Módulo `ports` da Forja: portas TCP em escuta, processo dono e bancada
//! provável, com abrir no navegador e encerrar com confirmação.

mod cwd;
mod model;
mod parser;
mod scan;
mod state;
mod view;

use forja_kernel::forja_protocol::{Method, MethodEffect, ResultSchema};
use forja_kernel::{
    Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot,
};
use serde_json::json;
use state::Shared;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[forja_kernel::module]
static PORTS: fn() -> Box<dyn Module> = || Box::new(PortsModule::new());

pub struct PortsModule {
    state: Shared,
}

impl PortsModule {
    pub fn new() -> Self {
        Self {
            state: Shared::new(),
        }
    }
}

impl Default for PortsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for PortsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("ports", "ports.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);

        let refresh = self.state.clone();
        r.command(
            "ports.refresh",
            "ports.command.refresh",
            move |cx, _args| {
                refresh.expire();
                view::ensure_scan(cx, &refresh);
                Ok(json!({ "pending": true }))
            },
        );

        let kill = self.state.clone();
        r.command("ports.kill", "ports.command.kill", move |cx, args| {
            let pid = args.i64("pid").unwrap_or(0).max(0) as u32;
            let confirm = args.bool("confirm").unwrap_or(false);
            if pid == 0 || pid == 4 || pid == std::process::id() {
                return Err(forja_kernel::KernelError::Message(
                    "refusing to terminate this process".to_owned(),
                ));
            }
            let ask = cx
                .setting("ports.confirm.kill")
                .and_then(SettingValue::as_bool)
                .unwrap_or(true);
            if ask && !confirm {
                return Err(forja_kernel::KernelError::Message(
                    "confirmation is required to terminate the process".to_owned(),
                ));
            }
            let state = kill.clone();
            cx.spawn(
                move || terminate(pid),
                move |_cx, result| match result {
                    Ok(()) => state.expire(),
                    Err(error) => state.set_error(error),
                },
            );
            Ok(json!({ "pending": true, "pid": pid }))
        });

        let rpc_state = self.state.clone();
        r.rpc(
            Method::new("ports.list", "ports.method.list.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("ports.method.list.result")),
            move |cx, _params| {
                view::ensure_scan(cx, &rpc_state);
                let snapshot = rpc_state.snapshot();
                Ok(json!({
                    "ports": snapshot.ports,
                    "scanned_at_ms": snapshot.scanned_at_ms,
                    "busy": rpc_state.busy(),
                    "error": snapshot.error,
                }))
            },
        );

        for key in used_keys() {
            r.uses_i18n(key);
        }

        let panel = self.state.clone();
        r.view(
            Slot::RightPanel,
            "ports.panel",
            "ports.panel.title",
            right_order(50),
            move || Box::new(view::PortsView::new(panel.clone())),
        );
        let status = self.state.clone();
        r.view(
            Slot::StatusBar,
            "ports.status",
            "ports.panel.title",
            60,
            move || Box::new(view::PortsStatus::new(status.clone())),
        );
    }
}

fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "ports.scan.interval_s",
        SettingKind::Number {
            min: Some(0.0),
            max: Some(600.0),
        },
        SettingValue::Number(30.0),
        "setting-ports-scan-interval_s-label",
        "setting-ports-scan-interval_s-help",
    ));
    r.setting(SettingSpec::new(
        "ports.confirm.kill",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-ports-confirm-kill-label",
        "setting-ports-confirm-kill-help",
    ));
}

fn terminate(pid: u32) -> Result<(), String> {
    let mut command = Command::new("taskkill");
    command.args(["/PID", &pid.to_string(), "/T", "/F"]);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let output = command
        .output()
        .map_err(|error| format!("could not run taskkill: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let line = stderr
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("");
        Err(if line.is_empty() {
            "taskkill failed".to_owned()
        } else {
            line.trim().to_owned()
        })
    }
}

fn right_order(default: i32) -> i32 {
    if std::env::var("FORJA_RIGHT_TAB").ok().as_deref() == Some("ports") {
        1
    } else {
        default
    }
}

fn used_keys() -> Vec<&'static str> {
    vec![
        "ports.name",
        "ports.panel.title",
        "ports.status.count",
        "ports.view.count",
        "ports.view.refresh",
        "ports.view.empty",
        "ports.view.open",
        "ports.view.system",
        "ports.view.kill",
        "ports.view.confirm",
        "ports.view.cancel",
        "ports.view.no-bench",
        "ports.command.refresh",
        "ports.command.kill",
        "ports.method.list.description",
        "ports.method.list.result",
        "setting-ports-scan-interval_s-label",
        "setting-ports-scan-interval_s-help",
        "setting-ports-confirm-kill-label",
        "setting-ports-confirm-kill-help",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::Args;
    use forja_kernel::{contract, Runtime, RuntimeOptions};

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-ports-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn runtime(dir: &std::path::Path) -> Runtime {
        Runtime::new(
            vec![Box::new(PortsModule::new())],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("ports registers cleanly")
    }

    #[test]
    fn contract_holds() {
        let dir = temp_dir();
        let runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn killing_your_own_process_is_refused() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let mut args = Args::new();
        args.insert("pid", json!(std::process::id()));
        args.insert("confirm", json!(true));
        assert!(runtime.run_command("ports.kill", args).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn confirmation_is_required_by_default() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let mut args = Args::new();
        args.insert("pid", json!(999_999u32));
        assert!(runtime.run_command("ports.kill", args).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn list_rpc_returns_an_empty_snapshot_before_the_first_scan() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let value = runtime
            .call_method("ports.list", serde_json::Map::new())
            .expect("ports.list");
        assert!(value["ports"].is_array());
        assert!(value["busy"].is_boolean());
        let _ = std::fs::remove_dir_all(dir);
    }
}
