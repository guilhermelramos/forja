//! Renders `design/coverage.md` from catalog comparison results.

use crate::catalog::compare::{CoverageReport, KeybindComparison, SettingComparison};
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub fn render(report: &CoverageReport) -> String {
    let mut out = String::new();
    let total_cat_settings = report.settings.len();
    let project_cat_settings = report
        .settings
        .iter()
        .filter(|s| s.scope.as_deref() == Some("project"))
        .count();
    let reg_cat_settings = report.settings.iter().filter(|s| s.registered).count();
    let deferred_settings = report.settings.iter().filter(|s| s.deferred.is_some()).count();
    let missing_settings = report.settings.iter().filter(|s| !s.registered && s.deferred.is_none()).count();
    let with_effect_cat = report.settings.iter().filter(|s| s.registered && s.has_effect).count();
    let no_effect_cat = reg_cat_settings - with_effect_cat;
    let extra_with_effect = report.extra_settings.len() - report.no_effect_extra_settings.len();
    let total_with_effect = with_effect_cat + extra_with_effect;
    let total_no_effect = report.runtime_settings_total - total_with_effect;
    let mismatches = report.type_or_default_mismatches().len();
    let range_notes = report
        .settings
        .iter()
        .filter(|s| s.range_or_options_note.is_some() || s.module_mismatch.is_some())
        .count();

    let total_cat_keys = report.keybinds.len();
    let reg_cat_keys = report.keybinds.iter().filter(|k| k.registered).count();
    let missing_keys = total_cat_keys - reg_cat_keys;
    let key_mismatches = report.keybinds.iter().filter(|k| k.mismatch.is_some()).count();

    writeln!(out, "# Cobertura do catálogo (`design/settings.toml` e `design/keymap.toml`)").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "> Gerado automaticamente por `cargo xtask catalog`. Não edite à mão; atualize os módulos ou o catálogo e rode `cargo xtask catalog` novamente."
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Resumo").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| Métrica | Valor |").unwrap();
    writeln!(out, "|---|---|").unwrap();
    writeln!(out, "| Configurações no catálogo | {total_cat_settings} |").unwrap();
    writeln!(
        out,
        "| Configurações sobreponíveis por projeto (`scope = \"project\"`) | {project_cat_settings} |"
    )
    .unwrap();
    writeln!(
        out,
        "| Configurações registradas no app | {} ({reg_cat_settings} do catálogo + {} extras) |",
        report.runtime_settings_total,
        report.extra_settings.len()
    )
    .unwrap();
    writeln!(out, "| Configurações com efeito real no código | {total_with_effect} ({with_effect_cat} do catálogo) |").unwrap();
    writeln!(out, "| Configurações registradas sem efeito | {total_no_effect} ({no_effect_cat} do catálogo) |").unwrap();
    writeln!(out, "| Configurações do catálogo ausentes no app | {missing_settings} |").unwrap();
    writeln!(out, "| Configurações adiadas | {deferred_settings} |").unwrap();
    writeln!(out, "| Divergências de tipo ou padrão | {mismatches} |").unwrap();
    writeln!(out, "| Divergências de faixa, opções ou módulo | {range_notes} |").unwrap();
    writeln!(out, "| Atalhos no catálogo | {total_cat_keys} |").unwrap();
    writeln!(
        out,
        "| Atalhos registrados no app | {} ({reg_cat_keys} do catálogo + {} extras) |",
        report.runtime_keybinds_total,
        report.extra_keybinds.len()
    )
    .unwrap();
    writeln!(out, "| Atalhos do catálogo ausentes no app | {missing_keys} |").unwrap();
    writeln!(out, "| Atalhos com tecla ou escopo divergente | {key_mismatches} |").unwrap();
    writeln!(out).unwrap();

    render_mismatches(&mut out, report);
    render_deferred(&mut out, report);
    render_settings_table(&mut out, &report.settings);
    render_keybinds_table(&mut out, &report.keybinds);
    render_extras(&mut out, report);
    render_no_effect(&mut out, report);
    render_backlog(&mut out, report);

    out
}

fn render_deferred(out: &mut String, report: &CoverageReport) {
    writeln!(out, "## Configurações adiadas").unwrap();
    writeln!(out).unwrap();
    for item in &report.settings {
        if let Some(reason) = &item.deferred {
            writeln!(out, "- `{}`: {}", item.key, reason).unwrap();
        }
    }
    writeln!(out).unwrap();
}

fn render_mismatches(out: &mut String, report: &CoverageReport) {
    writeln!(out, "## Divergências entre catálogo e app").unwrap();
    writeln!(out).unwrap();
    let setting_issues: Vec<&SettingComparison> = report
        .settings
        .iter()
        .filter(|s| {
            s.type_mismatch.is_some()
                || s.default_mismatch.is_some()
                || s.range_or_options_note.is_some()
                || s.module_mismatch.is_some()
        })
        .collect();
    let key_issues: Vec<&KeybindComparison> = report
        .keybinds
        .iter()
        .filter(|k| k.mismatch.is_some())
        .collect();

    if setting_issues.is_empty() && key_issues.is_empty() {
        writeln!(out, "Nenhuma divergência de tipo, padrão, faixa, opções, módulo ou tecla.").unwrap();
        writeln!(out).unwrap();
        return;
    }

    if !setting_issues.is_empty() {
        writeln!(out, "| Chave | Tipo | Padrão | Faixa/Opções | Módulo |").unwrap();
        writeln!(out, "|---|---|---|---|---|").unwrap();
        for item in setting_issues {
            writeln!(
                out,
                "| `{}` | {} | {} | {} | {} |",
                item.key,
                item.type_mismatch.as_deref().unwrap_or("ok"),
                item.default_mismatch.as_deref().unwrap_or("ok"),
                item.range_or_options_note.as_deref().unwrap_or("ok"),
                item.module_mismatch.as_deref().unwrap_or("ok"),
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }

    if !key_issues.is_empty() {
        writeln!(out, "| Atalho | Catálogo | App | Divergência |").unwrap();
        writeln!(out, "|---|---|---|---|").unwrap();
        for item in key_issues {
            writeln!(
                out,
                "| `{}` | `{}` ({}) | `{}` ({}) | {} |",
                item.id,
                item.catalog_key,
                item.catalog_scope,
                item.runtime_key.as_deref().unwrap_or("-"),
                item.runtime_scope.as_deref().unwrap_or("-"),
                item.mismatch.as_deref().unwrap_or("-"),
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }
}

fn render_settings_table(out: &mut String, settings: &[SettingComparison]) {
    writeln!(out, "## Configurações (`design/settings.toml`)").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| Chave | Módulo (catálogo) | Módulo (app) | Registrada | Tipo/Padrão/Faixa | Efeito |").unwrap();
    writeln!(out, "|---|---|---|---|---|---|").unwrap();
    for item in settings {
        let reg = if item.registered { "sim" } else if item.deferred.is_some() { "adiada" } else { "ausente" };
        let run_mod = item.runtime_module.as_deref().unwrap_or("-");
        let contract = if !item.registered {
            "-".to_owned()
        } else {
            let mut errs = Vec::new();
            if let Some(e) = &item.type_mismatch {
                errs.push(format!("tipo: {e}"));
            }
            if let Some(e) = &item.default_mismatch {
                errs.push(format!("padrão: {e}"));
            }
            if let Some(e) = &item.range_or_options_note {
                errs.push(e.clone());
            }
            if let Some(e) = &item.module_mismatch {
                errs.push(format!("módulo: {e}"));
            }
            if errs.is_empty() {
                "ok".to_owned()
            } else {
                errs.join("; ")
            }
        };
        let effect = if item.deferred.is_some() {
            "adiada"
        } else if !item.registered {
            "ausente"
        } else if item.has_effect {
            "sim"
        } else {
            "sem efeito"
        };
        writeln!(
            out,
            "| `{}` | `{}` | `{}` | {} | {} | {} |",
            item.key, item.catalog_module, run_mod, reg, contract, effect
        )
        .unwrap();
    }
    writeln!(out).unwrap();
}

fn render_keybinds_table(out: &mut String, keybinds: &[KeybindComparison]) {
    writeln!(out, "## Atalhos (`design/keymap.toml`)").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| ID | Tecla (catálogo) | Escopo | Registrado | Tecla (app) | Situação |").unwrap();
    writeln!(out, "|---|---|---|---|---|---|").unwrap();
    for item in keybinds {
        let reg = if item.registered { "sim" } else { "ausente" };
        let run_key = item.runtime_key.as_deref().unwrap_or("-");
        let status = if !item.registered {
            "ausente".to_owned()
        } else if let Some(note) = &item.mismatch {
            note.clone()
        } else {
            "ok".to_owned()
        };
        writeln!(
            out,
            "| `{}` | `{}` | `{}` | {} | `{}` | {} |",
            item.id, item.catalog_key, item.catalog_scope, reg, run_key, status
        )
        .unwrap();
    }
    writeln!(out).unwrap();
}

fn render_extras(out: &mut String, report: &CoverageReport) {
    writeln!(out, "## Registrados no app mas ausentes do catálogo").unwrap();
    writeln!(out).unwrap();
    if report.extra_settings.is_empty() && report.extra_keybinds.is_empty() {
        writeln!(out, "Nenhum item registrado fora dos catálogos.").unwrap();
        writeln!(out).unwrap();
        return;
    }
    if !report.extra_settings.is_empty() {
        writeln!(out, "### Configurações extras").unwrap();
        writeln!(out).unwrap();
        for item in &report.extra_settings {
            writeln!(
                out,
                "- `{}` (módulo `{}`, tipo `{}`, padrão `{}`)",
                item.key,
                item.module,
                item.kind.label(),
                item.default
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }
    if !report.extra_keybinds.is_empty() {
        writeln!(out, "### Atalhos extras").unwrap();
        writeln!(out).unwrap();
        for item in &report.extra_keybinds {
            writeln!(
                out,
                "- `{}` (`{}` no escopo `{}`, módulo `{}`)",
                item.id, item.key, item.scope, item.module
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }
}

fn render_no_effect(out: &mut String, report: &CoverageReport) {
    writeln!(out, "## Configurações registradas sem efeito real").unwrap();
    writeln!(out).unwrap();
    let mut by_module: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for item in &report.settings {
        if item.registered && !item.has_effect {
            let module = item.runtime_module.as_deref().unwrap_or(&item.catalog_module);
            by_module.entry(module).or_default().push(&item.key);
        }
    }
    for item in &report.extra_settings {
        if report.no_effect_extra_settings.contains(&item.key) {
            by_module.entry(&item.module).or_default().push(&item.key);
        }
    }
    if by_module.is_empty() {
        writeln!(out, "Todas as configurações registradas têm leitura real no código.").unwrap();
        writeln!(out).unwrap();
        return;
    }
    for (module, keys) in by_module {
        writeln!(out, "- **`{module}`** ({}): {}", keys.len(), keys.iter().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", ")).unwrap();
    }
    writeln!(out).unwrap();
}

fn render_backlog(out: &mut String, report: &CoverageReport) {
    writeln!(out, "## Backlog priorizado (ausentes e sem efeito)").unwrap();
    writeln!(out).unwrap();
    let mut missing_by_group: BTreeMap<&str, usize> = BTreeMap::new();
    for item in &report.settings {
        if !item.registered && item.deferred.is_none() {
            *missing_by_group.entry(item.catalog_module.as_str()).or_default() += 1;
        }
    }
    let mut missing_keys_by_family: BTreeMap<&str, usize> = BTreeMap::new();
    for item in &report.keybinds {
        if !item.registered {
            let fam = if item.family.is_empty() {
                "outros"
            } else {
                item.family.as_str()
            };
            *missing_keys_by_family.entry(fam).or_default() += 1;
        }
    }
    let mut no_effect_by_module: BTreeMap<&str, usize> = BTreeMap::new();
    for item in &report.settings {
        if item.registered && !item.has_effect {
            *no_effect_by_module.entry(item.catalog_module.as_str()).or_default() += 1;
        }
    }

    writeln!(out, "| Prioridade | Frente | Pendentes | Tamanho estimado | Observação |").unwrap();
    writeln!(out, "|---|---|---|---|---|").unwrap();

    for (module, count) in missing_by_group {
        let (prio, size, note) = classify_missing_module(module, count);
        writeln!(out, "| {prio} | Settings ausentes: `{module}` | {count} | {size} | {note} |").unwrap();
    }
    for (module, count) in no_effect_by_module {
        let (prio, size, note) = classify_no_effect_module(module, count);
        writeln!(out, "| {prio} | Settings sem efeito: `{module}` | {count} | {size} | {note} |").unwrap();
    }
    for (family, count) in missing_keys_by_family {
        let size = estimate_size(count * 35);
        writeln!(
            out,
            "| P2 | Atalhos ausentes: `{family}` | {count} | {size} | Registrar comandos e `keymap` no módulo dono |"
        )
        .unwrap();
    }
}

fn classify_missing_module(module: &str, count: usize) -> (&'static str, String, &'static str) {
    match module {
        "privacy" | "network" => (
            "P1",
            estimate_size(count * 120),
            "Exige módulo dedicado de privacidade/rede no shell",
        ),
        "extension" => (
            "P2",
            estimate_size(count * 80),
            "Aguardando integração completa de limites da runtime Lua",
        ),
        "appearance" | "app" | "editor" | "helper" => (
            "P2",
            estimate_size(count * 60),
            "Recursos maiores de tema, fonte, bandeja ou contexto do ajudante",
        ),
        _ => (
            "P3",
            estimate_size(count * 45),
            "Opções adicionais do catálogo ainda não registradas",
        ),
    }
}

fn classify_no_effect_module(module: &str, count: usize) -> (&'static str, String, &'static str) {
    match module {
        "editor" => (
            "P1",
            estimate_size(count * 70),
            "Recursos visuais e de renderização do editor (minimap, ligaduras, line_height, preview)",
        ),
        "muster" | "helper" | "mcp" => (
            "P1",
            estimate_size(count * 60),
            "Políticas operacionais e de ciclo de vida ainda sem implementação completa",
        ),
        "browser" | "remote" | "request" => (
            "P2",
            estimate_size(count * 50),
            "Preferências de rede, cookies, SSH ou automação de PR",
        ),
        _ => (
            "P2",
            estimate_size(count * 30),
            "Ligar leitura em runtime e cobrir com testes unitários",
        ),
    }
}

fn estimate_size(lines: usize) -> String {
    let tier = if lines <= 60 {
        "P"
    } else if lines <= 250 {
        "M"
    } else {
        "G"
    };
    format!("{tier} (~{lines} L)")
}
