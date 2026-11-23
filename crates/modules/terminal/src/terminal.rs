//! Um terminal: ConPTY, superfície VT, seleção, busca e rolagem.
//!
//! Nada aqui toca a interface além de copiar para a área de transferência via
//! `egui::Context` quando OSC 52 está ligado. A view chama `poll` uma vez por
//! quadro e desenha a superfície.

use crate::links;
use crate::pty::PtyProcess;
use crate::search::{self, Hit};
pub use crate::selection::Selection;
use crate::settings::TermSettings;
use crate::shells;
use crate::shells::ShellCommand;
use crate::vt::{Signal, Surface};
use forja_kernel::ui_contract::egui;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Identificador estável, atribuído pelo hub.
pub type TerminalId = u64;

const MAX_SNAPSHOT_BYTES: usize = 256 * 1024;
const WORD_SEPARATORS: &str = "/\\()[]{}<>\"'`,;:=|";
/// Tique enquanto o shell nasce: a thread que o abre não acorda a interface.
pub const SPAWN_TICK: Duration = Duration::from_millis(50);

/// Evento produzido por um terminal.
#[derive(Clone, Debug, PartialEq)]
pub enum TerminalEvent {
    /// BEL recebido.
    Bell,
    /// O processo terminou.
    Exited(Option<i32>),
    /// OSC 133;D recebido.
    CommandFinished(Option<i32>),
    /// Título mudou.
    TitleChanged,
}

/// Estado da busca dentro de um terminal.
#[derive(Clone, Debug, Default)]
pub struct SearchState {
    /// Barra visível.
    pub open: bool,
    /// Pediu foco no campo neste quadro.
    pub focus_requested: bool,
    /// O campo de texto tem foco (a view atualiza).
    pub editing: bool,
    /// Termo.
    pub query: String,
    /// Revisão da superfície já pesquisada.
    pub revision: u64,
    /// Ocorrências.
    pub hits: Vec<Hit>,
    /// Ocorrência atual.
    pub current: usize,
}

/// Terminal com processo, tela e estado de interação.
pub struct Terminal {
    /// Id estável.
    pub id: TerminalId,
    pub(crate) focus_id: Option<egui::Id>,
    pub(crate) drawn_frame: Option<u64>,
    /// Pasta de trabalho.
    pub dir: PathBuf,
    /// Executável do shell.
    pub shell: String,
    /// Argumentos fixos.
    pub args: String,
    launch_env: BTreeMap<String, String>,
    pty: Option<PtyProcess>,
    spawning: Option<mpsc::Receiver<Result<PtyProcess, String>>>,
    pending_input: Vec<Vec<u8>>,
    /// Grade e histórico.
    pub surface: Surface,
    /// Seleção de texto.
    pub selection: Option<Selection>,
    /// Busca.
    pub search: SearchState,
    /// Título definido pelo usuário.
    pub custom_title: Option<String>,
    last_title: String,
    exit_reported: bool,
    /// Falha ao iniciar.
    pub error: Option<String>,
    /// Acompanha a saída.
    pub follow: bool,
    /// Primeira linha visível quando `follow` é falso.
    pub scroll_row: usize,
    /// Quantas linhas cabem na última medição.
    pub visible_rows: usize,
    /// Instante até quando a campainha visual fica acesa.
    pub bell_until: Option<Instant>,
    /// Pede foco no próximo quadro.
    pub focus_request: bool,
    /// Pede ao backend a colagem da área de transferência.
    pub paste_request: bool,
    preview_alive: bool,
    dirty: bool,
    /// Rótulo curto do shell usado quando não há título.
    shell_label: String,
}

impl Terminal {
    /// Cria o terminal e começa a abrir o processo em uma thread própria.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        id: TerminalId,
        dir: PathBuf,
        shell: ShellCommand,
        environment: BTreeMap<String, String>,
        settings: &TermSettings,
        custom_title: Option<String>,
        restored_text: Option<String>,
        context: Option<egui::Context>,
    ) -> Self {
        let had_restore = restored_text.is_some();
        let mut surface = Surface::new();
        surface.set_history_limit(settings.history_limit());
        surface.set_shell_marks(settings.shell_marks);
        surface.set_inline_images(settings.images_enabled);
        if let Some(text) = restored_text {
            surface.feed(b"\x1b[2m");
            surface.feed(text.replace('\n', "\r\n").as_bytes());
            surface.feed(b"\r\n\x1b[0m");
        }
        let shell_label = shell.label();
        let receiver = spawn_pty_worker(
            "forja-terminal-launch",
            shell.program.clone(),
            shell.arguments.clone(),
            dir.clone(),
            environment.clone(),
            context,
        );
        let title = custom_title.clone().unwrap_or_else(|| shell_label.clone());
        Self {
            id,
            focus_id: None, drawn_frame: None,
            dir,
            shell: shell.program,
            args: shell.arguments,
            launch_env: environment,
            pty: None,
            spawning: Some(receiver),
            pending_input: Vec::new(),
            surface,
            selection: None,
            search: SearchState::default(),
            custom_title,
            last_title: title,
            exit_reported: false,
            error: None,
            follow: settings.follow_output,
            scroll_row: 0,
            visible_rows: 24,
            bell_until: None,
            focus_request: false,
            paste_request: false,
            preview_alive: false,
            dirty: had_restore,
            shell_label,
        }
    }

    /// Lê a saída, trata sinais e reporta eventos. Chamar uma vez por quadro.
    pub fn poll(
        &mut self,
        now: Instant,
        settings: &TermSettings,
        context: Option<&egui::Context>,
    ) -> Vec<TerminalEvent> {
        let mut events = Vec::new();
        if let Some(receiver) = &self.spawning {
            match receiver.try_recv() {
                Ok(Ok(pty)) => {
                    self.spawning = None;
                    for bytes in self.pending_input.drain(..) {
                        let _ = pty.input.send(bytes);
                    }
                    self.pty = Some(pty);
                }
                Ok(Err(error)) => {
                    self.spawning = None;
                    self.error = Some(error);
                    self.pending_input.clear();
                    if !self.exit_reported {
                        self.exit_reported = true;
                        self.dirty = true;
                        events.push(TerminalEvent::Exited(Some(1)));
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.spawning = None;
                    self.error = Some("terminal launch worker stopped".to_owned());
                    self.pending_input.clear();
                    if !self.exit_reported {
                        self.exit_reported = true;
                        self.dirty = true;
                        events.push(TerminalEvent::Exited(Some(1)));
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let mut data = Vec::new();
        if let Some(pty) = &mut self.pty {
            if let Some(context) = context {
                pty.set_context(context);
            }
            pty.acknowledge_output();
            while let Ok(chunk) = pty.output.try_recv() {
                data.extend_from_slice(&chunk);
            }
        }
        if !data.is_empty() {
            self.surface.feed(&data);
            self.dirty = true;
        }
        self.surface.publish_if_due(now);
        for signal in std::mem::take(&mut self.surface.signals) {
            match signal {
                Signal::Bell => {
                    if settings.bell_visual() {
                        self.bell_until = Some(now + Duration::from_millis(140));
                    }
                    if settings.bell_audible() {
                        shells::bell();
                    }
                    events.push(TerminalEvent::Bell);
                }
                Signal::CommandFinished(code) => {
                    events.push(TerminalEvent::CommandFinished(code));
                }
                Signal::Clipboard(text) => {
                    if settings.osc52_enabled {
                        if let Some(context) = context {
                            context.copy_text(text);
                        }
                    }
                }
                Signal::Prompt => {}
            }
        }
        let responses = std::mem::take(&mut self.surface.responses);
        if !responses.is_empty() {
            self.write(&responses);
        }
        if !self.exit_reported {
            if let Some(code) = self.pty.as_ref().and_then(PtyProcess::exit_code) {
                self.surface.feed(
                    format!("\r\n[process exited with code {code}]\r\n").as_bytes(),
                );
                self.exit_reported = true;
                self.dirty = true;
                if settings.bell_visual() {
                    self.bell_until = Some(now + Duration::from_millis(140));
                }
                events.push(TerminalEvent::Exited(Some(code as i32)));
                self.pty.take();
            }
        }
        let title = self.title();
        if title != self.last_title {
            self.last_title = title;
            events.push(TerminalEvent::TitleChanged);
        }
        events
    }

    /// Delivers a pane/window focus transition to the terminal process.
    pub fn report_focus(&mut self, focused: bool) {
        if let Some(bytes) = self.surface.focus_event(focused) { self.write(bytes); }
    }

    /// Escreve bytes no processo, guardando se ele ainda está abrindo.
    pub fn write(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Some(pty) = &self.pty {
            let _ = pty.input.send(bytes.to_vec());
        } else if self.spawning.is_some() {
            self.pending_input.push(bytes.to_vec());
        }
    }

    /// Envia texto e, opcionalmente, Enter.
    pub fn send(&mut self, text: &str, enter: bool) {
        self.write(text.as_bytes());
        if enter {
            self.write(b"\r");
        }
    }

    /// Redimensiona grade e pseudoconsola.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        if cols == 0 || rows == 0 {
            return;
        }
        if self.surface.cols != cols || self.surface.rows != rows {
            self.surface.resize(cols, rows);
            if let Some(pty) = &mut self.pty {
                pty.resize(cols as i16, rows as i16);
            }
            if self.follow {
                self.scroll_row = self.max_scroll();
            } else {
                self.scroll_row = self.scroll_row.min(self.max_scroll());
            }
        }
        self.visible_rows = rows;
    }

    /// Processo aberto ou abrindo.
    pub fn alive(&self) -> bool {
        self.preview_alive
            || self.spawning.is_some()
            || self.pty.as_ref().is_some_and(PtyProcess::alive)
    }

    /// Quando este terminal precisa de outro quadro sem que chegue saída.
    ///
    /// A saída acorda a interface sozinha (leitor do ConPTY) e o fim do
    /// processo também (vigia do processo). Sobra um tique rápido só enquanto
    /// o shell nasce e enquanto a campainha visual está acesa. `None` = nenhum quadro.
    pub fn next_wake(&self, now: Instant) -> Option<Duration> {
        let mut wake: Option<Duration> = None;
        let mut want = |delay: Duration| {
            wake = Some(wake.map_or(delay, |current| current.min(delay)));
        };
        if let Some(wait) = self.surface.synchronization_wait(now) { want(wait); }
        if self.spawning.is_some() {
            want(SPAWN_TICK);
        }
        if let Some(until) = self.bell_until.filter(|until| *until > now) {
            want(until - now);
        }
        wake
    }

    /// Congela um terminal de captura de tela para o shell não limpar o quadro.
    pub fn freeze_preview(&mut self) {
        self.pty.take();
        self.spawning = None;
        self.pending_input.clear();
        self.preview_alive = true;
    }

    /// Tenta iniciar novamente o processo com o mesmo comando e pasta.
    pub fn retry(&mut self, context: Option<egui::Context>) {
        if self.alive() {
            return;
        }
        self.error = None;
        self.exit_reported = false;
        self.pending_input.clear();
        self.spawning = Some(spawn_pty_worker(
            "forja-terminal-retry",
            self.shell.clone(),
            self.args.clone(),
            self.dir.clone(),
            self.launch_env.clone(),
            context,
        ));
    }

    /// Título mostrado na aba.
    pub fn title(&self) -> String {
        if let Some(title) = &self.custom_title {
            return title.clone();
        }
        if self.surface.title.is_empty() {
            self.shell_label.clone()
        } else {
            shells::short_title(&self.surface.title)
        }
    }

    /// Aplica as configurações que mudam o motor.
    pub fn apply_settings(&mut self, settings: &TermSettings) {
        self.surface.set_history_limit(settings.history_limit());
        self.surface.set_shell_marks(settings.shell_marks);
        self.surface.set_inline_images(settings.images_enabled);
        if settings.follow_output {
            self.follow = true;
        }
    }

    /// Intervalo de linhas visível.
    pub fn visible_range(&self) -> (usize, usize) {
        let total = self.surface.display().total_rows();
        let rows = self.visible_rows.max(1);
        let start = if self.follow {
            self.surface.display().history.len()
        } else {
            self.scroll_row.min(self.surface.display().history.len())
        };
        (start, (start + rows).min(total))
    }

    /// Última posição de rolagem: o topo da tela atual.
    pub fn max_scroll(&self) -> usize {
        self.surface.history.len()
    }

    /// Rola por linhas (positivo desce).
    pub fn scroll_by(&mut self, rows: isize) {
        let max = self.max_scroll();
        let current = if self.follow {
            max
        } else {
            self.scroll_row.min(max)
        };
        let next = current.saturating_add_signed(rows).min(max);
        self.scroll_row = next;
        self.follow = next >= max && rows >= 0;
    }

    /// Rola uma página.
    pub fn scroll_page(&mut self, down: bool) {
        let step = self.visible_rows.max(1) as isize;
        self.scroll_by(if down { step } else { -step });
    }

    /// Vai ao início do histórico.
    pub fn scroll_top(&mut self) {
        self.follow = false;
        self.scroll_row = 0;
    }

    /// Volta ao fim.
    pub fn scroll_bottom(&mut self) {
        self.follow = true;
        self.scroll_row = self.max_scroll();
    }

    /// Mantém a linha indicada visível.
    pub fn reveal_row(&mut self, row: usize) {
        let rows = self.visible_rows.max(1);
        let start = row.saturating_sub(rows / 4);
        self.follow = false;
        self.scroll_row = start.min(self.max_scroll());
    }

    /// Texto selecionado, respeitando `terminal.copy.trim`.
    pub fn selected_text(&self, settings: &TermSettings) -> Option<String> {
        let selection = self.selection?;
        if selection.generation != self.surface.generation || selection.anchor == selection.head {
            return None;
        }
        let (start, end) = if selection.anchor <= selection.head {
            (selection.anchor, selection.head)
        } else {
            (selection.head, selection.anchor)
        };
        let mut lines = Vec::new();
        for absolute in start.0..=end.0 {
            let relative = absolute.saturating_sub(self.surface.history_start);
            let Some(line) = self.surface.line_at(relative) else {
                continue;
            };
            let Some(mut range) = selection.range(absolute, self.surface.cols, self.surface.generation)
            else {
                continue;
            };
            let visible = line.len().min(self.surface.cols);
            if range.start < visible && line[range.start].width == 0 {
                range.start = range.start.saturating_sub(1);
            }
            if range.end < visible && line[range.end].width == 0 {
                range.end += 1;
            }
            let mut text = String::new();
            for col in range.clone() {
                let cell = line.display_cell(col, self.surface.cols);
                if cell.width != 0 {
                    text.push(cell.ch);
                    if cell.emoji {
                        text.push('\u{fe0f}');
                    }
                }
            }
            if range.end == self.surface.cols {
                text.truncate(text.trim_end().len());
            }
            if settings.copy_trim {
                text.truncate(text.trim_end().len());
            }
            lines.push(text);
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    /// Tudo quando não há seleção.
    pub fn copy_text(&self, settings: &TermSettings) -> String {
        self.selected_text(settings)
            .unwrap_or_else(|| self.surface.text())
    }

    /// Seleciona o histórico inteiro.
    pub fn select_all(&mut self) {
        let end = self.surface.history_start + self.surface.total_rows().saturating_sub(1);
        self.selection = Some(Selection {
            anchor: (self.surface.history_start, 0),
            head: (end, self.surface.cols),
            generation: self.surface.generation,
        });
        self.follow = false;
    }

    /// Limpa tela e histórico locais.
    pub fn clear(&mut self) {
        let (cols, rows) = (self.surface.cols, self.surface.rows);
        self.surface = Surface::new();
        self.surface.resize(cols, rows);
        self.selection = None;
        self.search.revision = u64::MAX;
        self.follow = true;
        self.scroll_row = 0;
        self.dirty = true;
    }

    /// Abre ou fecha a barra de busca.
    pub fn toggle_search(&mut self) {
        self.search.open = !self.search.open;
        self.search.focus_requested = self.search.open;
        if !self.search.open {
            self.search.editing = false;
        }
    }

    /// Abre a busca com um termo.
    pub fn open_search(&mut self, query: String) {
        self.search.open = true;
        self.search.focus_requested = true;
        if self.search.query != query {
            self.search.query = query;
            self.search.revision = u64::MAX;
            self.search.current = 0;
        }
    }

    /// Recalcula ocorrências quando a tela mudou.
    pub fn refresh_search(&mut self) {
        if !self.search.open {
            return;
        }
        if self.search.revision != self.surface.revision {
            self.search.hits = search::find(&self.surface, &self.search.query);
            self.search.revision = self.surface.revision;
            self.search.current = self
                .search
                .current
                .min(self.search.hits.len().saturating_sub(1));
        }
    }

    /// Próxima/anterior ocorrência.
    pub fn search_step(&mut self, previous: bool) {
        if self.search.hits.is_empty() {
            return;
        }
        let count = self.search.hits.len();
        self.search.current = if previous {
            (self.search.current + count - 1) % count
        } else {
            (self.search.current + 1) % count
        };
        self.reveal_row(self.search.hits[self.search.current].row);
    }

    /// Ocorrência atual.
    pub fn current_hit(&self) -> Option<Hit> {
        self.search.hits.get(self.search.current).copied()
    }

    /// Intervalo de palavra na célula, para duplo clique.
    pub fn word_at(&self, row: usize, col: usize) -> Option<(usize, usize)> {
        let line = self
            .surface
            .line_at(row.saturating_sub(self.surface.history_start))?;
        Some(links::word_bounds(line, col, WORD_SEPARATORS))
    }

    /// Link sob a célula absoluta.
    pub fn link_at(&self, absolute_row: usize, col: usize) -> Option<links::Link> {
        links::detect(
            &self.surface,
            absolute_row.saturating_sub(self.surface.history_start),
            col,
        )
    }

    /// Resolve um link de arquivo contra a pasta do terminal.
    pub fn resolve_link(&self, link: &links::Link) -> links::Link {
        match link {
            links::Link::File { path, line, column } => links::Link::File {
                path: links::resolve(path, &self.dir),
                line: *line,
                column: *column,
            },
            other => other.clone(),
        }
    }

    /// Texto limitado para gravar em segundo plano.
    pub fn snapshot_text(&self) -> String {
        let text = self.surface.tail(self.surface.history_limit.min(5_000));
        if text.len() <= MAX_SNAPSHOT_BYTES {
            return text;
        }
        let mut start = text.len() - MAX_SNAPSHOT_BYTES;
        while !text.is_char_boundary(start) {
            start += 1;
        }
        text[start..].to_owned()
    }

    /// Conteúdo mudou desde a última gravação.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// Cria um terminal sem processo filho para testes de renderização.
    #[cfg(test)]
    pub(crate) fn test_dummy(id: TerminalId) -> Self {
        Self {
            id,
            focus_id: None,
            drawn_frame: None,
            dir: PathBuf::from("."),
            shell: "test".to_owned(),
            args: String::new(),
            launch_env: BTreeMap::new(),
            pty: None,
            spawning: None,
            pending_input: Vec::new(),
            surface: Surface::new(),
            selection: None,
            search: SearchState::default(),
            custom_title: None,
            last_title: "test".to_owned(),
            exit_reported: false,
            error: None,
            follow: true,
            scroll_row: 0,
            visible_rows: 24,
            bell_until: None,
            focus_request: false,
            paste_request: false,
            preview_alive: true,
            dirty: false,
            shell_label: "test".to_owned(),
        }
    }

    /// Encerra a árvore de processos agora (usado nos testes do hub).
    #[cfg(test)]
    pub fn stop(&mut self) {
        self.spawning = None;
        self.pending_input.clear();
        if let Some(mut pty) = self.pty.take() {
            pty.stop();
        }
    }
}

fn spawn_pty_worker(
    thread_name: &str,
    program: String,
    arguments: String,
    cwd: PathBuf,
    environment: BTreeMap<String, String>,
    context: Option<egui::Context>,
) -> mpsc::Receiver<Result<PtyProcess, String>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name(thread_name.to_owned())
        .spawn(move || {
            let result = PtyProcess::spawn_with_env(&program, &arguments, &cwd, &environment);
            if let (Ok(pty), Some(context)) = (&result, &context) {
                pty.set_context(context);
            }
            // Se a aba foi fechada, soltar o resultado encerra a árvore.
            let _ = sender.send(result);
            if let Some(context) = context {
                context.request_repaint();
            }
        })
        .ok();
    receiver
}



