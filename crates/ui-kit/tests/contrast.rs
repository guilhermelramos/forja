use forja_ui_kit::{contrast_ratio, Density, Mode, Tokens};

fn assert_min(name: &str, ratio: f32, floor: f32) {
    assert!(
        ratio + f32::EPSILON >= floor,
        "{name} ficou em {ratio:.2}:1, abaixo de {floor}:1"
    );
}

#[test]
fn text_and_muted_clear_aa_on_every_surface() {
    for tokens in [Tokens::brasa_dark(), Tokens::brasa_light(), Tokens::carvao(), Tokens::cinza_quente(), Tokens::aco()] {
        for (surface_name, surface) in [
            ("bg", tokens.color.bg),
            ("surface", tokens.color.surface),
            ("surface_alt", tokens.color.surface_alt),
        ] {
            assert_min(
                &format!("{} texto/{surface_name}", tokens.name),
                contrast_ratio(tokens.color.text, surface),
                4.5,
            );
            assert_min(
                &format!("{} muted/{surface_name}", tokens.name),
                contrast_ratio(tokens.color.text_muted, surface),
                4.5,
            );
        }
    }
}

#[test]
fn button_labels_clear_aa() {
    for tokens in [Tokens::brasa_dark(), Tokens::brasa_light()] {
        assert_min(
            &format!("{} on_accent", tokens.name),
            contrast_ratio(tokens.color.on_accent, tokens.color.accent),
            4.5,
        );
        assert_min(
            &format!("{} on_danger", tokens.name),
            contrast_ratio(tokens.color.on_danger, tokens.color.danger),
            4.5,
        );
    }
}

#[test]
fn focus_ring_clears_non_text_contrast() {
    for tokens in [Tokens::brasa_dark(), Tokens::brasa_light()] {
        for surface in [
            tokens.color.bg,
            tokens.color.surface,
            tokens.color.surface_alt,
        ] {
            assert_min(
                &format!("{} foco", tokens.name),
                contrast_ratio(tokens.color.accent, surface),
                3.0,
            );
        }
    }
}

#[test]
fn status_marks_stay_visible() {
    let dark = Tokens::brasa_dark();
    for (name, color) in [
        ("accent", dark.color.accent),
        ("spark", dark.color.spark),
        ("ok", dark.color.ok),
        ("danger", dark.color.danger),
        ("info", dark.color.info),
    ] {
        assert_min(
            &format!("escuro {name}"),
            contrast_ratio(color, dark.color.bg),
            4.5,
        );
    }
    let light = Tokens::brasa_light();
    assert_min(
        "claro info",
        contrast_ratio(light.color.info, light.color.bg),
        4.5,
    );
    assert_min(
        "claro danger",
        contrast_ratio(light.color.danger, light.color.bg),
        4.5,
    );
    assert_min(
        "claro spark",
        contrast_ratio(light.color.spark, light.color.surface),
        3.0,
    );
    assert_min(
        "claro ok",
        contrast_ratio(light.color.ok, light.color.surface),
        3.0,
    );
    assert_min(
        "claro accent texto não AA",
        contrast_ratio(light.color.accent, light.color.bg),
        3.0,
    );
}

#[test]
fn theme_toml_roundtrip_and_partial_overlay() {
    let dark = Tokens::brasa_dark();
    let loaded_dark = Tokens::from_toml(&dark.to_toml().unwrap()).unwrap();
    assert_eq!(loaded_dark, dark);

    let light = Tokens::brasa_light();
    let loaded_light = Tokens::from_toml(&light.to_toml().unwrap()).unwrap();
    assert_eq!(loaded_light.mode, Mode::Light);
    assert_eq!(loaded_light.color.bg, light.color.bg);

    let partial = r#"
        name = "Brasa da bancada"
        mode = "dark"
        density = "compact"
        reduce_motion = true

        [color]
        accent = '#E06A20'
    "#;
    let custom = Tokens::from_toml(partial).unwrap();
    assert_eq!(custom.name, "Brasa da bancada");
    assert_eq!(custom.density, Density::Compact);
    assert!(custom.reduce_motion);
    assert_eq!(
        custom.color.accent,
        forja_ui_kit::Color::parse("#E06A20").unwrap()
    );
    assert_eq!(custom.color.bg, dark.color.bg);
}

#[test]
fn grid_spacing_stays_on_four_pixels() {
    for tokens in [Tokens::brasa_dark(), Tokens::brasa_light()] {
        for density in [Density::Comfortable, Density::Compact] {
            let mut tokens = tokens.clone();
            tokens.density = density;
            let metrics = tokens.metrics();
            for value in [
                metrics.control,
                metrics.pad_x,
                metrics.pad_y,
                metrics.gap,
                metrics.row,
                metrics.progress,
                metrics.indent,
                metrics.badge,
                metrics.toggle.0,
                metrics.toggle.1,
            ] {
                let steps = value / forja_ui_kit::GRID;
                assert!(
                    (steps - steps.round()).abs() < 0.01,
                    "{value} fora da grade"
                );
            }
        }
    }
}
