use forja_ui_kit::{Motion, Tokens};

#[test]
fn motion_tokens_use_three_durations_and_never_exceed_the_ceiling() {
    let mut tokens = Tokens::brasa_dark();
    tokens.deterministic = false;
    assert_eq!(tokens.duration(Motion::Fast), 0.09);
    assert_eq!(tokens.duration(Motion::Base), 0.15);
    assert_eq!(tokens.duration(Motion::Slow), 0.24);
    tokens.motion_duration_ms = 400.0;
    for motion in [Motion::Fast, Motion::Base, Motion::Slow] {
        assert!(tokens.duration(motion) <= 0.25);
    }
    tokens.reduce_motion = true;
    for motion in [Motion::Fast, Motion::Base, Motion::Slow] {
        assert_eq!(tokens.duration(motion), 0.0);
    }
}

#[test]
fn duration_setting_scales_every_token_and_zero_disables_motion() {
    let mut tokens = Tokens::brasa_dark();
    tokens.deterministic = false;
    tokens.motion_duration_ms = 75.0;
    assert_eq!(tokens.duration(Motion::Base), 0.075);
    assert_eq!(tokens.duration(Motion::Fast), 0.045);
    tokens.motion_duration_ms = 0.0;
    assert_eq!(tokens.duration(Motion::Slow), 0.0);
    tokens.motion_duration_ms = 150.0;
    tokens.deterministic = true;
    assert_eq!(tokens.duration(Motion::Slow), 0.0);
}

#[test]
fn disabled_tooltips_keep_accessibility_without_opening_a_popup() {
    use forja_ui_kit::egui;
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut tokens = Tokens::brasa_dark();
    tokens.tooltips_enabled = false;
    forja_ui_kit::apply(&ctx, &tokens);
    let output = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            forja_ui_kit::icon_button(ui, forja_ui_kit::ADD, "Add");
            assert!(!forja_ui_kit::tooltips_enabled(ui.ctx()));
        });
    });
    assert!(output.platform_output.accesskit_update.unwrap().nodes.iter().any(|(_, n)| n.label() == Some("Add")));
}
