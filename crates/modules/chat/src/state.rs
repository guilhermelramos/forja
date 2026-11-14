//! Estado da aba de chat: perfil, transcrição, turno e fila.

use crate::model::{now_ms, ChatMessage, ChatRole, ChatUsage, SessionStatus, TurnQueue};
use crate::profile::{self, ChatProfile};
use crate::runner::{self, Turn};
use crate::stream::StreamEvent;
use crate::transcript::{self, Format, TranscriptTail};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Uma sessão de ajudante aberta no chat.
pub struct ChatSession {
    /// Perfil do ajudante.
    pub profile: ChatProfile,
    /// Bancada.
    pub bench_path: PathBuf,
    /// Id da sessão, quando conhecido.
    pub session_id: Option<String>,
    /// Arquivo da transcrição, quando encontrado.
    pub transcript: Option<PathBuf>,
    /// Formato da transcrição.
    pub format: Format,
    /// Mensagens exibidas.
    pub messages: Vec<ChatMessage>,
    /// Estado do turno.
    pub status: SessionStatus,
    /// Fila de acompanhamento.
    pub queue: TurnQueue,
    /// Último erro.
    pub error: Option<String>,
    /// Aviso curto.
    pub notice: Option<String>,
    /// Modelo escolhido.
    pub model: Option<String>,
    /// Esforço escolhido.
    pub effort: Option<String>,
    /// Modo de permissão.
    pub permission: String,
    /// Anexos do próximo turno.
    pub attachments: Vec<PathBuf>,
    /// Terminal correspondente, quando houver.
    pub terminal_id: Option<String>,
    /// Processo do turno.
    turn: Option<TurnHandle>,
    /// Uso acumulado.
    pub usage: ChatUsage,
    /// Último evento do turno.
    pub last_event_at: Instant,
    /// Leitor incremental da transcrição.
    tail: TranscriptTail,
    /// Lê a transcrição enquanto não há turno do Forja.
    pub live_from_transcript: bool,
    /// Arquivos da bancada para o seletor `@`.
    pub files: Vec<String>,
    /// A lista de arquivos está sendo montada.
    pub files_loading: bool,
    /// Comandos descobertos para o seletor `/`.
    pub commands: Vec<String>,
    /// Comandos já carregados.
    pub commands_loaded: bool,
}

struct TurnHandle {
    turn: Turn,
    started: Instant,
}

impl ChatSession {
    /// Cria uma sessão vazia.
    pub fn new(profile: ChatProfile, bench_path: PathBuf, permission: String) -> Self {
        let format = profile::transcript_format(&profile);
        Self {
            profile,
            bench_path,
            session_id: None,
            transcript: None,
            format,
            messages: Vec::new(),
            status: SessionStatus::Idle,
            queue: TurnQueue::default(),
            error: None,
            notice: None,
            model: None,
            effort: None,
            permission,
            attachments: Vec::new(),
            terminal_id: None,
            turn: None,
            usage: ChatUsage::default(),
            last_event_at: Instant::now(),
            tail: TranscriptTail::new(),
            live_from_transcript: true,
            files: Vec::new(),
            files_loading: false,
            commands: Vec::new(),
            commands_loaded: false,
        }
    }

    /// Abre uma sessão existente pela transcrição.
    pub fn attach_session(&mut self, session_id: String) {
        self.session_id = Some(session_id.clone());
        if let Some(path) = profile::find_transcript(&self.profile, &session_id) {
            self.messages = transcript::read_transcript(self.format, &path);
            self.tail = TranscriptTail::new();
            self.transcript = Some(path);
            self.live_from_transcript = true;
        }
    }

    /// Acrescenta uma mensagem da pessoa.
    pub fn push_user(&mut self, prompt: &str) {
        self.messages.push(ChatMessage {
            role: ChatRole::User,
            blocks: vec![crate::model::ChatBlock::Text(prompt.to_owned())],
            timestamp_ms: now_ms(),
            usage: None,
        });
    }

    /// Inicia um turno agora.
    pub fn start_prompt(&mut self, prompt: String) -> Result<(), String> {
        profile::turn_args(
            &self.profile,
            self.session_id.as_deref(),
            &self.permission,
            self.model.as_deref(),
            self.effort.as_deref(),
        )?;
        self.push_user(&prompt);
        let environment = profile::turn_env(&self.profile, &self.permission);
        let turn = match runner::start_turn(
            &self.profile,
            &self.bench_path,
            self.session_id.as_deref(),
            &prompt,
            &self.permission,
            self.model.as_deref(),
            self.effort.as_deref(),
            &environment,
        ) {
            Ok(turn) => turn,
            Err(error) => {
                self.status = SessionStatus::Error;
                self.error = Some(error.clone());
                self.last_event_at = Instant::now();
                return Err(error);
            }
        };
        self.turn = Some(TurnHandle {
            turn,
            started: Instant::now(),
        });
        self.status = SessionStatus::Working;
        self.error = None;
        self.notice = None;
        self.last_event_at = Instant::now();
        self.live_from_transcript = false;
        Ok(())
    }

    /// Envia ou enfileira um prompt.
    pub fn send(&mut self, prompt: String) -> Result<bool, String> {
        if self.turn.is_some() {
            self.queue.enqueue(prompt);
            return Ok(true);
        }
        self.start_prompt(prompt)?;
        Ok(false)
    }

    /// Cancela o turno e limpa a fila.
    pub fn cancel(&mut self) {
        if let Some(handle) = self.turn.take() {
            handle.turn.cancel();
            self.notice = Some("turno cancelado".to_owned());
        }
        self.queue.cancel();
        self.status = SessionStatus::Cancelled;
        self.last_event_at = Instant::now();
    }

    /// Consome eventos do turno. Devolve quantos chegaram.
    pub fn poll_turn(&mut self, limit: usize) -> usize {
        let mut events = Vec::new();
        let mut done = None;
        if let Some(handle) = &self.turn {
            while events.len() < limit {
                match handle.turn.rx.try_recv() {
                    Ok(event) => events.push(event),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        done = Some(None);
                        break;
                    }
                }
            }
        }
        let count = events.len();
        for event in events {
            self.apply_event(event);
        }
        if let Some(code) = done {
            self.finish_turn(code);
        }
        count
    }

    fn apply_event(&mut self, event: StreamEvent) {
        self.last_event_at = Instant::now();
        match event {
            StreamEvent::SessionId(id) => {
                self.session_id = Some(id.clone());
                if self.transcript.is_none() {
                    self.transcript = profile::find_transcript(&self.profile, &id);
                }
            }
            StreamEvent::Message(message) => {
                self.status = SessionStatus::Working;
                self.messages.push(message);
            }
            StreamEvent::Usage(usage) => {
                self.usage.add(&usage);
                if let Some(message) = self
                    .messages
                    .iter_mut()
                    .rev()
                    .find(|message| message.role == ChatRole::Assistant)
                {
                    message.merge_usage(&usage);
                }
            }
            StreamEvent::Error(message) => {
                self.error = Some(message);
                self.status = SessionStatus::Error;
            }
            StreamEvent::Done(code) => {
                self.finish_turn(code);
            }
        }
    }

    fn finish_turn(&mut self, code: Option<i32>) {
        if self.turn.take().is_none() {
            return;
        }
        self.queue.finish();
        if code.is_some_and(|code| code != 0) {
            self.status = SessionStatus::Error;
            if self.error.is_none() {
                self.error = Some(format!(
                    "o processo terminou com código {}",
                    code.unwrap_or(-1)
                ));
            }
        } else {
            self.status = SessionStatus::Idle;
        }
        self.last_event_at = Instant::now();
    }

    /// Inicia o próximo item da fila, quando ocioso.
    pub fn take_next_prompt(&mut self) -> Option<String> {
        if self.turn.is_some() {
            return None;
        }
        self.queue.start_next()
    }

    /// True quando o turno passou do tempo ocioso configurado.
    pub fn idle_expired(&self, minutes: u64) -> bool {
        self.turn.is_some()
            && minutes > 0
            && self.last_event_at.elapsed() >= Duration::from_secs(minutes * 60)
    }

    /// Cancela por ociosidade.
    pub fn stop_idle(&mut self) {
        self.cancel();
        self.notice = Some("turno parado por ociosidade".to_owned());
        self.status = SessionStatus::Idle;
    }

    /// Lê o que a transcrição ganhou desde a última consulta.
    pub fn poll_transcript(&mut self) -> usize {
        if !self.live_from_transcript || self.turn.is_some() {
            return 0;
        }
        let Some(path) = &self.transcript else {
            return 0;
        };
        let messages = self.tail.poll(self.format, path);
        let count = messages.len();
        self.messages.extend(messages);
        count
    }

    /// Contagem de itens na fila.
    pub fn pending(&self) -> usize {
        self.queue.pending()
    }

    /// Resumo do estado para o cabeçalho.
    pub fn status_label(&self) -> &'static str {
        match self.status {
            SessionStatus::Idle => "idle",
            SessionStatus::Working => "working",
            SessionStatus::Cancelled => "cancelled",
            SessionStatus::Error => "error",
        }
    }

    /// Duração do turno atual em segundos.
    pub fn turn_seconds(&self) -> u64 {
        self.turn
            .as_ref()
            .map(|handle| handle.started.elapsed().as_secs())
            .unwrap_or(0)
    }
}

/// Estado do módulo.
#[derive(Default)]
pub struct ChatState {
    /// Perfis carregados.
    pub profiles: Vec<ChatProfile>,
    /// Bancada da última carga.
    pub loaded_for: Option<PathBuf>,
    /// Sessão ativa.
    pub active: Option<ChatSession>,
    /// Prévia já semeada.
    pub preview_seeded: bool,
    /// Perfis compatíveis detectados como instalados.
    pub installed_helpers: BTreeSet<String>,
    /// Comandos sugeridos de instalação por perfil.
    pub install_hints: BTreeMap<String, String>,
    /// Se a detecção já foi sincronizada pelo menos uma vez.
    pub detection_known: bool,
    /// Se o usuário escolheu manualmente um ajudante no seletor.
    pub user_picked_profile: bool,
}

impl ChatState {
    /// Carrega os perfis quando a bancada muda.
    pub fn ensure_profiles(&mut self, bench: Option<&std::path::Path>) {
        if self.loaded_for.as_deref() == bench && !self.profiles.is_empty() {
            return;
        }
        self.profiles = profile::load_profiles(bench);
        self.loaded_for = bench.map(std::path::Path::to_path_buf);
    }

    /// Perfil por id.
    pub fn profile(&self, id: &str) -> Option<&ChatProfile> {
        self.profiles.iter().find(|profile| profile.id == id)
    }

    /// Indica se um perfil está instalado conforme a última detecção.
    pub fn is_installed(&self, profile_id: &str) -> bool {
        self.installed_helpers.contains(profile_id)
    }

    /// Escolhe o perfil padrão entre os que suportam chat e estão instalados.
    pub fn pick_installed_default(&self, preferred: &str) -> Option<String> {
        if preferred != "auto"
            && self.is_installed(preferred)
            && self
                .profile(preferred)
                .is_some_and(ChatProfile::supports_stream)
        {
            return Some(preferred.to_owned());
        }
        self.profiles
            .iter()
            .find(|profile| profile.supports_stream() && self.is_installed(&profile.id))
            .map(|profile| profile.id.clone())
    }

    /// Abre uma sessão, mantendo ou não a anterior.
    pub fn open(
        &mut self,
        bench: &std::path::Path,
        profile_id: &str,
        session_id: Option<String>,
        permission: String,
    ) -> Result<(), String> {
        let profile = self
            .profile(profile_id)
            .cloned()
            .ok_or_else(|| format!("perfil de ajudante `{profile_id}` não encontrado"))?;
        let mut session = ChatSession::new(profile, bench.to_path_buf(), permission);
        if let Some(session_id) = session_id {
            session.attach_session(session_id);
        }
        self.active = Some(session);
        Ok(())
    }

    /// Fecha a sessão ativa.
    pub fn close(&mut self) {
        if let Some(session) = &mut self.active {
            if let Some(handle) = session.turn.take() {
                handle.turn.cancel();
            }
        }
        self.active = None;
    }

    /// Arquivos da bancada para o seletor `@`, em caminhos relativos.
    pub fn list_files(bench: &std::path::Path, limit: usize) -> Vec<String> {
        let mut files = Vec::new();
        let walker = ignore::WalkBuilder::new(bench)
            .hidden(false)
            .git_ignore(true)
            .max_depth(Some(12))
            .build();
        for entry in walker.flatten() {
            let path = entry.path();
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                continue;
            }
            if let Ok(relative) = path.strip_prefix(bench) {
                files.push(relative.display().to_string().replace('\\', "/"));
            }
            if files.len() >= limit {
                break;
            }
        }
        files.sort();
        files
    }

    /// Comandos de `commands/*.md` das camadas de conteúdo.
    pub fn list_commands(bench: &std::path::Path) -> Vec<String> {
        let mut commands = Vec::new();
        let mut roots = vec![
            bench.join(".forja").join("commands"),
            profile::user_content_root().join("commands"),
        ];
        if let Some(content) = std::env::var_os("FORJA_CONTENT_DIR") {
            roots.push(PathBuf::from(content).join("commands"));
        }
        for root in roots {
            let Ok(entries) = std::fs::read_dir(root) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|extension| extension == "md") {
                    if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                        commands.push(stem.to_owned());
                    }
                }
            }
        }
        commands.sort();
        commands.dedup();
        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::ChatProfile;

    fn profile() -> ChatProfile {
        let text = crate::profile::builtin_text("claude-code").unwrap();
        ChatProfile::parse(text, None, false).unwrap()
    }

    #[test]
    fn queue_and_cancel_are_local_to_the_session() {
        let mut session = ChatSession::new(
            profile(),
            std::env::temp_dir(),
            "ask".to_owned(),
        );
        assert_eq!(session.pending(), 0);
        session.queue.enqueue("one".into());
        session.queue.enqueue("two".into());
        assert_eq!(session.take_next_prompt().as_deref(), Some("one"));
        assert_eq!(session.take_next_prompt(), None);
        session.queue.finish();
        assert_eq!(session.take_next_prompt().as_deref(), Some("two"));
        session.cancel();
        assert_eq!(session.pending(), 0);
        assert_eq!(session.status, SessionStatus::Cancelled);
    }
}
