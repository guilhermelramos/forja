//! Estado compartilhado do módulo: terminais, painéis, receitas e persistência.
//!
//! Tudo roda na thread da interface; o `Mutex` existe para que comandos, RPC e
//! a view compartilhem o mesmo hub sem depender do shell. Processos e leitura
//! de arquivos acontecem em threads próprias.

mod hub;
mod util;

use crate::links::Link;
use crate::panes::tree::{PaneId, PaneNode, TerminalId};
use crate::project::ProjectProfile;
use crate::recipes::{Recipe, RecipePicker};
use crate::settings::TermSettings;
use crate::shells;
use crate::terminal::Terminal;
use crate::theme::ThemeLibrary;
use forja_contracts::TerminalRef;
use forja_kernel::ui_contract::egui;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Pedido de abertura aceito por `terminal.open`.
#[derive(Clone, Debug, Default)]
pub struct OpenRequest {
    /// Pasta em que o processo começa.
    pub cwd: Option<PathBuf>,
    /// Programa que substitui o shell.
    pub command: Option<String>,
    /// Argumentos do programa.
    pub args: Option<String>,
    /// Variáveis extras.
    pub environment: BTreeMap<String, String>,
    /// Título inicial.
    pub title: Option<String>,
    /// Escolha de shell (`pwsh`, `cmd`, `git-bash`, `wsl`, `wsl:Ubuntu`).
    pub shell: Option<String>,
}

/// Evento que o hub entrega à view para publicação no barramento.
#[derive(Clone, Debug, PartialEq)]
pub struct HubEvent {
    /// Terminal que originou.
    pub terminal: TerminalRef,
    /// Tipo do evento.
    pub kind: HubEventKind,
    /// Código de saída, quando houver.
    pub exit_code: Option<i32>,
}

/// Tipo de evento do hub.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubEventKind {
    /// Campainha.
    Bell,
    /// Processo terminou.
    Exited,
    /// OSC 133;D.
    CommandFinished,
    /// Título mudou.
    TitleChanged,
}

/// Alvo de um fechamento pendente de confirmação.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseTarget {
    /// Painel inteiro.
    Pane(PaneId),
    /// Uma aba.
    Tab(PaneId, TerminalId),
}

/// Fechamento aguardando confirmação.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingClose {
    /// O que será fechado.
    pub target: CloseTarget,
    /// Quantos processos seguem vivos.
    pub running: usize,
}

/// Colagem aguardando confirmação por ter várias linhas.
#[derive(Clone, Debug)]
pub struct PendingPaste {
    /// Terminal de destino.
    pub terminal: TerminalId,
    /// Texto colado.
    pub text: String,
}

/// Terminal restaurado de uma execução anterior.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredTerminal {
    /// Id original.
    pub id: TerminalId,
    /// Pasta de trabalho.
    pub dir: PathBuf,
    /// Shell original.
    pub shell: String,
    /// Argumentos originais.
    pub args: String,
    /// Título personalizado.
    pub title: Option<String>,
    /// Texto do histórico (limitado).
    pub text: String,
}

/// Layout gravado em `%APPDATA%\Forja\modules\terminal.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredLayout {
    /// Árvore de painéis.
    pub root: Option<PaneNode>,
    /// Painel focado.
    pub focused: PaneId,
    /// Próximo id de terminal.
    pub next_terminal: TerminalId,
    /// Próximo id de painel.
    pub next_pane: PaneId,
    /// Conteúdo dos terminais.
    pub terminals: Vec<StoredTerminal>,
}

/// Hub protegido por mutex.
pub struct Hub {
    /// Terminais por id.
    pub terminals: BTreeMap<TerminalId, Terminal>,
    /// Active helpers reported by the helper module.
    pub helper_terminals: std::collections::BTreeSet<TerminalId>,
    /// Árvore de painéis.
    pub root: Option<PaneNode>,
    /// Painel focado.
    pub focused: PaneId,
    next_terminal: TerminalId,
    next_pane: PaneId,
    /// Bancada atual, quando outro módulo informa.
    pub bench_path: Option<PathBuf>,
    /// Pasta usada quando não há bancada.
    pub base_dir: PathBuf,
    /// Pasta do projeto.
    pub project_path: Option<PathBuf>,
    /// Receitas descobertas.
    pub recipes: Vec<Recipe>,
    /// Tipo do projeto da bancada, para ordenar o seletor de receitas.
    pub project_profile: ProjectProfile,
    /// Estado do seletor de receitas.
    pub recipe_picker: RecipePicker,
    /// Temas descobertos.
    pub themes: ThemeLibrary,
    content_loaded: bool,
    content_loaded_for: Option<PathBuf>,
    /// Temas e receitas vieram de `content.*` e não devem ser relidos do disco.
    contract_content: bool,
    /// Fechamento aguardando confirmação.
    pub pending_close: Option<PendingClose>,
    /// Receita aguardando confirmação (`terminal.recipes.confirm`).
    pub pending_recipe: Option<Recipe>,
    /// Receita aguardando decisão de confiança (`trust`).
    pub pending_trust_recipe: Option<Recipe>,
    /// Colagem aguardando confirmação.
    pub pending_paste: Option<PendingPaste>,
    /// Link aguardando confirmação, com o terminal de origem.
    pub pending_link: Option<(TerminalId, Link)>,
    /// Contexto egui da última view (copiar, OSC 52, repaint).
    pub ui_context: Option<egui::Context>,
    /// Configurações aplicadas por último.
    pub applied: Option<TermSettings>,
    /// Aviso curto na view (por exemplo, receita sem terminal).
    pub notice: Option<String>,
    /// Já abriu o terminal inicial.
    pub auto_open_done: bool,
    /// Leitura do layout anterior começou.
    pub restore_pending: bool,
    /// Conteúdo mudou desde a última gravação.
    dirty_since: Option<Instant>,
    structure_dirty: bool,
    preview: Option<String>,
    preview_done: bool,
}

pub struct Core {
    /// Hub de terminais.
    pub hub: Mutex<Hub>,
    /// Distros WSL descobertas em segundo plano.
    pub wsl: Arc<Mutex<Vec<String>>>,
}

impl Core {
    /// Cria o hub e inicia a descoberta de distros WSL.
    pub fn new() -> Self {
        let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let preview = std::env::var("FORJA_TERMINAL_PREVIEW").ok();
        let wsl = Arc::new(Mutex::new(Vec::new()));
        let slot = Arc::clone(&wsl);
        let _ = std::thread::Builder::new()
            .name("forja-terminal-wsl".to_owned())
            .spawn(move || {
                let distros = shells::wsl_distros();
                if let Ok(mut guard) = slot.lock() {
                    *guard = distros;
                }
            });
        Self {
            hub: Mutex::new(Hub::new(base, preview)),
            wsl,
        }
    }

    /// Distros conhecidas; nunca bloqueia por muito tempo.
    pub fn wsl_distros(&self) -> Vec<String> {
        self.wsl.lock().map(|guard| guard.clone()).unwrap_or_default()
    }

    /// Executa com o hub bloqueado.
    pub fn with_hub<T>(&self, run: impl FnOnce(&mut Hub) -> T) -> Result<T, String> {
        let mut hub = self.hub.lock().map_err(|_| "terminal state is poisoned".to_owned())?;
        Ok(run(&mut hub))
    }
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panes::tree::SplitDir;

    struct TempGuard(PathBuf);

    impl Drop for TempGuard {
        fn drop(&mut self) {
            for attempt in 0..20 {
                if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                    return;
                }
                if attempt < 19 {
                    std::thread::sleep(std::time::Duration::from_millis(15));
                }
            }
        }
    }

    fn temp_guard(label: &str) -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-terminal-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir)
    }

    #[test]
    fn recipe_preview_uses_the_bench_for_the_root_directory() {
        let guard = temp_guard("recipe-preview");
        let dir = &guard.0;
        let mut hub = Hub::new(dir.clone(), None);
        hub.bench_path = Some(dir.clone());
        let settings = TermSettings::default();
        let recipe = crate::recipes::Recipe {
            id: "node-test".to_owned(),
            title_pt: "Testes".to_owned(),
            title_en: "Tests".to_owned(),
            command: "npm test".to_owned(),
            scope: "project".to_owned(),
            enter: true,
            directory: ".".to_owned(),
            manifest: "package.json".to_owned(),
            source_path: String::new(),
            content: String::new(),
            content_hash: String::new(),
        };
        assert_eq!(hub.recipe_target_dir(&recipe, &settings), *dir);
    }

    #[test]
    fn pane_tree_operations_keep_ids_stable() {
        let guard = temp_guard("panes");
        let dir = &guard.0;
        let mut hub = Hub::new(dir.clone(), None);
        let settings = TermSettings::default();
        let first = hub.open(&settings, OpenRequest::default()).unwrap();
        assert_eq!(hub.visual_terminals(), vec![first]);
        let second = hub.new_tab(&settings, None).unwrap();
        assert_eq!(hub.visual_terminals(), vec![first, second]);
        let split = hub.split(SplitDir::Columns, &settings).unwrap();
        assert_eq!(hub.terminals.len(), 3);
        assert!(hub.visual_terminals().contains(&split));
        hub.focus_previous();
        hub.close_now(CloseTarget::Tab(0, first));
        assert_eq!(hub.terminals.len(), 2);
        hub.close_now(CloseTarget::Pane(hub.focused));
        assert!(!hub.terminals.is_empty() || hub.root.is_none());
        for terminal in hub.terminals.values_mut() {
            terminal.stop();
        }
    }

    #[test]
    fn helper_close_policy_is_independent_of_generic_terminal_confirmation() {
        let guard = temp_guard("helper-close");
        let dir = &guard.0;
        let mut hub = Hub::new(dir.clone(), None);
        let mut settings = TermSettings { close_confirm_running: false, ..TermSettings::default() };
        let id = hub.open(&settings, OpenRequest::default()).unwrap();
        assert!(hub.terminals.get(&id).unwrap().alive());
        hub.helper_terminals.insert(id);
        assert!(hub.request_close(CloseTarget::Tab(hub.focused, id), &settings));
        hub.cancel_close();
        settings.helper_close_confirm_running = false;
        assert!(!hub.request_close(CloseTarget::Tab(hub.focused, id), &settings));
        assert!(!hub.terminals.contains_key(&id));
    }

    #[test]
    fn persistence_roundtrips_layout_and_text() {
        let guard = temp_guard("persist");
        let dir = &guard.0;
        let mut hub = Hub::new(dir.clone(), None);
        let settings = TermSettings::default();
        let id = hub.open(&settings, OpenRequest::default()).unwrap();
        if let Some(terminal) = hub.terminal_mut(id) {
            terminal.surface.feed(b"restored_marker");
        }
        let snapshot = hub.snapshot();
        let json = serde_json::to_string(&snapshot).unwrap();
        let back: StoredLayout = serde_json::from_str(&json).unwrap();
        let mut restored = Hub::new(dir.clone(), None);
        assert!(restored.restore(back, &settings));
        assert!(restored
            .terminal(id)
            .is_some_and(|terminal| terminal.surface.text().contains("restored_marker")));
        for terminal in restored.terminals.values_mut() {
            terminal.stop();
        }
        for terminal in hub.terminals.values_mut() {
            terminal.stop();
        }
    }

    #[test]
    fn preview_seeds_visible_text() {
        let guard = temp_guard("preview");
        let dir = &guard.0;
        let mut hub = Hub::new(dir.clone(), Some("split".to_owned()));
        let settings = TermSettings::default();
        hub.seed_preview(&settings);
        let texts: Vec<String> = hub
            .terminals
            .values()
            .map(|terminal| terminal.surface.text())
            .collect();
        assert_eq!(texts.len(), 3, "{texts:?}");
        assert!(texts.iter().any(|text| text.contains("cargo build")), "{texts:?}");
        assert!(texts.iter().any(|text| text.contains("cargo test")), "{texts:?}");
        for terminal in hub.terminals.values_mut() {
            terminal.stop();
        }
    }

    #[test]
    fn list_filters_by_folder() {
        let guard = temp_guard("list");
        let other_guard = temp_guard("list-other");
        let dir = &guard.0;
        let other = &other_guard.0;
        let mut hub = Hub::new(dir.clone(), None);
        let settings = TermSettings::default();
        hub.open(
            &settings,
            OpenRequest {
                cwd: Some(dir.clone()),
                ..OpenRequest::default()
            },
        )
        .unwrap();
        hub.open(
            &settings,
            OpenRequest {
                cwd: Some(other.clone()),
                ..OpenRequest::default()
            },
        )
        .unwrap();
        assert_eq!(hub.list(Some(&dir.display().to_string())).len(), 1);
        assert_eq!(hub.list(None).len(), 2);
        for terminal in hub.terminals.values_mut() {
            terminal.stop();
        }
    }
}
