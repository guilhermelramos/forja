use std::path::PathBuf;

use forja_ui_kit::{contrast_ratio, TerminalTheme};

fn temas_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content/themes")
}

#[test]
fn shipped_terminal_themes_are_readable() {
    let themes = TerminalTheme::load_dir(&temas_dir()).expect("content/themes");
    let ids: Vec<_> = themes.iter().map(|theme| theme.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "aco",
            "brasa-claro",
            "brasa-escuro",
            "carvao",
            "cinza-quente"
        ]
    );

    for theme in &themes {
        assert_min(
            &format!("{} frente", theme.name),
            contrast_ratio(theme.foreground, theme.background),
            4.5,
        );
        assert_min(
            &format!("{} cursor", theme.name),
            contrast_ratio(theme.cursor, theme.background),
            3.0,
        );
        assert_min(
            &format!("{} seleção", theme.name),
            contrast_ratio(theme.selection_foreground, theme.selection),
            4.5,
        );
        assert_min(
            &format!("{} seleção visível", theme.name),
            contrast_ratio(theme.selection, theme.background),
            1.4,
        );
        assert_ne!(theme.cursor, theme.foreground);
        let mut seen = Vec::new();
        for (name, color) in theme.ansi.entries() {
            assert_min(
                &format!("{} {name}", theme.name),
                contrast_ratio(color, theme.background),
                4.5,
            );
            assert!(
                !seen.contains(&color),
                "{} {name} repete outra ANSI",
                theme.name
            );
            seen.push(color);
        }
    }
}

fn assert_min(name: &str, ratio: f32, floor: f32) {
    assert!(
        ratio + f32::EPSILON >= floor,
        "{name} ficou em {ratio:.2}:1, abaixo de {floor}:1"
    );
}
