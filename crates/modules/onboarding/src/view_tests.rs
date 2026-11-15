use super::*;
use serde_json::json;

#[test]
fn summarize_helpers_never_leaks_started_json() {
    assert_eq!(summarize_helpers(&json!({ "started": true })), HelperSummary::None);
    assert_eq!(
        summarize_helpers(&json!({ "detecting": true, "profiles": [] })),
        HelperSummary::Detecting
    );
    assert_eq!(
        summarize_helpers(&json!({
            "detecting": false,
            "profiles": [
                { "id": "claude-code", "name": "Claude Code", "installed": false },
                { "id": "codex", "name": "Codex", "installed": true }
            ]
        })),
        HelperSummary::Found(vec!["Codex".to_owned()])
    );
}

fn empty_frame() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1000.0, 700.0),
        )),
        ..Default::default()
    }
}

fn key_frame(key: egui::Key, pressed: bool) -> egui::RawInput {
    egui::RawInput {
        events: vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        ..empty_frame()
    }
}

fn focused_label(output: &egui::FullOutput) -> Option<String> {
    let update = output.platform_output.accesskit_update.as_ref()?;
    let focus = update.focus;
    update
        .nodes
        .iter()
        .find(|(id, _)| *id == focus)
        .and_then(|(_, node)| node.label().map(str::to_owned))
}

fn render_wizard(
    runtime: &mut forja_kernel::Runtime,
    view: &mut WizardView,
    ctx: &egui::Context,
    input: egui::RawInput,
) -> egui::FullOutput {
    let mut cx = runtime.ctx("onboarding");
    let output = ctx.run(input, |ctx| {
        egui::Area::new(egui::Id::new(("forja.modal", "onboarding.wizard")))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.push_id("onboarding.wizard", |ui| {
                    forja_kernel::ui_contract::View::ui(view, ui, &mut cx);
                });
            });
    });
    drop(cx);
    runtime.poll_tasks();
    output
}

fn press_key(
    runtime: &mut forja_kernel::Runtime,
    view: &mut WizardView,
    ctx: &egui::Context,
    key: egui::Key,
) -> egui::FullOutput {
    let _ = render_wizard(runtime, view, ctx, key_frame(key, true));
    let _ = render_wizard(runtime, view, ctx, key_frame(key, false));
    render_wizard(runtime, view, ctx, empty_frame())
}

struct FakeHelpers;

impl forja_kernel::Module for FakeHelpers {
    fn manifest(&self) -> forja_kernel::ModuleManifest {
        forja_kernel::ModuleManifest::new("helper", "onboarding.name", "0.0.0")
    }

    fn register(&mut self, r: &mut forja_kernel::Registrar<'_>) {
        r.command("helper.detect", "onboarding.name", |_, _| {
            Ok(json!({ "detecting": false, "profiles": [] }))
        });
    }
}

fn setup_wizard(tag: &str) -> (forja_kernel::Runtime, Shared, egui::Context, PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("forja-onb-{tag}-{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    let state = Shared::new();
    let runtime = forja_kernel::Runtime::new(
        vec![
            Box::new(FakeHelpers),
            Box::new(crate::OnboardingModule {
                state: state.clone(),
            }),
        ],
        forja_kernel::RuntimeOptions {
            data_dir: Some(dir.clone()),
            user_dir: Some(dir.join("user")),
            locale: Some("en".to_owned()),
            wake: None,
        },
    )
    .unwrap();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    forja_ui_kit::install_fonts(&ctx).unwrap();
    forja_ui_kit::apply(&ctx, &forja_ui_kit::Tokens::brasa_dark());
    (runtime, state, ctx, dir)
}

#[test]
fn tab_order_starts_with_content_and_wraps_cleanly_across_steps() {
    let (mut runtime, state, ctx, dir) = setup_wizard("tab-order");
    let mut view = WizardView {
        state: state.clone(),
    };
    let _ = render_wizard(&mut runtime, &mut view, &ctx, empty_frame());
    let _ = render_wizard(&mut runtime, &mut view, &ctx, empty_frame());

    // Step 0 (Language): ComboBox -> Next -> Close -> wraps back to ComboBox.
    let mut cycle = Vec::new();
    for _ in 0..4 {
        let out = press_key(&mut runtime, &mut view, &ctx, egui::Key::Tab);
        cycle.push(focused_label(&out));
    }
    assert_eq!(
        cycle,
        vec![
            Some("Follow Windows".into()),
            Some("Next".into()),
            Some("Close".into()),
            Some("Follow Windows".into()),
        ]
    );

    // Space on focused ComboBox must open its popup and keep it open across settle frames.
    let _ = press_key(&mut runtime, &mut view, &ctx, egui::Key::Space);
    for _ in 0..5 {
        let _ = render_wizard(&mut runtime, &mut view, &ctx, empty_frame());
    }
    assert!(
        egui::Popup::is_any_open(&ctx),
        "Space on focused ComboBox must keep popup open across settle frames"
    );
    let _ = press_key(&mut runtime, &mut view, &ctx, egui::Key::ArrowDown);
    let _ = press_key(&mut runtime, &mut view, &ctx, egui::Key::Enter);
    assert!(
        !egui::Popup::is_any_open(&ctx),
        "Enter inside ComboBox popup picks an item and closes the popup"
    );
    assert!(state.should_show(), "Enter in popup must not close wizard");

    // Activate Next via Tab + Enter -> moves to Theme with focus cleared.
    let _ = press_key(&mut runtime, &mut view, &ctx, egui::Key::Tab);
    let _ = press_key(&mut runtime, &mut view, &ctx, egui::Key::Enter);
    assert_eq!(state.step(), 1);
    assert_eq!(ctx.memory(|m| m.focused()), None);

    // Pressing Enter right after advancing must not activate Back or Next.
    let _ = press_key(&mut runtime, &mut view, &ctx, egui::Key::Enter);
    assert_eq!(state.step(), 1);

    // Step 1 (Theme): ComboBox -> Back -> Next -> Close -> ComboBox.
    let mut theme_cycle = Vec::new();
    for _ in 0..5 {
        let out = press_key(&mut runtime, &mut view, &ctx, egui::Key::Tab);
        theme_cycle.push(focused_label(&out));
    }
    assert_eq!(
        theme_cycle,
        vec![
            Some("Follow Windows".into()),
            Some("Back".into()),
            Some("Next".into()),
            Some("Close".into()),
            Some("Follow Windows".into()),
        ]
    );

    // Navigate Back -> Back to Language cleanly with focus cleared.
    let _ = press_key(&mut runtime, &mut view, &ctx, egui::Key::Tab); // Back
    let _ = press_key(&mut runtime, &mut view, &ctx, egui::Key::Enter);
    assert_eq!(state.step(), 0);
    assert_eq!(ctx.memory(|m| m.focused()), None);

    // Advance Language -> Theme -> Folder -> Helpers.
    for _ in 0..3 {
        state.set_step(state.step() + 1);
        let _ = render_wizard(&mut runtime, &mut view, &ctx, empty_frame());
    }
    assert_eq!(state.step(), 3);
    let mut helpers_cycle = Vec::new();
    for _ in 0..4 {
        let out = press_key(&mut runtime, &mut view, &ctx, egui::Key::Tab);
        helpers_cycle.push(focused_label(&out));
    }
    assert_eq!(
        helpers_cycle,
        vec![
            Some("Back".into()),
            Some("Next".into()),
            Some("Close".into()),
            Some("Back".into()),
        ]
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn folder_validation_rejects_invalid_and_missing_paths_with_human_error() {
    assert!(validate_folder_syntax("C:\\projects\\forja").is_ok());
    assert!(validate_folder_syntax("C:/projects/forja").is_ok());
    assert!(validate_folder_syntax("<>:\"/\\|?* missing").is_err());
    assert!(validate_folder_syntax("C:\\foo\\bar:baz").is_err());

    let (mut runtime, state, ctx, dir) = setup_wizard("folder-validation");
    let valid_projects = dir.join("valid-projects");
    std::fs::create_dir_all(&valid_projects).unwrap();
    let file_not_dir = dir.join("not-a-dir.txt");
    std::fs::write(&file_not_dir, "x").unwrap();
    state.set_step(2); // Step::Folder
    let mut view = WizardView {
        state: state.clone(),
    };

    // 1. Forbidden Windows characters: blocked synchronously with localized error.
    state.set_folder_draft(Some(format!(
        "<>:\"/\\|?* esta pasta não existe 🚧{}",
        "x".repeat(120)
    )));
    {
        let mut cx = runtime.ctx("onboarding");
        advance_folder_step(&mut cx, &state, 2);
    }
    assert_eq!(state.step(), 2);
    assert_eq!(
        state.folder_error().map(|e| e.summary),
        Some(runtime.i18n().t("kernel.error.invalid-name"))
    );

    // 2. Non-existent directory: blocked after background check.
    state.set_folder_draft(Some(dir.join("missing-folder").display().to_string()));
    {
        let mut cx = runtime.ctx("onboarding");
        advance_folder_step(&mut cx, &state, 2);
    }
    for _ in 0..20 {
        if !state.validating_folder() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _ = render_wizard(&mut runtime, &mut view, &ctx, empty_frame());
    }
    assert_eq!(state.step(), 2);
    assert_eq!(
        state.folder_error().map(|e| e.summary),
        Some(runtime.i18n().t("kernel.error.not-found"))
    );

    // 3. Regular file instead of directory: blocked with invalid-directory error.
    state.set_folder_draft(Some(file_not_dir.display().to_string()));
    {
        let mut cx = runtime.ctx("onboarding");
        advance_folder_step(&mut cx, &state, 2);
    }
    for _ in 0..20 {
        if !state.validating_folder() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _ = render_wizard(&mut runtime, &mut view, &ctx, empty_frame());
    }
    assert_eq!(state.step(), 2);
    assert_eq!(
        state.folder_error().map(|e| e.summary),
        Some(runtime.i18n().t("kernel.error.invalid-directory"))
    );

    // 4. Valid directory: clears error, saves setting, and advances step.
    state.set_folder_draft(Some(valid_projects.display().to_string()));
    {
        let mut cx = runtime.ctx("onboarding");
        advance_folder_step(&mut cx, &state, 2);
    }
    for _ in 0..20 {
        if !state.validating_folder() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _ = render_wizard(&mut runtime, &mut view, &ctx, empty_frame());
    }
    assert_eq!(state.step(), 3);
    assert!(state.folder_error().is_none());
    let saved = runtime
        .ctx("onboarding")
        .setting("onboarding.projects.dir")
        .and_then(|v| v.as_str().map(str::to_owned));
    assert!(saved.is_some_and(|s| !s.is_empty()));
    let _ = std::fs::remove_dir_all(dir);
}
