//! Operações do módulo que tocam processo ou disco. Tudo roda em `Ctx::spawn`: o teste de
//! conexão, a lista do WSL, a importação do `ssh_config`, o início e a parada de encaminhamentos,
//! e a leitura e a gravação do arquivo de destinos.

use crate::forward;
use crate::ssh::{self, ProbeError, SshOptions};
use crate::ssh_config::{self, FsSource};
use crate::state::{self, now_unix, with, Forwards, ProbeResult, Shared};
use crate::target::{SshTarget, TargetFile};
use crate::wsl::{self, WslDistro};
use forja_kernel::Ctx;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Instant;

/// Carrega o arquivo de destinos uma vez, em segundo plano.
pub fn ensure_loaded(cx: &mut Ctx<'_>, shared: &Shared, forwards: &Forwards) {
    let start = with(shared, |state| {
        if state.loaded || state.loading {
            false
        } else {
            state.loading = true;
            true
        }
    })
    .unwrap_or(false);
    if !start {
        return;
    }
    let path = cx.store().path();
    let shared = Arc::clone(shared);
    let forwards = Arc::clone(forwards);
    let restore = state::forward_mode(cx) == "auto";
    let options = state::ssh_options(cx);
    cx.spawn(move || load_file(&path), move |cx, result| {
        let targets = with(&shared, |state| {
            state.loading = false;
            state.loaded = true;
            match result {
                Ok(file) if file.targets.is_empty() && state::fixtures_enabled() => {
                    state.targets = crate::fixtures::targets();
                    state.probes = crate::fixtures::probes(now_unix());
                    state.distros = Some(crate::fixtures::distros());
                    state.fixture = true;
                }
                Ok(file) => state.targets = file.targets,
                Err(_) => {
                    state.load_failed = true;
                    state.notice = Some("remote-notice-load-failed".to_owned());
                }
            }
            state.targets.clone()
        })
        .unwrap_or_default();
        // Restaurar encaminhamentos: só quando a configuração manda e uma vez por sessão.
        if restore {
            let started = with(&shared, |state| std::mem::replace(&mut state.restored, true)).unwrap_or(true);
            if !started {
                for (target_id, port) in forward::restorable(&targets) {
                    if let Some(target) = targets.iter().find(|t| t.id == target_id) {
                        start_forward(cx, &shared, &forwards, target, &options, port);
                    }
                }
            }
        }
    });
}

/// Grava o arquivo de destinos em segundo plano. Não grava se a leitura falhou.
pub fn persist(cx: &mut Ctx<'_>, shared: &Shared) {
    let snapshot = with(shared, |state| {
        if state.load_failed || !state.loaded || state.fixture {
            return None;
        }
        Some(TargetFile {
            targets: state.targets.clone(),
        })
    })
    .flatten();
    let Some(file) = snapshot else {
        return;
    };
    let path = cx.store().path();
    let shared = Arc::clone(shared);
    cx.spawn(move || save_file(&path, &file), move |_cx, result| {
        if result.is_err() {
            with(&shared, |state| state.notice = Some("remote-notice-save-failed".to_owned()));
        }
    });
}

/// Teste de conexão de um destino. O resultado vai para `probes[id]`.
pub fn probe(cx: &mut Ctx<'_>, shared: &Shared, id: &str) -> Result<(), String> {
    let options = state::ssh_options(cx);
    let target = with(shared, |state| {
        if state.busy.contains(id) {
            return Err("this target is busy");
        }
        let target = state.targets.iter().find(|t| t.id == id).cloned().ok_or("unknown target")?;
        state.busy.insert(id.to_owned());
        Ok(target)
    })
    .unwrap_or(Err("the remote state is unavailable"))?;
    let shared = Arc::clone(shared);
    let id = id.to_owned();
    cx.spawn(move || run_probe(&target, &options), move |_cx, (outcome, elapsed_ms)| {
        with(&shared, |state| {
            state.busy.remove(&id);
            let at = now_unix();
            let result = match outcome {
                Ok(()) => ProbeResult {
                    ok: true,
                    error: None,
                    elapsed_ms,
                    at,
                },
                Err(error) => ProbeResult {
                    ok: false,
                    error: Some(error),
                    elapsed_ms,
                    at,
                },
            };
            state.probes.insert(id.clone(), result);
        });
    });
    Ok(())
}

/// Roda `ssh … echo ok` e mede o tempo. Bloqueante: só dentro de `spawn`.
pub fn run_probe(target: &SshTarget, options: &SshOptions) -> (Result<(), ProbeError>, u64) {
    let started = Instant::now();
    let mut command = forja_kernel::process::command(ssh::ssh_program());
    command.args(ssh::test_args(target, options)).stdin(Stdio::null());
    no_window(&mut command);
    let outcome = match command.output() {
        Ok(output) => ssh::probe_outcome(
            output.status.success(),
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
        ),
        Err(error) => Err(ProbeError::Other(format!("could not start ssh: {error}"))),
    };
    (outcome, started.elapsed().as_millis() as u64)
}

/// Lista as distros do WSL, em segundo plano. Sem o WSL instalado, a lista fica vazia.
pub fn list_distros(cx: &mut Ctx<'_>, shared: &Shared) {
    let start = with(shared, |state| {
        if state.distros_loading {
            false
        } else {
            state.distros_loading = true;
            true
        }
    })
    .unwrap_or(false);
    if !start {
        return;
    }
    let shared = Arc::clone(shared);
    cx.spawn(run_wsl_list, move |_cx, distros| {
        with(&shared, |state| {
            state.distros_loading = false;
            state.distros = Some(distros);
        });
    });
}

fn run_wsl_list() -> Vec<WslDistro> {
    let mut command = forja_kernel::process::command("wsl.exe");
    command.args(wsl::list_args()).stdin(Stdio::null());
    no_window(&mut command);
    match command.output() {
        Ok(output) if output.status.success() || !output.stdout.is_empty() => {
            wsl::parse_list(&wsl::decode_output(&output.stdout))
        }
        _ => Vec::new(),
    }
}

/// Importa os aliases do `ssh_config`. Os que já existem (mesmo alias) não são duplicados.
pub fn import_config(cx: &mut Ctx<'_>, shared: &Shared) {
    let path = state::config_path(cx);
    let shared = Arc::clone(shared);
    cx.spawn(move || import_from(&path), move |cx, imported: Vec<SshTarget>| {
        let added = with(&shared, |state| {
            let mut added = 0;
            for target in imported {
                if state.targets.iter().any(|t| t.host == target.host && t.label == target.label) {
                    continue;
                }
                let mut target = target;
                target.id = unique_id(&target.id, &state.targets);
                state.targets.push(target);
                added += 1;
            }
            added
        })
        .unwrap_or(0);
        if added > 0 {
            persist(cx, &shared);
        }
    });
}

/// Lê o arquivo de configuração e devolve os destinos que ele tem. Sem arquivo, nenhum.
pub fn import_from(path: &Path) -> Vec<SshTarget> {
    let Some(text) = std::fs::read_to_string(path).ok() else {
        return Vec::new();
    };
    let ssh_dir = path.parent().unwrap_or(Path::new("."));
    let entries = ssh_config::parse_hosts(&text, ssh_dir, &state::home_dir(), &FsSource);
    ssh_config::import_targets(&entries)
}

/// Sobe um encaminhamento e marca `active` para restaurá-lo na próxima abertura.
pub fn start_forward(cx: &mut Ctx<'_>, shared: &Shared, forwards: &Forwards, target: &SshTarget, options: &SshOptions, port: u16) {
    let Some(forward) = target.forwards.iter().find(|f| f.local_port == port).cloned() else {
        return;
    };
    let program = ssh::ssh_program().to_owned();
    let target = target.clone();
    let options = options.clone();
    let forwards = Arc::clone(forwards);
    let shared = Arc::clone(shared);
    cx.spawn(
        move || {
            let mut manager = forwards.lock().map_err(|_| "forwarding is unavailable".to_owned())?;
            manager.start(&program, &target, &options, &forward)
        },
        move |cx, result: Result<u32, String>| {
            let target_id = target_id_for(&shared, port);
            if let (Ok(_), Some(id)) = (&result, target_id) {
                set_active(&shared, &id, port, true);
                persist(cx, &shared);
            }
            if let Err(message) = result {
                with(&shared, |state| state.notice = Some(state::short(&message)));
            }
        },
    );
}

/// Para um encaminhamento e o marca como não restaurável.
pub fn stop_forward(cx: &mut Ctx<'_>, shared: &Shared, forwards: &Forwards, target_id: &str, port: u16) {
    let target_id = target_id.to_owned();
    let forwards = Arc::clone(forwards);
    let shared = Arc::clone(shared);
    cx.spawn(
        move || {
            forwards.lock().map(|mut manager| manager.stop(&target_id, port)).unwrap_or(false)
        },
        move |cx, _stopped| {
            let id = target_id_for(&shared, port);
            if let Some(id) = id {
                set_active(&shared, &id, port, false);
                persist(cx, &shared);
            }
        },
    );
}

/// Encontra o destino que tem um encaminhamento nesta porta local.
fn target_id_for(shared: &Shared, port: u16) -> Option<String> {
    with(shared, |state| {
        state
            .targets
            .iter()
            .find(|t| t.forwards.iter().any(|f| f.local_port == port))
            .map(|t| t.id.clone())
    })
    .flatten()
}

fn set_active(shared: &Shared, target_id: &str, port: u16, active: bool) {
    with(shared, |state| {
        if let Some(target) = state.targets.iter_mut().find(|t| t.id == target_id) {
            for forward in target.forwards.iter_mut().filter(|f| f.local_port == port) {
                forward.active = active;
            }
        }
    });
}

/// Id único para um destino novo, a partir do alias ou do rótulo.
pub fn unique_id(base: &str, taken: &[SshTarget]) -> String {
    let base = if base.is_empty() { "ssh-host".to_owned() } else { base.to_owned() };
    if !taken.iter().any(|t| t.id == base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !taken.iter().any(|t| t.id == *candidate))
        .unwrap_or(base)
}

pub fn load_file(path: &Path) -> Result<TargetFile, String> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(TargetFile::default()),
        Err(error) => Err(error.to_string()),
    }
}

pub fn save_file(path: &Path, file: &TargetFile) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(file).map_err(|error| error.to_string())?;
    forja_kernel::store::write_atomic_bytes(path, &bytes).map_err(|error| error.to_string())
}

#[cfg(windows)]
fn no_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn no_window(_command: &mut Command) {}
