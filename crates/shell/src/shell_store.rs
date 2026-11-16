//! Persistent `"shell"` store data (`anvil_history` + per-bench `layout`) and async atomic writer.

use crate::app::ForjaApp;
use crate::tabs::TabSet;
use forja_kernel::settings::SettingValue;
use forja_kernel::store::write_atomic_bytes;
use forja_kernel::{Args, Runtime};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// Per-bench workspace chrome layout saved under `"layout.<bench_id>"` in the `"shell"` store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BenchLayout {
    pub active_tab: Option<String>,
    pub rail_page: Option<String>,
    pub closed_tabs: Vec<String>,
    pub sidebar_tab: Option<String>,
    pub right_tab: Option<String>,
    pub sidebar_open: bool,
    pub right_panel_open: bool,
}

impl Default for BenchLayout {
    fn default() -> Self {
        Self {
            active_tab: None,
            rail_page: None,
            closed_tabs: Vec::new(),
            sidebar_tab: None,
            right_tab: None,
            sidebar_open: true,
            right_panel_open: true,
        }
    }
}

impl BenchLayout {
    pub fn from_json(value: &Value, default_sidebar_open: bool) -> Self {
        let str_opt = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        let closed_tabs = value
            .get("closed_tabs")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            active_tab: str_opt("active_tab"),
            rail_page: str_opt("rail_page"),
            closed_tabs,
            sidebar_tab: str_opt("sidebar_tab"),
            right_tab: str_opt("right_tab"),
            sidebar_open: value
                .get("sidebar_open")
                .and_then(Value::as_bool)
                .unwrap_or(default_sidebar_open),
            right_panel_open: value
                .get("right_panel_open")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "active_tab": self.active_tab,
            "rail_page": self.rail_page,
            "closed_tabs": self.closed_tabs,
            "sidebar_tab": self.sidebar_tab,
            "right_tab": self.right_tab,
            "sidebar_open": self.sidebar_open,
            "right_panel_open": self.right_panel_open,
        })
    }

    /// Resolves persisted view IDs into current runtime slot indices.
    pub fn resolve(
        &self,
        bench_views: &[&str],
        rail_views: &[&str],
        sidebar_views: &[&str],
        right_views: &[&str],
    ) -> ResolvedLayout {
        let closed_indexes: Vec<usize> = self
            .closed_tabs
            .iter()
            .filter_map(|id| bench_views.iter().position(|known| known == id))
            .collect();
        let tabs = TabSet::from_closed(closed_indexes, bench_views.len());
        let active_tab = self
            .active_tab
            .as_ref()
            .and_then(|id| bench_views.iter().position(|known| known == id))
            .filter(|&idx| tabs.is_visible(idx, bench_views.len()))
            .unwrap_or_else(|| tabs.clamp(0, bench_views.len()).unwrap_or(0));
        let rail_page = self
            .rail_page
            .as_ref()
            .filter(|id| rail_views.iter().any(|known| known == &id.as_str()))
            .cloned();
        let sidebar_tab = self
            .sidebar_tab
            .as_ref()
            .and_then(|id| sidebar_views.iter().position(|known| known == id))
            .unwrap_or(0);
        let right_tab = self
            .right_tab
            .as_ref()
            .and_then(|id| right_views.iter().position(|known| known == id))
            .unwrap_or(0);
        ResolvedLayout {
            active_tab,
            rail_page,
            tabs,
            sidebar_tab,
            right_tab,
            sidebar_open: self.sidebar_open,
            right_panel_open: self.right_panel_open,
        }
    }
}

/// Runtime-resolved layout state ready to assign onto [`crate::app::ForjaApp`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedLayout {
    pub active_tab: usize,
    pub rail_page: Option<String>,
    pub tabs: TabSet,
    pub sidebar_tab: usize,
    pub right_tab: usize,
    pub sidebar_open: bool,
    pub right_panel_open: bool,
}

/// CLI / env startup flags that override any restored layout when provided.
#[derive(Clone, Debug, Default)]
pub struct CliOverrides<'a> {
    pub open: Option<&'a str>,
    pub rail: Option<&'a str>,
    pub bench_tab: Option<&'a str>,
    pub right_tab: Option<&'a str>,
    pub env_right_tab: Option<&'a str>,
}

impl ResolvedLayout {
    /// Applies explicit CLI or environment flags over the restored bench layout.
    pub fn apply_cli_overrides(
        &mut self,
        cli: &CliOverrides<'_>,
        bench_views: &[&str],
        rail_views: &[&str],
        right_views: &[&str],
    ) {
        let open_rail = cli.open == Some("rail");
        if open_rail {
            self.rail_page = cli
                .rail
                .filter(|id| rail_views.contains(id))
                .map(str::to_owned)
                .or_else(|| rail_views.first().map(|id| (*id).to_owned()));
        } else if let Some(rail_id) = cli.rail {
            if rail_views.contains(&rail_id) {
                self.rail_page = Some(rail_id.to_owned());
            }
        }

        if let Some(tab_id) = cli.bench_tab {
            if let Some(index) = bench_views.iter().position(|known| *known == tab_id) {
                self.tabs.show(index, bench_views.len());
                self.active_tab = index;
                if cli.rail.is_none() && !open_rail {
                    self.rail_page = None;
                }
            }
        }

        let forced_right = cli
            .right_tab
            .and_then(|id| right_views.iter().position(|known| *known == id))
            .or_else(|| {
                cli.env_right_tab
                    .and_then(|needle| right_views.iter().position(|known| known.contains(needle)))
            });
        if let Some(index) = forced_right {
            self.right_tab = index;
            self.right_panel_open = true;
        }
    }
}

/// Full `"shell"` store payload, combining Bigorna command history and per-bench layouts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShellData {
    pub anvil_history: Vec<String>,
    pub layout: BTreeMap<String, BenchLayout>,
}

impl ShellData {
    pub fn from_json(value: &Value, default_sidebar_open: bool) -> Self {
        let anvil_history = crate::anvil::load_history(value);
        let layout = value
            .get("layout")
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .filter(|(k, _)| !k.trim().is_empty())
                    .map(|(bench_id, entry)| {
                        (
                            bench_id.clone(),
                            BenchLayout::from_json(entry, default_sidebar_open),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            anvil_history,
            layout,
        }
    }

    pub fn to_json(&self) -> Value {
        let mut layout_map = Map::new();
        for (bench_id, entry) in &self.layout {
            layout_map.insert(bench_id.clone(), entry.to_json());
        }
        json!({
            "anvil_history": self.anvil_history,
            "layout": Value::Object(layout_map),
        })
    }
}

#[derive(Default)]
struct WriterState {
    busy: bool,
    pending: Option<ShellData>,
}

/// Writes `"shell"` store updates off the UI thread via `Runtime::tasks()`.
#[derive(Default)]
pub struct ShellWriter {
    state: Arc<Mutex<WriterState>>,
    last: Option<ShellData>,
}

impl ShellWriter {
    pub fn seed(&mut self, data: ShellData) {
        self.last = Some(data);
    }

    pub fn busy(&self) -> bool {
        self.state.lock().map(|state| state.busy).unwrap_or(false)
    }

    pub fn save(&mut self, runtime: &mut Runtime, data: &ShellData) {
        if self.last.as_ref() == Some(data) {
            return;
        }
        self.last = Some(data.clone());
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.pending = Some(data.clone());
        if state.busy {
            return;
        }
        state.busy = true;
        drop(state);

        let state = Arc::clone(&self.state);
        let path = runtime.store().path_for("shell");
        runtime.ctx("shell").spawn(move || loop {
            let next = match state.lock() {
                Ok(mut state) => match state.pending.take() {
                    Some(next) => next,
                    None => {
                        state.busy = false;
                        break;
                    }
                },
                Err(_) => break,
            };
            if let Ok(bytes) = serde_json::to_vec_pretty(&next.to_json()) {
                let _ = write_atomic_bytes(&path, &bytes);
            }
        }, |_, _| {});
    }
}

impl ForjaApp {
    pub(crate) fn runtime_setting_bool(runtime: &Runtime, key: &str, default: bool) -> bool {
        runtime
            .settings()
            .values()
            .get(key)
            .and_then(SettingValue::as_bool)
            .unwrap_or(default)
    }

    pub(crate) fn query_current_bench_id(runtime: &mut Runtime) -> Option<String> {
        runtime.registry().command("bench.current")?;
        let value = runtime.run_command("bench.current", Args::new()).ok()?;
        value
            .get("bench_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
    }

    pub(crate) fn default_bench_layout(&self) -> BenchLayout {
        BenchLayout {
            sidebar_open: self.last_sidebar_setting,
            ..Default::default()
        }
    }

    pub(crate) fn capture_bench_layout(&self) -> BenchLayout {
        let closed_tabs = self
            .tabs
            .closed_indexes()
            .iter()
            .filter_map(|&index| self.bench_views.get(index).map(|view| view.id.clone()))
            .collect();
        BenchLayout {
            active_tab: self.bench_views.get(self.active_tab).map(|view| view.id.clone()),
            rail_page: self.rail_page.clone(),
            closed_tabs,
            sidebar_tab: self
                .sidebar_views
                .get(self.sidebar_tab)
                .map(|view| view.id.clone()),
            right_tab: self
                .right_views
                .get(self.right_tab)
                .map(|view| view.id.clone()),
            sidebar_open: self.show_sidebar,
            right_panel_open: self.show_right_panel,
        }
    }

    pub(crate) fn resolve_layout(&self, layout: &BenchLayout) -> ResolvedLayout {
        let bench_ids: Vec<&str> = self.bench_views.iter().map(|view| view.id.as_str()).collect();
        let rail_ids: Vec<&str> = self.rail_views.iter().map(|view| view.id.as_str()).collect();
        let sidebar_ids: Vec<&str> = self
            .sidebar_views
            .iter()
            .map(|view| view.id.as_str())
            .collect();
        let right_ids: Vec<&str> = self.right_views.iter().map(|view| view.id.as_str()).collect();
        layout.resolve(&bench_ids, &rail_ids, &sidebar_ids, &right_ids)
    }

    pub(crate) fn apply_resolved_layout(&mut self, resolved: ResolvedLayout) {
        self.active_tab = resolved.active_tab;
        self.rail_page = resolved.rail_page;
        self.tabs = resolved.tabs;
        self.sidebar_tab = resolved.sidebar_tab;
        self.right_tab = resolved.right_tab;
        self.show_sidebar = resolved.sidebar_open;
        self.show_right_panel = resolved.right_panel_open;
    }

    pub(crate) fn switch_bench_layout(&mut self, next_bench_id: Option<String>, keep_page: bool) {
        if let Some(prev_id) = self.current_bench_id.take() {
            let layout = self.capture_bench_layout();
            self.shell_data.layout.insert(prev_id, layout);
        }
        let preserved_rail = keep_page.then(|| self.rail_page.clone()).flatten();
        let next_layout = next_bench_id
            .as_deref()
            .and_then(|id| self.shell_data.layout.get(id))
            .cloned()
            .unwrap_or_else(|| self.default_bench_layout());
        let mut resolved = self.resolve_layout(&next_layout);
        if keep_page {
            resolved.rail_page = preserved_rail;
        }
        self.current_bench_id = next_bench_id;
        self.apply_resolved_layout(resolved);
    }

    pub(crate) fn save_shell_data(&mut self) {
        if self.screenshot.is_some() {
            return;
        }
        if let Some(bench_id) = self.current_bench_id.as_deref() {
            let layout = self.capture_bench_layout();
            self.shell_data.layout.insert(bench_id.to_owned(), layout);
        }
        self.shell_writer.save(&mut self.runtime, &self.shell_data);
    }

    pub(crate) fn sync_shell_store(&mut self) {
        if self.screenshot.is_some() || self.current_bench_id.is_none() {
            return;
        }
        self.save_shell_data();
    }

    pub(crate) fn sync_sidebar_setting(&mut self) {
        let setting = self.setting_bool("appearance.sidebar.visible", true);
        if setting != self.last_sidebar_setting {
            self.last_sidebar_setting = setting;
            self.show_sidebar = setting;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{store::LoadResult, Runtime, RuntimeOptions};
    use std::path::PathBuf;

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "forja-shell-store-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn legacy_anvil_history_loads_without_layout() {
        let value = json!({ "anvil_history": ["command:anvil.open", "view:git.tab"] });
        let data = ShellData::from_json(&value, true);
        assert_eq!(
            data.anvil_history,
            vec!["command:anvil.open".to_owned(), "view:git.tab".to_owned()]
        );
        assert!(data.layout.is_empty());
    }

    #[test]
    fn resolve_restores_tabs_rail_closed_tabs_side_tabs_and_panels() {
        let bench_views = ["terminal.tab", "editor.tab", "git.tab", "browser.tab"];
        let rail_views = ["board.page", "routine.page", "files.page"];
        let sidebar_views = ["projects.list", "benches.tree"];
        let right_views = ["chat.panel", "helper.runs"];

        let layout = BenchLayout {
            active_tab: Some("git.tab".to_owned()),
            rail_page: Some("routine.page".to_owned()),
            closed_tabs: vec!["browser.tab".to_owned(), "terminal.tab".to_owned()],
            sidebar_tab: Some("benches.tree".to_owned()),
            right_tab: Some("helper.runs".to_owned()),
            sidebar_open: false,
            right_panel_open: false,
        };
        let mut resolved = layout.resolve(&bench_views, &rail_views, &sidebar_views, &right_views);
        assert_eq!(resolved.active_tab, 2);
        assert_eq!(resolved.rail_page.as_deref(), Some("routine.page"));
        assert_eq!(resolved.tabs.closed_indexes(), &[3, 0]);
        assert_eq!(resolved.tabs.reopen(bench_views.len()), Some(0));
        assert_eq!(resolved.tabs.reopen(bench_views.len()), Some(3));
        assert_eq!(resolved.sidebar_tab, 1);
        assert_eq!(resolved.right_tab, 1);
        assert!(!resolved.sidebar_open);
        assert!(!resolved.right_panel_open);
    }

    #[test]
    fn cli_flags_override_restored_bench_layout() {
        let bench_views = ["terminal.tab", "editor.tab", "git.tab"];
        let rail_views = ["board.page", "routine.page"];
        let sidebar_views = ["projects.list"];
        let right_views = ["chat.panel", "helper.runs"];

        let saved = BenchLayout {
            active_tab: Some("editor.tab".to_owned()),
            rail_page: Some("board.page".to_owned()),
            closed_tabs: vec!["git.tab".to_owned()],
            sidebar_tab: Some("projects.list".to_owned()),
            right_tab: Some("chat.panel".to_owned()),
            sidebar_open: true,
            right_panel_open: false,
        };
        let mut resolved = saved.resolve(&bench_views, &rail_views, &sidebar_views, &right_views);
        resolved.apply_cli_overrides(
            &CliOverrides {
                open: None,
                rail: None,
                bench_tab: Some("git.tab"),
                right_tab: Some("helper.runs"),
                env_right_tab: None,
            },
            &bench_views,
            &rail_views,
            &right_views,
        );
        assert_eq!(resolved.active_tab, 2);
        assert!(resolved.tabs.is_visible(2, bench_views.len()));
        assert_eq!(resolved.rail_page, None);
        assert_eq!(resolved.right_tab, 1);
        assert!(resolved.right_panel_open);
    }

    #[test]
    fn writer_persists_layout_and_history_together_via_runtime_tasks() {
        let root = temp_dir("writer");
        let mut runtime = Runtime::new(
            Vec::new(),
            RuntimeOptions {
                data_dir: Some(root.join("data")),
                user_dir: Some(root.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("runtime boots");

        let mut writer = ShellWriter::default();
        let mut data = ShellData {
            anvil_history: vec!["command:help.keymap".to_owned()],
            layout: BTreeMap::new(),
        };
        data.layout.insert(
            "proj:main".to_owned(),
            BenchLayout {
                active_tab: Some("git.tab".to_owned()),
                rail_page: Some("board.page".to_owned()),
                closed_tabs: vec!["browser.tab".to_owned()],
                sidebar_tab: Some("projects.list".to_owned()),
                right_tab: Some("helper.runs".to_owned()),
                sidebar_open: false,
                right_panel_open: true,
            },
        );

        writer.save(&mut runtime, &data);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while writer.busy() && std::time::Instant::now() < deadline {
            runtime.poll_tasks();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        runtime.poll_tasks();
        assert!(!writer.busy());

        let LoadResult::Loaded(raw) = runtime.store().load::<Value>("shell") else {
            panic!("expected shell store file to exist");
        };
        let loaded = ShellData::from_json(&raw, true);
        assert_eq!(loaded, data);
        drop(runtime);
        let _ = std::fs::remove_dir_all(root);
    }
}
