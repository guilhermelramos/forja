//! Módulo `remote` da Forja: destinos SSH (importados do `~/.ssh/config` ou salvos, sem segredos),
//! teste de conexão, terminal SSH, encaminhamentos de porta locais, e WSL (distros, terminal numa
//! distro com caminho traduzido, e `remote.wsl.wrap` para os ajudantes).
//!
//! Registra a seção de Ajustes "Remoto", os comandos `remote.*` e os métodos RPC `remote.ssh.list`,
//! `remote.ssh.test`, `remote.forward.list` e `remote.wsl.list`. Contrato em `design/contracts.md`,
//! seção "Remoto".

mod actions;
mod commands;
mod fixtures;
mod forward;
mod job;
mod ssh;
mod ssh_config;
mod state;
mod target;
mod view;
mod wsl;

#[cfg(test)]
mod tests;

use crate::state::{Forwards, Shared};
use forja_kernel::forja_protocol::{Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError};
use forja_kernel::ui_contract::View;
use forja_kernel::{Args, KernelError, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot};
use serde_json::{json, Value};
use std::sync::Arc;

#[forja_kernel::module]
static REMOTE: fn() -> Box<dyn Module> = || Box::new(RemoteModule::new());

/// Dono do estado remoto. A seção de Ajustes e os comandos compartilham o `Shared`.
pub struct RemoteModule {
    shared: Shared,
    forwards: Forwards,
}

impl RemoteModule {
    pub fn new() -> Self {
        Self {
            shared: state::new_shared(),
            forwards: state::new_forwards(),
        }
    }
}

impl Default for RemoteModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for RemoteModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("remote", "remote.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);

        let shared = Arc::clone(&self.shared);
        let forwards = Arc::clone(&self.forwards);
        r.command("remote.ssh.save", "remote.command.ssh_save", move |cx, args| {
            let target = args.get("target").cloned().unwrap_or(Value::Null);
            commands::save_target(cx, &shared, target)
        });

        let shared = Arc::clone(&self.shared);
        let forwards_clone = Arc::clone(&forwards);
        r.command("remote.ssh.delete", "remote.command.ssh_delete", move |cx, args| {
            let id = args.required_str("id")?.to_owned();
            commands::delete_target(cx, &shared, &forwards_clone, &id)
        });

        let shared = Arc::clone(&self.shared);
        r.command("remote.ssh.test", "remote.command.ssh_test", move |cx, args| {
            let id = args.required_str("id")?.to_owned();
            commands::test_target(cx, &shared, &id)
        });

        let shared = Arc::clone(&self.shared);
        r.command("remote.ssh.open", "remote.command.ssh_open", move |cx, args| {
            let id = args.str("id").unwrap_or("").to_owned();
            let remote_dir = args.str("remote_dir").map(str::to_owned);
            commands::open_ssh(cx, &shared, &id, remote_dir.as_deref())
        });

        let shared = Arc::clone(&self.shared);
        r.command("remote.ssh.import", "remote.command.ssh_import", move |cx, _args| {
            actions::import_config(cx, &shared);
            Ok(json!({ "started": true }))
        });

        let shared = Arc::clone(&self.shared);
        let forwards_clone = Arc::clone(&forwards);
        r.command("remote.forward.start", "remote.command.forward_start", move |cx, args| {
            let (id, port) = forward_args(&args)?;
            commands::forward(cx, &shared, &forwards_clone, &id, port, true)
        });

        let shared = Arc::clone(&self.shared);
        let forwards_clone = Arc::clone(&forwards);
        r.command("remote.forward.stop", "remote.command.forward_stop", move |cx, args| {
            let (id, port) = forward_args(&args)?;
            commands::forward(cx, &shared, &forwards_clone, &id, port, false)
        });

        let shared = Arc::clone(&self.shared);
        r.command("remote.wsl.open", "remote.command.wsl_open", move |cx, args| {
            let distro = args.str("distro").map(str::to_owned);
            let cwd = args.str("cwd").map(str::to_owned);
            commands::open_wsl(cx, &shared, distro.as_deref(), cwd.as_deref())
        });

        let shared = Arc::clone(&self.shared);
        r.command("remote.wsl.wrap", "remote.command.wsl_wrap", move |cx, args| {
            let command = args.required_str("command")?.to_owned();
            let distro = args.str("distro").map(str::to_owned);
            let cwd = args.str("cwd").map(str::to_owned);
            commands::wrap(cx, &shared, distro.as_deref(), cwd.as_deref(), &command)
        });

        let shared = Arc::clone(&self.shared);
        let forwards_clone = Arc::clone(&forwards);
        r.rpc(
            Method::new("remote.ssh.list", "remote.method.ssh_list.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("remote.method.ssh_list.result")),
            move |cx, _params| {
                actions_ensure(cx, &shared, &forwards_clone);
                Ok(commands::ssh_snapshot(cx, &shared, &forwards_clone))
            },
        );

        let shared = Arc::clone(&self.shared);
        r.rpc(
            Method::new("remote.ssh.test", "remote.method.ssh_test.description")
                .effect(MethodEffect::Read)
                .network(true)
                .param(ParamSchema::required("id", ParamKind::String).describe("Id do destino."))
                .result(ResultSchema::new("remote.method.ssh_test.result")),
            move |cx, params| {
                let id = params
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("id is required"))?
                    .to_owned();
                commands::test_target(cx, &shared, &id).map_err(|error: KernelError| RpcError::app(error.to_string()))
            },
        );

        let shared = Arc::clone(&self.shared);
        let forwards_clone = Arc::clone(&forwards);
        r.rpc(
            Method::new("remote.forward.list", "remote.method.forward_list.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("remote.method.forward_list.result")),
            move |cx, _params| Ok(commands::ssh_snapshot(cx, &shared, &forwards_clone)),
        );

        let shared = Arc::clone(&self.shared);
        r.rpc(
            Method::new("remote.wsl.list", "remote.method.wsl_list.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("remote.method.wsl_list.result")),
            move |cx, _params| {
                actions::list_distros(cx, &shared);
                Ok(commands::wsl_snapshot(&shared))
            },
        );

        let shared = Arc::clone(&self.shared);
        let forwards_clone = Arc::clone(&forwards);
        r.view(Slot::SettingsSection, "remote.settings", "remote.name", 200, move || {
            Box::new(view::RemoteView::new(Arc::clone(&shared), Arc::clone(&forwards_clone))) as Box<dyn View>
        });

        for key in view::USED_KEYS.iter().copied() {
            r.uses_i18n(key);
        }
        for key in state::USED_KEYS.iter().copied() {
            r.uses_i18n(key);
        }
        for key in ssh::USED_KEYS.iter().copied() {
            r.uses_i18n(key);
        }
    }
}

/// Carrega o arquivo de destinos e restaura encaminhamentos na primeira leitura da tela ou da API.
fn actions_ensure(cx: &mut forja_kernel::Ctx<'_>, shared: &Shared, forwards: &Forwards) {
    actions::ensure_loaded(cx, shared, forwards);
}

/// Argumentos `id` e `local_port` de um encaminhamento.
fn forward_args(args: &Args) -> forja_kernel::Result<(String, u16)> {
    let id = args.required_str("target_id")?.to_owned();
    let port = args
        .i64("local_port")
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port > 0)
        .ok_or_else(|| KernelError::message("local_port must be between 1 and 65535"))?;
    Ok((id, port))
}

/// Configurações do grupo "Remoto" (`design/settings.toml`), geradas do catálogo.
fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "remote.ssh.enabled",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-remote-ssh-enabled-label",
        "setting-remote-ssh-enabled-help",
    ));
    r.setting(SettingSpec::new(
        "remote.ssh.config_path",
        SettingKind::Path,
        SettingValue::Text("~/.ssh/config".to_owned()),
        "setting-remote-ssh-config_path-label",
        "setting-remote-ssh-config_path-help",
    ));
    r.setting(SettingSpec::new(
        "remote.ssh.default_host",
        SettingKind::Text { max_len: Some(255) },
        SettingValue::Text("".to_owned()),
        "setting-remote-ssh-default_host-label",
        "setting-remote-ssh-default_host-help",
    ));
    r.setting(SettingSpec::new(
        "remote.ssh.connect_timeout_s",
        SettingKind::Integer { min: Some(5), max: Some(120) },
        SettingValue::Integer(20),
        "setting-remote-ssh-connect_timeout_s-label",
        "setting-remote-ssh-connect_timeout_s-help",
    ));
    r.setting(SettingSpec::new(
        "remote.ssh.keep_alive_s",
        SettingKind::Integer { min: Some(0), max: Some(300) },
        SettingValue::Integer(30),
        "setting-remote-ssh-keep_alive_s-label",
        "setting-remote-ssh-keep_alive_s-help",
    ));
    r.setting(SettingSpec::new(
        "remote.ssh.reuse_connections",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-remote-ssh-reuse_connections-label",
        "setting-remote-ssh-reuse_connections-help",
    ));
    r.setting(SettingSpec::new(
        "remote.ssh.agent_forward",
        SettingKind::Bool,
        SettingValue::Bool(false),
        "setting-remote-ssh-agent_forward-label",
        "setting-remote-ssh-agent_forward-help",
    ));
    r.setting(SettingSpec::new(
        "remote.ssh.host_key_policy",
        SettingKind::Choice { options: vec!["strict".to_owned(), "ask".to_owned(), "accept_new".to_owned()] },
        SettingValue::Text("ask".to_owned()),
        "setting-remote-ssh-host_key_policy-label",
        "setting-remote-ssh-host_key_policy-help",
    ));
    r.setting(SettingSpec::new(
        "remote.wsl.default_distro",
        SettingKind::Text { max_len: Some(255) },
        SettingValue::Text("".to_owned()),
        "setting-remote-wsl-default_distro-label",
        "setting-remote-wsl-default_distro-help",
    ));
    r.setting(SettingSpec::new(
        "remote.wsl.default_user",
        SettingKind::Text { max_len: Some(255) },
        SettingValue::Text("".to_owned()),
        "setting-remote-wsl-default_user-label",
        "setting-remote-wsl-default_user-help",
    ));
    r.setting(SettingSpec::new(
        "remote.forward.mode",
        SettingKind::Choice { options: vec!["ask".to_owned(), "auto".to_owned(), "off".to_owned()] },
        SettingValue::Text("ask".to_owned()),
        "setting-remote-forward-mode-label",
        "setting-remote-forward-mode-help",
    ));
    r.setting(SettingSpec::new(
        "remote.latency.warning_ms",
        SettingKind::Integer { min: Some(50), max: Some(2000) },
        SettingValue::Integer(300),
        "setting-remote-latency-warning_ms-label",
        "setting-remote-latency-warning_ms-help",
    ));
}
