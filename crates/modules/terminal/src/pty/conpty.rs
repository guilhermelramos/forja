//! ConPTY no Windows: pipe de entrada, pipe de saída, Job Object e árvore.
//!
//! Cada terminal tem um console próprio. O Job Object recebe
//! `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, então fechar o terminal encerra o
//! processo e todos os descendentes sem bloquear a interface.

use forja_kernel::ui_contract::egui;
use std::{
    ffi::c_void,
    fs::File,
    io::{Read, Write},
    os::windows::io::FromRawHandle,
    path::Path,
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
};
use windows_sys::Win32::{
    Foundation::*,
    System::{Console::*, JobObjects::*, Pipes::CreatePipe, Threading::*},
};

const DEFAULT_COLS: i16 = 100;
const DEFAULT_ROWS: i16 = 30;

/// Um processo filho ligado a um pseudoconsola.
pub struct PtyProcess {
    console: HPCON,
    process: HANDLE,
    job: HANDLE,
    context: Arc<Mutex<Option<egui::Context>>>,
    repaint_pending: Arc<AtomicBool>,
    /// Bytes digitados pela interface.
    pub input: mpsc::Sender<Vec<u8>>,
    /// Saída bruta do processo.
    pub output: mpsc::Receiver<Vec<u8>>,
    /// Tamanho atual em colunas e linhas.
    pub size: (i16, i16),
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

impl PtyProcess {
    /// Inicia `program` com `arguments` dentro de `cwd`.
    #[cfg(test)]
    pub fn spawn(program: &str, arguments: &str, cwd: &Path) -> Result<Self, String> {
        Self::spawn_with_env(program, arguments, cwd, &std::collections::BTreeMap::new())
    }

    /// Inicia com variáveis de ambiente extras (`NOME=valor`).
    pub fn spawn_with_env(
        program: &str,
        arguments: &str,
        cwd: &Path,
        environment: &std::collections::BTreeMap<String, String>,
    ) -> Result<Self, String> {
        Self::spawn_with_assignment(program, arguments, cwd, environment, |job, process| unsafe {
            AssignProcessToJobObject(job, process) != 0
        })
    }

    fn spawn_with_assignment(
        program: &str,
        arguments: &str,
        cwd: &Path,
        environment: &std::collections::BTreeMap<String, String>,
        assign: impl FnOnce(HANDLE, HANDLE) -> bool,
    ) -> Result<Self, String> {
        unsafe {
            let (mut input_read, mut input_write, mut output_read, mut output_write) = (
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            if CreatePipe(&mut input_read, &mut input_write, ptr::null(), 0) == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if CreatePipe(&mut output_read, &mut output_write, ptr::null(), 0) == 0 {
                CloseHandle(input_read);
                CloseHandle(input_write);
                return Err(std::io::Error::last_os_error().to_string());
            }
            let mut console = 0;
            let result = CreatePseudoConsole(
                COORD {
                    X: DEFAULT_COLS,
                    Y: DEFAULT_ROWS,
                },
                input_read,
                output_write,
                0,
                &mut console,
            );
            if result < 0 {
                CloseHandle(input_read);
                CloseHandle(output_write);
                CloseHandle(input_write);
                CloseHandle(output_read);
                return Err(format!("ConPTY failed with HRESULT {result:#x}"));
            }

            let mut attribute_bytes = 0;
            InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut attribute_bytes);
            let mut storage =
                vec![0usize; attribute_bytes.div_ceil(std::mem::size_of::<usize>())];
            let attributes = storage.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
            if InitializeProcThreadAttributeList(attributes, 1, 0, &mut attribute_bytes) == 0 {
                let error = std::io::Error::last_os_error().to_string();
                CloseHandle(input_read);
                CloseHandle(output_write);
                ClosePseudoConsole(console);
                CloseHandle(input_write);
                CloseHandle(output_read);
                return Err(error);
            }
            if UpdateProcThreadAttribute(
                attributes,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                console as *const c_void,
                std::mem::size_of::<HPCON>(),
                ptr::null_mut(),
                ptr::null(),
            ) == 0
            {
                DeleteProcThreadAttributeList(attributes);
                let error = std::io::Error::last_os_error().to_string();
                CloseHandle(input_read);
                CloseHandle(output_write);
                ClosePseudoConsole(console);
                CloseHandle(input_write);
                CloseHandle(output_read);
                return Err(error);
            }

            let mut startup = STARTUPINFOEXW::default();
            startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
            startup.lpAttributeList = attributes;
            startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
            let mut process_info = PROCESS_INFORMATION::default();
            let mut command =
                wide(&format!("\"{}\" {}", program.replace('"', ""), arguments));
            let directory = wide(&cwd.display().to_string());

            let mut job = CreateJobObjectW(ptr::null(), ptr::null());
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if job.is_null()
                || SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const c_void,
                    std::mem::size_of_val(&limits) as u32,
                ) == 0
            {
                if !job.is_null() {
                    CloseHandle(job);
                }
                job = ptr::null_mut();
            }

            let block = environment_block(environment);
            let created = CreateProcessW(
                ptr::null(),
                command.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                0,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT | CREATE_SUSPENDED,
                block.as_ptr() as *const c_void,
                directory.as_ptr(),
                &startup.StartupInfo,
                &mut process_info,
            );
            DeleteProcThreadAttributeList(attributes);
            CloseHandle(input_read);
            CloseHandle(output_write);
            if created == 0 {
                let error = std::io::Error::last_os_error().to_string();
                CloseHandle(job);
                ClosePseudoConsole(console);
                CloseHandle(input_write);
                CloseHandle(output_read);
                return Err(error);
            }
            if !job.is_null() && !assign(job, process_info.hProcess) {
                CloseHandle(job);
                job = ptr::null_mut();
            }
            if ResumeThread(process_info.hThread) == u32::MAX {
                let error = std::io::Error::last_os_error().to_string();
                TerminateProcess(process_info.hProcess, 1);
                CloseHandle(process_info.hThread);
                CloseHandle(process_info.hProcess);
                CloseHandle(job);
                ClosePseudoConsole(console);
                CloseHandle(input_write);
                CloseHandle(output_read);
                return Err(error);
            }
            CloseHandle(process_info.hThread);

            let mut writer = File::from_raw_handle(input_write);
            let (input, input_receiver) = mpsc::channel::<Vec<u8>>();
            std::thread::Builder::new()
                .name("forja-pty-writer".to_owned())
                .spawn(move || {
                    while let Ok(bytes) = input_receiver.recv() {
                        if writer
                            .write_all(&bytes)
                            .and_then(|_| writer.flush())
                            .is_err()
                        {
                            break;
                        }
                    }
                })
                .map_err(|error| error.to_string())?;

            let mut reader = File::from_raw_handle(output_read);
            let (sender, output) = mpsc::channel();
            let context: Arc<Mutex<Option<egui::Context>>> = Arc::new(Mutex::new(None));
            let repaint_pending = Arc::new(AtomicBool::new(false));
            let reader_context = Arc::clone(&context);
            let reader_pending = Arc::clone(&repaint_pending);
            std::thread::Builder::new()
                .name("forja-pty-reader".to_owned())
                .spawn(move || {
                    let mut buffer = [0u8; 8192];
                    loop {
                        match reader.read(&mut buffer) {
                            Ok(0) | Err(_) => break,
                            Ok(count) => {
                                // Continua drenando mesmo sem receptor: fechar o
                                // pseudoconsola pode esperar por esta leitura.
                                let _ = sender.send(buffer[..count].to_vec());
                                if !reader_pending.swap(true, Ordering::AcqRel) {
                                    if let Ok(guard) = reader_context.lock() {
                                        if let Some(context) = guard.as_ref() {
                                            context.request_repaint();
                                        }
                                    }
                                }
                            }
                        }
                    }
                })
                .map_err(|error| error.to_string())?;

            Ok(Self {
                console,
                process: process_info.hProcess,
                job,
                context,
                repaint_pending,
                input,
                output,
                size: (DEFAULT_COLS, DEFAULT_ROWS),
            })
        }
    }

    /// Registra o contexto egui para acordar a interface quando chega saída.
    pub fn set_context(&self, context: &egui::Context) {
        if let Ok(mut guard) = self.context.lock() {
            *guard = Some(context.clone());
        }
    }

    /// A interface drenou a saída; a próxima leitura pode pedir repaint.
    pub fn acknowledge_output(&self) {
        self.repaint_pending.store(false, Ordering::Release);
    }

    /// Código de saída, quando o processo já terminou.
    pub fn exit_code(&self) -> Option<u32> {
        if self.alive() {
            return None;
        }
        let mut code = 0;
        unsafe {
            if GetExitCodeProcess(self.process, &mut code) != 0 {
                Some(code)
            } else {
                None
            }
        }
    }

    /// Ajusta o tamanho do pseudoconsola.
    pub fn resize(&mut self, cols: i16, rows: i16) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        if self.size != (cols, rows) {
            unsafe {
                ResizePseudoConsole(self.console, COORD { X: cols, Y: rows });
            }
            self.size = (cols, rows);
        }
    }

    /// Verdadeiro enquanto o processo raiz não terminou.
    pub fn alive(&self) -> bool {
        unsafe { WaitForSingleObject(self.process, 0) == WAIT_TIMEOUT }
    }

    /// Encerra a árvore de processos.
    pub fn stop(&mut self) {
        unsafe {
            if self.job.is_null() {
                TerminateProcess(self.process, 1);
            } else {
                TerminateJobObject(self.job, 1);
            }
        }
        self.repaint_pending.store(true, Ordering::Release);
    }
}

// Os handles pertencem a esta instância e o Windows permite usá-los de outra
// thread. O receptor viaja junto com o dono; não há acesso concorrente solto.
unsafe impl Send for PtyProcess {}

impl Drop for PtyProcess {
    fn drop(&mut self) {
        self.stop();
        let process = self.process as usize;
        let console = self.console;
        unsafe {
            if !self.job.is_null() {
                CloseHandle(self.job);
            }
        }
        // A árvore já foi encerrada; esperar o console fecha sem travar a UI.
        let _ = std::thread::Builder::new()
            .name("forja-pty-close".to_owned())
            .spawn(move || unsafe {
                WaitForSingleObject(process as HANDLE, INFINITE);
                ClosePseudoConsole(console);
                CloseHandle(process as HANDLE);
            });
    }
}

/// Bloco de ambiente UTF-16 com TERM e COLORTERM próprios.
pub fn environment_block(
    overrides: &std::collections::BTreeMap<String, String>,
) -> Vec<u16> {
    let mut variables: Vec<_> = std::env::vars_os()
        .filter(|(key, _)| {
            let key = key.to_string_lossy();
            !key.eq_ignore_ascii_case("TERM") && !key.eq_ignore_ascii_case("COLORTERM")
        })
        .collect();
    variables.push(("TERM".into(), "xterm-256color".into()));
    variables.push(("COLORTERM".into(), "truecolor".into()));
    for (key, value) in overrides {
        variables.retain(|(existing, _)| !existing.to_string_lossy().eq_ignore_ascii_case(key));
        variables.push((key.into(), value.into()));
    }
    variables.sort_by_key(|(key, _)| key.to_string_lossy().to_uppercase());
    use std::os::windows::ffi::OsStrExt;
    let mut block = Vec::new();
    for (key, value) in variables {
        block.extend(key.encode_wide());
        block.push('=' as u16);
        block.extend(value.encode_wide());
        block.push(0);
    }
    block.push(0);
    block
}
