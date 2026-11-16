//! Operações do hub: terminais, painéis, eventos e fechamento.

use super::util::{absolute_dir, resolve_shell, same_path};
use super::{
    CloseTarget, Hub, HubEvent, HubEventKind, OpenRequest, PendingClose, StoredLayout,
    StoredTerminal,
};
use crate::panes::tree::{first_pane, PaneId, PaneNode, SplitDir, TerminalId, MAX_PANES};
use crate::project::ProjectProfile;
use crate::recipes::{self, Recipe, RecipePicker};
use crate::settings::TermSettings;
use crate::shells::{self, ShellCommand};
use crate::terminal::{Terminal, TerminalEvent};
use crate::theme::ThemeLibrary;
use forja_contracts::TerminalRef;
use forja_kernel::ui_contract::egui;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Espera entre a última saída e a gravação do layout com o texto das telas.
const PERSIST_DELAY: Duration = Duration::from_secs(10);

impl Hub {
    /// Cria um hub vazio com a pasta base atual.
    pub fn new(base_dir: PathBuf, preview: Option<String>) -> Self {
        Self {
            terminals: BTreeMap::new(),
            root: None,
            focused: 0,
            next_terminal: 0,
            next_pane: 0,
            bench_path: None,
            base_dir,
            project_path: None,
            recipes: Vec::new(),
            project_profile: ProjectProfile::default(),
            recipe_picker: RecipePicker::default(),
            themes: ThemeLibrary::load(None),
            content_loaded: false,
            content_loaded_for: None,
            contract_content: false,
            pending_close: None,
            pending_recipe: None,
            pending_trust_recipe: None,
            helper_terminals: std::collections::BTreeSet::new(),
            pending_paste: None,
            pending_link: None,
            ui_context: None,
            applied: None,
            notice: None,
            auto_open_done: false,
            restore_pending: false,
            dirty_since: None,
            structure_dirty: false,
            preview,
            preview_done: false,
        }
    }

    /// Atualiza a bancada e recarrega conteúdo do projeto quando muda.
    pub fn set_bench(&mut self, bench: Option<PathBuf>) {
        if self.bench_path == bench {
            return;
        }
        self.bench_path = bench;
        self.project_profile = ProjectProfile::default();
        self.reload_content();
    }

    /// Abre o seletor de receitas com o filtro limpo.
    pub fn open_recipe_picker(&mut self) {
        self.recipe_picker = RecipePicker {
            open: true,
            query: String::new(),
            selected: 0,
            others_open: false,
            focus_query: true,
        };
    }

    /// Quantidade de painéis visíveis na bancada.
    pub fn pane_count(&self) -> usize {
        self.root.as_ref().map_or(0, PaneNode::leaf_count)
    }

    /// Pasta onde novos terminais começam.
    pub fn working_directory(&self, settings: &TermSettings) -> PathBuf {
        shells::start_directory(
            &settings.start_dir,
            self.bench_path.as_deref(),
            self.project_path.as_deref(),
        )
    }

    /// Carrega temas e receitas da primeira vez que a view desenha.
    pub fn ensure_content(&mut self) {
        if !self.content_loaded {
            self.reload_content();
        }
    }

    fn reload_content(&mut self) {
        let bench = self.bench_path.clone();
        if self.content_loaded && self.content_loaded_for == bench {
            return;
        }
        self.content_loaded = true;
        self.content_loaded_for = bench.clone();
        if self.contract_content {
            return;
        }
        self.themes = ThemeLibrary::load(bench.as_deref());
        self.recipes = recipes::load(bench.as_deref());
    }

    /// Aplica temas e receitas entregues pelo módulo `content`.
    pub fn set_contract_content(&mut self, themes: Vec<String>, recipes: Vec<Recipe>) {
        self.contract_content = true;
        self.content_loaded = true;
        self.themes = ThemeLibrary::from_texts(&themes);
        self.recipes = recipes;
    }

    /// Abre um terminal na aba focada ou cria o primeiro painel.
    pub fn open(&mut self, settings: &TermSettings, request: OpenRequest) -> Result<TerminalId, String> {
        let dir = absolute_dir(
            &request
                .cwd
                .clone()
                .unwrap_or_else(|| self.working_directory(settings)),
        );
        if !dir.is_dir() {
            return Err(format!("folder does not exist: {}", dir.display()));
        }
        let shell = resolve_shell(settings, request.shell.as_deref(), request.command.as_deref(), request.args.as_deref());
        let mut environment = shells::environment(&settings.environment);
        environment.extend(request.environment);
        let id = self.next_terminal;
        self.next_terminal += 1;
        let terminal = Terminal::start(
            id,
            dir,
            shell,
            environment,
            settings,
            request.title,
            None,
            self.ui_context.clone(),
        );
        self.terminals.insert(id, terminal);
        let mut attached = false;
        if let Some(root) = &mut self.root {
            if let Some(PaneNode::Leaf { tabs, active, .. }) = root.leaf_mut(self.focused) {
                tabs.push(id);
                *active = tabs.len() - 1;
                attached = true;
            }
        }
        if !attached {
            self.install_first_pane(id);
        }
        self.structure_dirty = true;
        Ok(id)
    }

    fn install_first_pane(&mut self, terminal: TerminalId) {
        let pane = self.next_pane;
        self.next_pane += 1;
        self.root = Some(PaneNode::leaf(pane, terminal));
        self.focused = pane;
    }

    /// Abre o terminal padrão quando ainda não há nenhum.
    pub fn ensure_open(&mut self, settings: &TermSettings) {
        if !self.terminals.is_empty() || self.restore_pending || self.auto_open_done {
            return;
        }
        self.auto_open_done = true;
        let _ = self.open(settings, OpenRequest::default());
    }

    /// Divide o painel focado e abre um terminal novo.
    pub fn split(
        &mut self,
        direction: SplitDir,
        settings: &TermSettings,
    ) -> Result<TerminalId, String> {
        if self.pane_count() >= MAX_PANES {
            return Err("terminal pane limit reached".to_owned());
        }
        let dir = self
            .focused_terminal()
            .map(|terminal| terminal.dir.clone())
            .unwrap_or_else(|| self.working_directory(settings));
        let shell = resolve_shell(settings, None, None, None);
        let id = self.next_terminal;
        self.next_terminal += 1;
        let environment = shells::environment(&settings.environment);
        let terminal = Terminal::start(
            id,
            dir,
            shell,
            environment,
            settings,
            None,
            None,
            self.ui_context.clone(),
        );
        self.terminals.insert(id, terminal);
        if self.root.is_none() {
            self.install_first_pane(id);
            return Ok(id);
        }
        let pane = self.next_pane;
        self.next_pane += 1;
        if let Some(root) = &mut self.root {
            if let Some(leaf) = root.leaf_mut(self.focused) {
                leaf.split(direction, pane, id);
                self.focused = pane;
            }
        }
        self.structure_dirty = true;
        Ok(id)
    }

    /// Abre uma aba no painel focado.
    pub fn new_tab(
        &mut self,
        settings: &TermSettings,
        shell: Option<&str>,
    ) -> Result<TerminalId, String> {
        let cwd = self.focused_terminal().map(|terminal| terminal.dir.clone());
        self.open(
            settings,
            OpenRequest {
                cwd,
                shell: shell.map(str::to_owned),
                ..OpenRequest::default()
            },
        )
    }

    /// Painel e aba focados.
    pub fn active(&self) -> Option<(PaneId, TerminalId)> {
        match self.root.as_ref()?.leaf_ref(self.focused)? {
            PaneNode::Leaf { pane, tabs, active } => {
                Some((*pane, *tabs.get(*active)?))
            }
            PaneNode::Split { .. } => None,
        }
    }

    /// Foca uma aba específica, em qualquer painel.
    pub fn focus_terminal(&mut self, id: TerminalId) -> bool {
        let Some(root) = &self.root else {
            return false;
        };
        let mut leaves = Vec::new();
        root.leaves(&mut leaves);
        for pane in leaves {
            let Some(PaneNode::Leaf { tabs, active, .. }) =
                self.root.as_mut().and_then(|root| root.leaf_mut(pane))
            else {
                continue;
            };
            if let Some(index) = tabs.iter().position(|candidate| *candidate == id) {
                *active = index;
                self.focused = pane;
                if let Some(terminal) = self.terminals.get_mut(&id) {
                    terminal.focus_request = true;
                }
                return true;
            }
        }
        false
    }

    /// Terminal focado.
    pub fn focused_terminal(&self) -> Option<&Terminal> {
        match self.root.as_ref()?.leaf_ref(self.focused)? {
            PaneNode::Leaf { tabs, active, .. } => {
                self.terminals.get(tabs.get(*active)?)
            }
            PaneNode::Split { .. } => None,
        }
    }

    /// Terminal focado, mutável.
    pub fn focused_terminal_mut(&mut self) -> Option<&mut Terminal> {
        let id = match self.root.as_ref()?.leaf_ref(self.focused)? {
            PaneNode::Leaf { tabs, active, .. } => tabs.get(*active).copied()?,
            PaneNode::Split { .. } => return None,
        };
        self.terminals.get_mut(&id)
    }

    /// Terminal pelo id.
    pub fn terminal_mut(&mut self, id: TerminalId) -> Option<&mut Terminal> {
        self.terminals.get_mut(&id)
    }

    /// Terminal pelo id.
    pub fn terminal(&self, id: TerminalId) -> Option<&Terminal> {
        self.terminals.get(&id)
    }

    /// Lista filtrada por pasta de bancada.
    pub fn list(&self, bench: Option<&str>) -> Vec<TerminalRef> {
        let wanted = bench.map(|bench| absolute_dir(Path::new(bench)));
        self.terminals
            .values()
            .filter(|terminal| {
                wanted
                    .as_ref()
                    .is_none_or(|wanted| same_path(&terminal.dir, wanted))
            })
            .map(|terminal| TerminalRef {
                terminal_id: terminal.id.to_string(),
                bench_path: terminal.dir.display().to_string(),
                title: terminal.title(),
            })
            .collect()
    }

    /// Terminais em ordem de visualização.
    pub fn visual_terminals(&self) -> Vec<TerminalId> {
        let mut ids = Vec::new();
        if let Some(root) = &self.root {
            root.terminals(&mut ids);
        }
        ids
    }

    /// Próximo painel.
    pub fn focus_next(&mut self) {
        self.focus_step(1);
    }

    /// Painel anterior.
    pub fn focus_previous(&mut self) {
        self.focus_step(-1);
    }

    fn focus_step(&mut self, step: isize) {
        let mut leaves = Vec::new();
        if let Some(root) = &self.root {
            root.leaves(&mut leaves);
        }
        if leaves.is_empty() {
            return;
        }
        let index = leaves
            .iter()
            .position(|pane| *pane == self.focused)
            .unwrap_or(0) as isize;
        let next = (index + step).rem_euclid(leaves.len() as isize) as usize;
        self.focused = leaves[next];
        if let Some(terminal) = self.focused_terminal_mut() {
            terminal.focus_request = true;
        }
    }

    /// Pede o fechamento de um alvo.
    pub fn request_close(&mut self, target: CloseTarget, settings: &TermSettings) -> bool {
        let running = self.running_in(target);
        let helper_running = self.target_ids(target).iter().any(|id| {
            self.helper_terminals.contains(id) && self.terminals.get(id).is_some_and(|terminal| terminal.alive())
        });
        if (settings.close_confirm_running && running > 0)
            || (settings.helper_close_confirm_running && helper_running)
        {
            self.pending_close = Some(PendingClose { target, running });
            return true;
        }
        self.close_now(target);
        false
    }

    fn running_in(&self, target: CloseTarget) -> usize {
        self.target_ids(target).iter()
            .filter_map(|id| self.terminals.get(id))
            .filter(|terminal| terminal.alive())
            .count()
    }

    fn target_ids(&self, target: CloseTarget) -> Vec<TerminalId> {
        match target {
            CloseTarget::Pane(pane) => self
                .root
                .as_ref()
                .and_then(|root| root.leaf_ref(pane))
                .map(|leaf| match leaf {
                    PaneNode::Leaf { tabs, .. } => tabs.clone(),
                    _ => Vec::new(),
                })
                .unwrap_or_default(),
            CloseTarget::Tab(_, terminal) => vec![terminal],
        }
    }

    /// Executa o fechamento pendente.
    pub fn confirm_close(&mut self) {
        if let Some(pending) = self.pending_close.take() {
            self.close_now(pending.target);
        }
    }

    /// Cancela o fechamento pendente.
    pub fn cancel_close(&mut self) {
        self.pending_close = None;
    }

    /// Fecha agora, sem perguntar.
    pub fn close_now(&mut self, target: CloseTarget) {
        match target {
            CloseTarget::Pane(pane) => {
                let mut ids = Vec::new();
                if let Some(PaneNode::Leaf { tabs, .. }) =
                    self.root.as_mut().and_then(|root| root.leaf_mut(pane))
                {
                    ids = tabs.clone();
                }
                for id in ids {
                    self.terminals.remove(&id);
                }
                self.root = self.root.take().and_then(|root| root.remove(pane));
                self.focused = self
                    .root
                    .as_ref()
                    .map(first_pane)
                    .unwrap_or(0);
                if let Some(terminal) = self.focused_terminal_mut() {
                    terminal.focus_request = true;
                }
            }
            CloseTarget::Tab(pane, terminal) => {
                self.terminals.remove(&terminal);
                let mut empty = false;
                if let Some(PaneNode::Leaf { tabs, active, .. }) =
                    self.root.as_mut().and_then(|root| root.leaf_mut(pane))
                {
                    if let Some(index) = tabs.iter().position(|id| *id == terminal) {
                        tabs.remove(index);
                        if index < *active {
                            *active -= 1;
                        }
                        *active = (*active).min(tabs.len().saturating_sub(1));
                        empty = tabs.is_empty();
                    }
                }
                if empty {
                    self.root = self.root.take().and_then(|root| root.remove(pane));
                    self.focused = self.root.as_ref().map(first_pane).unwrap_or(0);
                }
            }
        }
        self.structure_dirty = true;
        self.dirty_since = Some(Instant::now());
    }

    /// Lê a saída e devolve eventos para a view publicar.
    pub fn poll(
        &mut self,
        now: Instant,
        settings: &TermSettings,
        context: Option<&egui::Context>,
    ) -> Vec<HubEvent> {
        if self.applied.as_ref() != Some(settings) {
            for terminal in self.terminals.values_mut() {
                terminal.apply_settings(settings);
            }
            self.applied = Some(settings.clone());
        }
        let mut events = Vec::new();
        for terminal in self.terminals.values_mut() {
            let reference = TerminalRef {
                terminal_id: terminal.id.to_string(),
                bench_path: terminal.dir.display().to_string(),
                title: terminal.title(),
            };
            for event in terminal.poll(now, settings, context) {
                let (kind, exit_code) = match event {
                    TerminalEvent::Bell => (HubEventKind::Bell, None),
                    TerminalEvent::Exited(code) => (HubEventKind::Exited, code),
                    TerminalEvent::CommandFinished(code) => {
                        (HubEventKind::CommandFinished, code)
                    }
                    TerminalEvent::TitleChanged => (HubEventKind::TitleChanged, None),
                };
                events.push(HubEvent {
                    terminal: reference.clone(),
                    kind,
                    exit_code,
                });
            }
            if terminal.take_dirty() {
                self.dirty_since.get_or_insert(now);
            }
        }
        self.search_refresh();
        events
    }

    fn search_refresh(&mut self) {
        for terminal in self.terminals.values_mut() {
            terminal.refresh_search();
        }
    }

    /// Próximo quadro que o hub precisa sem novidade de fora: o menor pedido
    /// dos terminais e a gravação adiada do layout. `None` = nenhum.
    pub fn next_wake(&self, now: Instant) -> Option<Duration> {
        let persist = self.dirty_since.map(|since| {
            (since + PERSIST_DELAY).saturating_duration_since(now)
        });
        self.terminals
            .values()
            .filter_map(|terminal| terminal.next_wake(now))
            .chain(persist)
            .min()
    }

    /// Prepara a gravação do layout quando há novidades.
    pub fn persist_if_due(&mut self, now: Instant, force: bool) -> Option<StoredLayout> {
        let due = self.structure_dirty
            || force
            || self
                .dirty_since
                .is_some_and(|since| now.duration_since(since) >= PERSIST_DELAY);
        if !due {
            return None;
        }
        self.dirty_since = None;
        self.structure_dirty = false;
        Some(self.snapshot())
    }

    /// Layout atual.
    pub fn snapshot(&self) -> StoredLayout {
        StoredLayout {
            root: self.root.clone(),
            focused: self.focused,
            next_terminal: self.next_terminal,
            next_pane: self.next_pane,
            terminals: self
                .terminals
                .values()
                .map(|terminal| StoredTerminal {
                    id: terminal.id,
                    dir: terminal.dir.clone(),
                    shell: terminal.shell.clone(),
                    args: terminal.args.clone(),
                    title: terminal.custom_title.clone(),
                    text: terminal.snapshot_text(),
                })
                .collect(),
        }
    }

    /// Aplica um layout lido em segundo plano.
    pub fn restore(&mut self, layout: StoredLayout, settings: &TermSettings) -> bool {
        if !self.terminals.is_empty() {
            return false;
        }
        let stored: BTreeMap<TerminalId, StoredTerminal> = layout
            .terminals
            .into_iter()
            .map(|terminal| (terminal.id, terminal))
            .collect();
        let Some(root) = layout.root else {
            return false;
        };
        // Uma bancada gravada pode ser anterior ao limite de painéis. Fica com
        // os primeiros para todo quadro continuar limitado. Refs #72, #74.
        let root = root.pruned(MAX_PANES);
        let mut referenced = Vec::new();
        root.terminals(&mut referenced);
        if referenced.iter().any(|id| !stored.contains_key(id)) {
            return false;
        }
        for id in referenced {
            let Some(entry) = stored.get(&id) else {
                continue;
            };
            let dir = if entry.dir.is_dir() {
                absolute_dir(&entry.dir)
            } else {
                self.base_dir.clone()
            };
            // Reopening Forja never relaunches a helper (claude, codex, …) by
            // itself: a saved helper terminal comes back as the default shell,
            // keeping its text. Plain shells reopen as they were. Refs #167.
            let shell = if is_plain_shell(&entry.shell) {
                ShellCommand {
                    program: entry.shell.clone(),
                    arguments: entry.args.clone(),
                }
            } else {
                resolve_shell(settings, None, None, None)
            };
            let terminal = Terminal::start(
                id,
                dir,
                shell,
                std::collections::BTreeMap::new(),
                settings,
                entry.title.clone(),
                Some(entry.text.clone()),
                self.ui_context.clone(),
            );
            self.terminals.insert(id, terminal);
        }
        let focused = if root.leaf_ref(layout.focused).is_some() {
            layout.focused
        } else {
            let mut leaves = Vec::new();
            root.leaves(&mut leaves);
            leaves.first().copied().unwrap_or(0)
        };
        self.root = Some(root);
        self.focused = focused;
        self.next_terminal = layout.next_terminal.max(self.next_terminal);
        self.next_pane = layout.next_pane.max(self.next_pane);
        self.auto_open_done = true;
        self.structure_dirty = true;
        true
    }

    /// Semeia conteúdo de demonstração apenas para capturas de tela.
    pub fn seed_preview(&mut self, settings: &TermSettings) {
        let Some(kind) = self.preview.clone() else {
            return;
        };
        if self.preview_done {
            return;
        }
        self.preview_done = true;
        self.auto_open_done = true;
        let _ = self.open(settings, OpenRequest::default());
        match kind.as_str() {
            "demo" => {
                if let Some(terminal) = self.focused_terminal_mut() {
                    terminal.freeze_preview();
                    terminal.surface.feed(
                        b"PS C:\\forja> cargo build --workspace\r\n   Compiling forja-kernel v0.1.0\r\n\x1b[32m    Finished\x1b[0m `dev` profile [unoptimized + debuginfo] target(s) in 4.21s\r\nPS C:\\forja> forja terminal list\r\n{\"terminals\":[{\"terminal_id\":\"0\",\"title\":\"pwsh\"}]}\r\nPS C:\\forja> ",
                    );
                }
            }
            "split" => {
                let _ = self.split(SplitDir::Rows, settings);
                let _ = self.split(SplitDir::Columns, settings);
                let ids = self.visual_terminals();
                let banners = [
                    "PS C:\\forja> cargo build --workspace\r\n   Compiling forja-kernel v0.1.0\r\n\x1b[32m    Finished\x1b[0m dev profile in 4.21s\r\nPS C:\\forja> ",
                    "PS C:\\forja> cargo test --workspace\r\nrunning 128 tests\r\ntest result: ok. 128 passed\r\nPS C:\\forja> ",
                    "PS C:\\forja> forja terminal list\r\n{\"terminals\":[{\"terminal_id\":\"0\"}]}\r\nPS C:\\forja> ",
                ];
                for (terminal, banner) in ids.iter().zip(banners) {
                    if let Some(terminal) = self.terminals.get_mut(terminal) {
                        terminal.freeze_preview();
                        terminal.surface.feed(banner.as_bytes());
                    }
                }
            }
            "search" => {
                if let Some(terminal) = self.focused_terminal_mut() {
                    terminal.freeze_preview();
                    terminal.surface.feed(
                        b"PS C:\\forja> forja terminal open --cwd .\r\nterminal opened: forja\r\nPS C:\\forja> forja terminal send --text \"echo forja\"\r\nforja\r\nPS C:\\forja> ",
                    );
                    terminal.open_search("forja".to_owned());
                    terminal.refresh_search();
                }
            }
            _ => {}
        }
    }


    /// Pasta em que a receita roda, para a prévia do seletor.
    ///
    /// Repete a escolha de `run_recipe`: pasta própria da receita, pasta da
    /// aba focada ou a bancada. Com um shell de receita, a aba nova começa na
    /// bancada.
    pub fn recipe_target_dir(&self, recipe: &Recipe, settings: &TermSettings) -> PathBuf {
        if let Some(target) = self.recipe_dir(recipe) {
            if recipe.directory.trim() == "." {
                return self.bench_path.clone().unwrap_or(target);
            }
            return target;
        }
        if settings.recipe_program().is_some() {
            return self
                .bench_path
                .clone()
                .unwrap_or_else(|| self.base_dir.clone());
        }
        self.focused_terminal()
            .filter(|terminal| {
                terminal.alive() && !self.helper_terminals.contains(&terminal.id)
            })
            .map(|terminal| terminal.dir.clone())
            .or_else(|| self.bench_path.clone())
            .unwrap_or_else(|| self.base_dir.clone())
    }

    /// Executa uma receita. Sem `terminal.recipes.shell`, roda no terminal focado;
    /// com um shell escolhido, abre uma aba nova com ele e roda ali.
    pub fn run_recipe(&mut self, recipe: &Recipe, settings: &TermSettings) -> Result<(), String> {
        let enter = recipe.enter && settings.recipe_press_enter;
        let target = self.recipe_dir(recipe);
        if let Some(program) = settings.recipe_program() {
            self.open(
                settings,
                OpenRequest {
                    cwd: target.or_else(|| self.bench_path.clone()),
                    command: Some(program.to_owned()),
                    title: Some(recipe.title_en.clone()),
                    ..OpenRequest::default()
                },
            )?;
        } else if let Some(target) = target {
            if self.focused_terminal().is_none_or(|terminal| {
                !same_path(&terminal.dir, &target)
                    || !terminal.alive()
                    || self.helper_terminals.contains(&terminal.id)
            }) {
                self.open(
                    settings,
                    OpenRequest {
                        cwd: Some(target),
                        title: Some(recipe.title_en.clone()),
                        ..OpenRequest::default()
                    },
                )?;
            }
        } else if self.focused_terminal().is_none_or(|terminal| {
            !terminal.alive() || self.helper_terminals.contains(&terminal.id)
        }) {
            self.open(
                settings,
                OpenRequest {
                    cwd: self.bench_path.clone(),
                    title: Some(recipe.title_en.clone()),
                    ..OpenRequest::default()
                },
            )?;
        }
        let terminal = self
            .focused_terminal_mut()
            .ok_or_else(|| "no terminal is open".to_owned())?;
        terminal.focus_request = true;
        terminal.send(&recipe.command, enter);
        Ok(())
    }

    /// Pasta da receita: `directory` dentro da bancada, se existir.
    fn recipe_dir(&self, recipe: &Recipe) -> Option<PathBuf> {
        if recipe.directory.trim().is_empty() {
            return None;
        }
        let target = self.bench_path.as_ref()?.join(&recipe.directory);
        target.is_dir().then_some(target)
    }
}

/// Interactive shells that are safe to reopen. Anything else (a helper CLI
/// such as `claude`, `codex` or `opencode`) is not relaunched on restore.
fn is_plain_shell(program: &str) -> bool {
    let name = std::path::Path::new(program)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(program)
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "pwsh" | "powershell" | "cmd" | "bash" | "sh" | "zsh" | "fish" | "nu" | "wsl" | "git-bash"
    )
}

#[cfg(test)]
mod restore_shell_tests {
    #[test]
    fn helpers_are_not_plain_shells() {
        assert!(super::is_plain_shell("pwsh"));
        assert!(super::is_plain_shell(r"C:\Windows\System32\cmd.exe"));
        assert!(!super::is_plain_shell("claude"));
        assert!(!super::is_plain_shell("codex"));
        assert!(!super::is_plain_shell("opencode"));
    }
}

