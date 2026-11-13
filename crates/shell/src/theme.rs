//! Brasa tokens for the shell, chosen from the appearance settings.

use forja_ui_kit::{Density, Tokens};

/// Builds the tokens the window should paint.
///
/// `mode` is `system`, `dark` or `light`. Named terminal palettes stay on the
/// Brasa preset of that mode: they are not interface themes.
pub fn tokens_for(
    mode: &str,
    density: &str,
    reduce_motion: bool,
    system_dark: bool,
    dark_name: &str,
    light_name: &str,
) -> Tokens {
    let dark = match mode {
        "light" => false,
        "dark" => true,
        _ => system_dark,
    };
    let mut tokens = if dark {
        dark_preset(dark_name)
    } else {
        light_preset(light_name)
    };
    tokens.density = if density == "compact" {
        Density::Compact
    } else {
        Density::Comfortable
    };
    tokens.reduce_motion = reduce_motion;
    tokens
}

/// Interface palette of a named dark theme (`appearance.theme.dark_name`).
fn dark_preset(name: &str) -> Tokens {
    match name {
        "carvao" => Tokens::carvao(),
        "cinza_quente" => Tokens::cinza_quente(),
        "aco" => Tokens::aco(),
        _ => Tokens::brasa_dark(),
    }
}

/// Interface palette of the light theme (`appearance.theme.light_name`).
fn light_preset(name: &str) -> Tokens {
    match name {
        "brasa_claro" => Tokens::brasa_light(),
        _ => Tokens::brasa_light(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_ui_kit::Mode;

    #[test]
    fn system_mode_follows_the_probe_and_density_is_independent() {
        let dark = tokens_for("system", "compact", true, true, "brasa_escuro", "brasa_claro");
        assert_eq!(dark.mode, Mode::Dark);
        assert_eq!(dark.density, Density::Compact);
        assert!(dark.reduce_motion);
        let light = tokens_for("system", "comfortable", false, false, "brasa_escuro", "brasa_claro");
        assert_eq!(light.mode, Mode::Light);
        assert_eq!(light.density, Density::Comfortable);
        assert_eq!(tokens_for("light", "comfortable", false, true, "brasa_escuro", "brasa_claro").mode, Mode::Light);
        assert_eq!(tokens_for("dark", "comfortable", false, false, "brasa_escuro", "brasa_claro").mode, Mode::Dark);
    }
}

#[cfg(test)]
mod named_theme_tests {
    use super::*;

    #[test]
    fn dark_name_picks_the_named_dark_palette() {
        let carvao = tokens_for("dark", "comfortable", false, false, "carvao", "brasa_claro");
        assert_eq!(carvao.name, "Carvão");
        let brasa = tokens_for("dark", "comfortable", false, false, "unknown", "brasa_claro");
        assert_eq!(brasa.name, Tokens::brasa_dark().name);
        let light = tokens_for("light", "comfortable", false, true, "carvao", "brasa_claro");
        assert_eq!(light.name, Tokens::brasa_light().name);
    }
}
