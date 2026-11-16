//! Current bench context contributed through the status slot.
use crate::state::Shared;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::Ctx;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct BenchStatus {
    shared: Shared,
    branch: Arc<Mutex<Option<(String, String)>>>,
    requested: Option<(String, Instant)>,
}
impl BenchStatus {
    pub fn new(shared: Shared) -> Self {
        Self {
            shared,
            branch: Arc::new(Mutex::new(None)),
            requested: None,
        }
    }
}
impl View for BenchStatus {
    fn poll(&mut self, cx: &mut Ctx<'_>) {
        let Some(bench) = self.shared.current() else {
            return;
        };
        if self
            .requested
            .as_ref()
            .is_some_and(|(path, at)| path == &bench.path && at.elapsed() < Duration::from_secs(30))
        {
            return;
        }
        self.requested = Some((bench.path.clone(), Instant::now()));
        let branch = self.branch.clone();
        cx.spawn(
            move || {
                let current = crate::git::current_branch(std::path::Path::new(&bench.path))
                    .unwrap_or_default();
                (bench.path, current)
            },
            move |_cx, result| {
                if let Ok(mut slot) = branch.lock() {
                    *slot = Some(result);
                }
            },
        );
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        self.poll(cx);
        if let Some(bench) = self.shared.current() {
            let branch = self
                .branch
                .lock()
                .ok()
                .and_then(|value| {
                    value
                        .as_ref()
                        .filter(|(path, _)| path == &bench.path)
                        .map(|(_, branch)| branch.clone())
                })
                .filter(|branch| !branch.is_empty())
                .or(bench.branch);
            let label = context_label(&bench.name, branch.as_deref());
            ui.set_max_width(ui.available_width().min(200.0));
            ui.add(egui::Label::new(&label).truncate())
                .on_hover_text(&label);
        }
    }
}
fn context_label(name: &str, branch: Option<&str>) -> String {
    match branch {
        Some(branch) if branch != name => format!("{branch} · {name}"),
        _ => name.to_owned(),
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn context_keeps_branch_and_bench_without_repeating_names() {
        assert_eq!(
            super::context_label("My bench", Some("feature/work")),
            "feature/work · My bench"
        );
        assert_eq!(super::context_label("main", Some("main")), "main");
    }
}
