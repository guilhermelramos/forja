//! Diverts egui's exit commands before eframe can hand them to the operating
//! system. Kept apart from the already large `app.rs`.

use eframe::egui;

/// Moves egui's URL and clipboard output into the kernel's test log. A test run
/// must never open the person's browser or touch the system clipboard, so the
/// commands are consumed here, before `egui-winit` sees them. Outside a test
/// run this does nothing.
pub(crate) fn isolate_external_output(ctx: &egui::Context) {
    if !forja_kernel::process::is_test_mode() {
        return;
    }
    ctx.output_mut(|output| {
        for command in std::mem::take(&mut output.commands) {
            match command {
                egui::OutputCommand::OpenUrl(url) => {
                    forja_kernel::process::record_external(
                        forja_kernel::process::ExternalKind::Open,
                        url.url,
                    );
                }
                egui::OutputCommand::CopyText(text) => {
                    forja_kernel::process::record_external(
                        forja_kernel::process::ExternalKind::Copy,
                        text,
                    );
                }
                egui::OutputCommand::CopyImage(_) => {
                    forja_kernel::process::record_external(
                        forja_kernel::process::ExternalKind::Copy,
                        "<image>",
                    );
                }
            }
        }
    });
}
