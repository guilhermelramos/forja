//! Helper activity contributed without a shell dependency.
use crate::Shared;
use forja_contracts::HelperStatus;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::Ctx;

pub struct HelperStatusView(pub Shared);
impl View for HelperStatusView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let (working, waiting) = self
            .0
            .state
            .lock()
            .map(|state| activity_counts(state.runs.values().map(|run| run.status)))
            .unwrap_or_default();
        let label = cx.t_args(
            "helper.status.summary",
            &[
                ("working", &working.to_string()),
                ("waiting", &waiting.to_string()),
            ],
        );
        ui.label(egui::RichText::new(label).small());
    }
}

fn activity_counts(statuses: impl Iterator<Item = HelperStatus>) -> (usize, usize) {
    statuses.fold((0, 0), |(working, waiting), status| {
        (
            working + usize::from(status == HelperStatus::Working),
            waiting + usize::from(status == HelperStatus::NeedsYou),
        )
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_working_and_waiting_runs_are_counted() {
        assert_eq!(
            activity_counts(
                [
                    HelperStatus::Working,
                    HelperStatus::NeedsYou,
                    HelperStatus::Finished,
                    HelperStatus::Working
                ]
                .into_iter()
            ),
            (2, 1)
        );
    }
}
