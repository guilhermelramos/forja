//! Comandos e métodos RPC do módulo `remote`, e as fotos (snapshots) que o CLI e a tela leem.

use crate::actions;
use crate::forward::ForwardStatus;
use crate::ssh::{self, ProbeError};
use crate::state::{self, with, Forwards, ProbeResult, Shared};
use crate::target::{LocalForward, SshTarget, DEFAULT_SSH_PORT};
use crate::wsl;
use forja_kernel::{Args, Ctx, KernelError, Result};
use serde_json::{json, Value};

/// Lista de destinos, com o último teste, os encaminhamentos e o aviso de latência.
pub fn ssh_snapshot(cx: &Ctx<'_>, shared: &Shared, forwards: &Forwards) -> Value {
    let warning = state::latency_warning_ms(cx);
    let targets = with(shared, |state| state.targets.clone()).unwrap_or_default();
    let statuses: Vec<ForwardStatus> = forwards
        .lock()
        .map(|mut manager| manager.statuses(&targets))
        .unwrap_or_default();
    let rows: Vec<Value> = targets
        .iter()
        .map(|target| {
            let probe = with(shared, |state| state.probes.get(&target.id).cloned()).flatten();
            json!({
                "id": target.id,
                "label": target.label,
                "host": target.host,
                "user": target.user,
                "port": target.port,
                "probe": probe_json(probe.as_ref(), warning),
                "forwards": target.forwards.iter().map(|forward| forward_json(forward, &statuses)).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "enabled": state::ssh_enabled(cx),
        "default_host": state::default_host(cx),
        "targets": rows,
    })
}

fn probe_json(probe: Option<&ProbeResult>, warning_ms: u64) -> Value {
    match probe {
        None => Value::Null,
        Some(result) => json!({
            "ok": result.ok,
            "error": result.error.as_ref().map(ProbeError::key),
            "elapsed_ms": result.elapsed_ms,
            "slow": result.ok && result.elapsed_ms > warning_ms,
            "at": result.at,
        }),
    }
}

fn forward_json(forward: &LocalForward, statuses: &[ForwardStatus]) -> Value {
    let running = statuses
        .iter()
        .find(|row| row.local_port == forward.local_port)
        .map(|row| matches!(row.state, crate::forward::ForwardState::Running))
        .unwrap_or(false);
    json!({
        "local_port": forward.local_port,
        "remote_host": forward.remote_host,
        "remote_port": forward.remote_port,
        "active": forward.active,
        "running": running,
    })
}

/// Distros do WSL: o que já foi listado, e se a lista está sendo lida agora.
pub fn wsl_snapshot(shared: &Shared) -> Value {
    with(shared, |state| {
        json!({
            "loading": state.distros_loading,
            "distros": state.distros.as_ref().map(|distros| {
                distros
                    .iter()
                    .map(|d| json!({ "name": d.name, "state": d.state, "version": d.version, "default": d.is_default }))
                    .collect::<Vec<_>>()
            }),
        })
    })
    .unwrap_or_else(|| json!({ "loading": false, "distros": null }))
}

/// Grava um destino. Sem id, cria um; com id, substitui o que existe.
pub fn save_target(cx: &mut Ctx<'_>, shared: &Shared, value: Value) -> Result<Value> {
    let mut target: SshTarget = match serde_json::from_value(value) {
        Ok(target) => target,
        Err(_) => return Ok(json!({ "saved": false, "error": "remote-error-invalid" })),
    };
    if target.port == 0 {
        target.port = DEFAULT_SSH_PORT;
    }
    if let Err(message) = ssh::validate_target(&target) {
        return Ok(json!({ "saved": false, "error": "remote-error-validation", "message": message }));
    }
    if target.label.trim().is_empty() {
        target.label = target.host.clone();
    }
    let id = with(shared, |state| {
        if !state.loaded || state.load_failed {
            return Err("remote-error-loading");
        }
        if target.id.is_empty() {
            target.id = actions::unique_id(&slug(&target.label), &state.targets);
            state.targets.push(target.clone());
        } else if let Some(existing) = state.targets.iter_mut().find(|t| t.id == target.id) {
            *existing = target.clone();
        } else {
            state.targets.push(target.clone());
        }
        Ok(target.id.clone())
    })
    .unwrap_or(Err("remote-error-loading"));
    match id {
        Err(key) => Ok(json!({ "saved": false, "error": key })),
        Ok(id) => {
            actions::persist(cx, shared);
            Ok(json!({ "saved": true, "id": id }))
        }
    }
}

/// Apaga um destino e para os encaminhamentos dele.
pub fn delete_target(cx: &mut Ctx<'_>, shared: &Shared, forwards: &Forwards, id: &str) -> Result<Value> {
    let ports: Vec<u16> = with(shared, |state| {
        state
            .targets
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.forwards.iter().map(|f| f.local_port).collect())
    })
    .flatten()
    .ok_or_else(|| KernelError::message(format!("unknown target `{id}`")))?;
    if let Ok(mut manager) = forwards.lock() {
        for port in ports {
            manager.stop(id, port);
        }
    }
    with(shared, |state| state.targets.retain(|t| t.id != id));
    actions::persist(cx, shared);
    Ok(json!({ "deleted": true }))
}

/// Abre um terminal com `ssh` no destino. A conexão e as perguntas acontecem no terminal.
pub fn open_ssh(cx: &mut Ctx<'_>, shared: &Shared, id: &str, remote_dir: Option<&str>) -> Result<Value> {
    if !state::ssh_enabled(cx) {
        return Err(KernelError::message("SSH is turned off in Settings"));
    }
    let options = state::ssh_options(cx);
    let default_host = state::default_host(cx);
    let key = if id.trim().is_empty() { default_host.as_str() } else { id };
    let target = with(shared, |state| {
        state
            .targets
            .iter()
            .find(|t| t.id == key || (id.trim().is_empty() && t.host == key))
            .cloned()
    })
    .flatten()
    .ok_or_else(|| KernelError::message(format!("unknown target `{key}`")))?;
    ssh::validate_target(&target).map_err(KernelError::message)?;
    let mut args = Args::new();
    args.insert("cwd", json!(state::home_dir().display().to_string()));
    args.insert("command", json!(ssh::ssh_program()));
    args.insert("args", json!(ssh::terminal_args(&target, &options, remote_dir)));
    args.insert("title", json!(target.label));
    cx.run_command("terminal.open", args)
}

/// Pede o teste de conexão. O resultado aparece em `remote.ssh.list`.
pub fn test_target(cx: &mut Ctx<'_>, shared: &Shared, id: &str) -> Result<Value> {
    if !state::ssh_enabled(cx) {
        return Err(KernelError::message("SSH is turned off in Settings"));
    }
    actions::probe(cx, shared, id).map_err(KernelError::message)?;
    Ok(json!({ "started": true }))
}

/// Sobe ou para um encaminhamento.
pub fn forward(cx: &mut Ctx<'_>, shared: &Shared, forwards: &Forwards, id: &str, port: u16, start: bool) -> Result<Value> {
    if !state::ssh_enabled(cx) {
        return Err(KernelError::message("SSH is turned off in Settings"));
    }
    if start {
        let options = state::ssh_options(cx);
        let target = with(shared, |state| state.targets.iter().find(|t| t.id == id).cloned())
            .flatten()
            .ok_or_else(|| KernelError::message(format!("unknown target `{id}`")))?;
        actions::start_forward(cx, shared, forwards, &target, &options, port);
    } else {
        actions::stop_forward(cx, shared, forwards, id, port);
    }
    Ok(json!({ "started": true }))
}

/// Abre um terminal numa distro, com a pasta traduzida.
pub fn open_wsl(cx: &mut Ctx<'_>, shared: &Shared, distro: Option<&str>, cwd: Option<&str>) -> Result<Value> {
    let distro = resolve_distro(cx, shared, distro);
    let user = state::wsl_default_user(cx);
    let linux_cwd = cwd.and_then(wsl::windows_to_wsl_path);
    let title = if distro.is_empty() { "WSL".to_owned() } else { format!("WSL {distro}") };
    let mut args = Args::new();
    args.insert("cwd", json!(state::home_dir().display().to_string()));
    args.insert("command", json!("wsl.exe"));
    args.insert("args", json!(wsl::terminal_args(&distro, &user, linux_cwd.as_deref())));
    args.insert("title", json!(title));
    cx.run_command("terminal.open", args)
}

/// Monta o comando que roda um programa dentro do WSL. Puro: não chama o `wsl.exe`.
pub fn wrap(cx: &Ctx<'_>, shared: &Shared, distro: Option<&str>, cwd: Option<&str>, command: &str) -> Result<Value> {
    let command = command.trim();
    if command.is_empty() {
        return Err(KernelError::message("the command is empty"));
    }
    let distro = resolve_distro(cx, shared, distro);
    let user = state::wsl_default_user(cx);
    let linux_cwd = cwd.and_then(wsl::windows_to_wsl_path);
    let (program, args) = wsl::wrap_command(&distro, &user, linux_cwd.as_deref(), command);
    Ok(json!({ "program": program, "args": args, "cwd": linux_cwd }))
}

/// Distro pedida, ou a da configuração, ou a padrão do WSL já listada, nessa ordem.
fn resolve_distro(cx: &Ctx<'_>, shared: &Shared, requested: Option<&str>) -> String {
    if let Some(name) = requested.map(str::trim).filter(|name| !name.is_empty()) {
        return name.to_owned();
    }
    let configured = state::wsl_default_distro(cx);
    if !configured.is_empty() {
        return configured;
    }
    with(shared, |state| {
        state
            .distros
            .as_deref()
            .and_then(wsl::default_distro)
            .map(|distro| distro.name.clone())
    })
    .flatten()
    .unwrap_or_default()
}

/// Id de destino a partir de um rótulo: `ssh-<rótulo em minúsculas>`.
pub fn slug(label: &str) -> String {
    let slug: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "ssh-host".to_owned()
    } else {
        format!("ssh-{trimmed}")
    }
}

/// Encaminhamento novo de um destino, para o editor e o CLI.
pub fn forward_from(local_port: u16, remote_host: &str, remote_port: u16) -> LocalForward {
    LocalForward {
        local_port,
        remote_host: remote_host.to_owned(),
        remote_port,
        active: false,
    }
}
