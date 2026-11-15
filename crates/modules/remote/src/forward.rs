//! Encaminhamentos de porta locais: um `ssh -N -L` em segundo plano por encaminhamento.
//!
//! Todos entram num Job Object com "matar ao fechar": os processos acabam com o app, mesmo se
//! ele cair. O gerenciador não guarda nada em disco; quem decide o que deve subir é a camada de
//! comandos, que lê o `active` de cada encaminhamento.

use crate::job::JobObject;
use crate::ssh::{self, SshOptions};
use crate::target::{LocalForward, SshTarget};
use std::collections::BTreeMap;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

/// Quanto do stderr de um processo fica guardado para diagnóstico.
const STDERR_LIMIT: usize = 4096;

/// Estado de um encaminhamento.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForwardState {
    Stopped,
    Running,
    /// O processo `ssh` saiu (host inalcançável, porta ocupada…). `stderr` diz por quê.
    Exited { code: Option<i32>, stderr: String },
}

/// Uma linha de status por encaminhamento configurado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForwardStatus {
    pub target_id: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub pid: Option<u32>,
    pub state: ForwardState,
}

struct Running {
    child: Child,
    pid: u32,
    stderr: Arc<Mutex<String>>,
}

#[derive(Default)]
pub struct PortForwards {
    running: BTreeMap<(String, u16), Running>,
    job: Option<JobObject>,
}

impl PortForwards {
    /// Sobe um encaminhamento. Se já está rodando, deixa como está e devolve o pid.
    pub fn start(&mut self, program: &str, target: &SshTarget, options: &SshOptions, forward: &LocalForward) -> Result<u32, String> {
        ssh::validate_target(target)?;
        ssh::validate_forward(forward)?;
        let key = (target.id.clone(), forward.local_port);
        if let Some(existing) = self.running.get_mut(&key) {
            if existing.child.try_wait().ok().flatten().is_none() {
                return Ok(existing.pid);
            }
        }
        self.running.remove(&key);

        let mut command = forja_kernel::process::command(program);
        command
            .args(ssh::forward_args(target, options, forward))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        no_window(&mut command);
        let mut child = command
            .spawn()
            .map_err(|error| format!("could not start ssh: {error}"))?;
        let pid = child.id();
        self.assign_to_job(&child);

        let stderr = Arc::new(Mutex::new(String::new()));
        if let Some(mut pipe) = child.stderr.take() {
            let sink = Arc::clone(&stderr);
            std::thread::spawn(move || {
                use std::io::Read;
                let mut buf = [0u8; 512];
                while let Ok(n) = pipe.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    if let Ok(mut text) = sink.lock() {
                        if text.len() < STDERR_LIMIT {
                            text.push_str(&String::from_utf8_lossy(&buf[..n]));
                        }
                    }
                }
            });
        }
        self.running.insert(key, Running { child, pid, stderr });
        Ok(pid)
    }

    /// Para um encaminhamento. Devolve se havia um processo.
    pub fn stop(&mut self, target_id: &str, local_port: u16) -> bool {
        match self.running.remove(&(target_id.to_string(), local_port)) {
            Some(mut running) => {
                let _ = running.child.kill();
                let _ = running.child.wait();
                true
            }
            None => false,
        }
    }

    pub fn stop_all(&mut self) {
        let keys: Vec<(String, u16)> = self.running.keys().cloned().collect();
        for (target_id, port) in keys {
            self.stop(&target_id, port);
        }
    }

    /// Estado de cada encaminhamento configurado, na ordem dos destinos.
    pub fn statuses(&mut self, targets: &[SshTarget]) -> Vec<ForwardStatus> {
        let mut rows = Vec::new();
        for target in targets {
            for forward in &target.forwards {
                let key = (target.id.clone(), forward.local_port);
                let (pid, state) = match self.running.get_mut(&key) {
                    None => (None, ForwardState::Stopped),
                    Some(running) => match running.child.try_wait() {
                        Ok(Some(status)) => (
                            None,
                            ForwardState::Exited {
                                code: status.code(),
                                stderr: running.stderr.lock().map(|s| s.trim().to_string()).unwrap_or_default(),
                            },
                        ),
                        _ => (Some(running.pid), ForwardState::Running),
                    },
                };
                rows.push(ForwardStatus {
                    target_id: target.id.clone(),
                    local_port: forward.local_port,
                    remote_host: forward.remote_host.clone(),
                    remote_port: forward.remote_port,
                    pid,
                    state,
                });
            }
        }
        rows
    }

    fn assign_to_job(&mut self, child: &Child) {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            if self.job.is_none() {
                self.job = JobObject::new();
            }
            if let Some(job) = &self.job {
                let _ = job.assign(child.as_raw_handle());
            }
        }
        #[cfg(not(windows))]
        {
            let _ = child;
            let _ = &self.job;
        }
    }
}

impl Drop for PortForwards {
    fn drop(&mut self) {
        self.stop_all();
    }
}

/// Sobe os encaminhamentos com `active` ligado, quando a configuração manda restaurar.
pub fn restorable(targets: &[SshTarget]) -> Vec<(String, u16)> {
    targets
        .iter()
        .flat_map(|target| {
            target
                .forwards
                .iter()
                .filter(|forward| forward.active)
                .map(move |forward| (target.id.clone(), forward.local_port))
        })
        .collect()
}

#[cfg(windows)]
fn no_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn no_window(_command: &mut Command) {}

#[cfg(test)]
mod tests;
