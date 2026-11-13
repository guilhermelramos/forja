//! Helpers the atlas agent loop calls between model runs.
//!
//! ```text
//! cargo xtask atlas brief  --out <dir> --state <id>            # context for one state (stdout)
//! cargo xtask atlas accept --out <dir> --state <id> --from <file> [--model <id>]
//! cargo xtask atlas index  --out <dir>                         # README.md + PROBLEMAS.md
//! ```
//!
//! The model never writes files: it answers with the page between
//! `<<<PAGINA` and `PAGINA>>>`, and `accept` checks it and stamps the front
//! matter (`state`, `capture_sha`), so `status` can tell when it goes stale.

use crate::util::{read, write};
use serde_json::Value;
use std::fmt::Write as _;
use std::path::Path;

pub fn run(command: &str, args: &[String]) -> Result<(), String> {
    let mut out = None;
    let mut state = None;
    let mut from = None;
    let mut model = String::from("unknown");
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().cloned().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--out" => out = Some(value()?),
            "--state" => state = Some(value()?),
            "--from" => from = Some(value()?),
            "--model" => model = value()?,
            other => return Err(format!("unknown atlas option `{other}`")),
        }
    }
    let out = out.ok_or_else(|| "--out is required".to_owned())?;
    let out = Path::new(&out);
    match command {
        "brief" => {
            let state = state.ok_or_else(|| "--state is required".to_owned())?;
            print!("{}", brief(out, &state)?);
            Ok(())
        }
        "accept" => {
            let state = state.ok_or_else(|| "--state is required".to_owned())?;
            let from = from.ok_or_else(|| "--from is required".to_owned())?;
            accept(out, &state, &read(Path::new(&from))?, &model)
        }
        "index" => index(out),
        _ => Err(format!("unknown atlas command `{command}`")),
    }
}

fn manifest(out: &Path) -> Result<Value, String> {
    serde_json::from_str(&read(&out.join("manifest.json"))?)
        .map_err(|error| format!("manifest.json: {error}"))
}

fn find_state<'a>(manifest: &'a Value, id: &str) -> Result<&'a Value, String> {
    manifest["states"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|state| state["state"] == id)
        .ok_or_else(|| format!("state `{id}` is not in manifest.json"))
}

fn rect(node: &Value) -> String {
    node["bounds"]["pixels"]
        .as_array()
        .map(|values| {
            let v: Vec<String> = values.iter().map(|v| v.as_f64().unwrap_or(0.0).to_string()).collect();
            format!("({},{})–({},{})", v[0], v[1], v[2], v[3])
        })
        .unwrap_or_else(|| "sem retângulo".to_owned())
}

/// Plain-text context for one state: how to open it, its widgets, and what is stale.
pub fn brief(out: &Path, id: &str) -> Result<String, String> {
    let manifest = manifest(out)?;
    let state = find_state(&manifest, id)?;
    let tree: Value = serde_json::from_str(&read(&out.join(state["tree"].as_str().unwrap_or("")))?)
        .map_err(|error| error.to_string())?;
    let mut text = String::new();
    let _ = writeln!(text, "ESTADO: {id}");
    let _ = writeln!(text, "IDIOMA: {}  TEMA: {}", state["locale"], state["theme"]);
    let _ = writeln!(text, "TIPO: {}  ALVO: {}", state["kind"], state["target"]);
    let args: Vec<&str> = state["args"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let _ = writeln!(text, "COMO ABRIR: forja-app --locale {} --theme {} {}", state["locale"].as_str().unwrap_or(""), state["theme"].as_str().unwrap_or(""), args.join(" "));
    let _ = writeln!(text, "IMAGEM ANEXADA: {} (pixels; escala {} px por ponto)", state["png"].as_str().unwrap_or(""), tree["pixels_per_point"]);
    let same: Vec<&str> = manifest["states"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|other| other["same_as"] == id)
        .filter_map(|other| other["state"].as_str())
        .collect();
    if !same.is_empty() {
        let _ = writeln!(text, "ESTADOS COM PIXELS IDÊNTICOS (cubra-os nesta página): {}", same.join(", "));
    }
    let _ = writeln!(text, "\nELEMENTOS DA ÁRVORE (papel | nome | retângulo em px | ações | estado):");
    let mut nodes: Vec<&Value> = tree["nodes"].as_array().into_iter().flatten().collect();
    let corner = |node: &Value, index: usize| {
        node["bounds"]["pixels"][index].as_f64().unwrap_or(f64::MAX)
    };
    nodes.sort_by(|a, b| {
        (corner(a, 1), corner(a, 0))
            .partial_cmp(&(corner(b, 1), corner(b, 0)))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for node in nodes {
        let role = node["role"].as_str().unwrap_or("");
        let name = node["name"].as_str().unwrap_or("").trim();
        let actions: Vec<&str> = node["actions"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let interesting = role != "TextRun" && (!name.is_empty() || !actions.is_empty() || role == "Unknown");
        if !interesting || node["bounds"].is_null() {
            continue;
        }
        let mut flags = Vec::new();
        if node["disabled"] == true {
            flags.push("desativado".to_owned());
        }
        if let Some(toggled) = node["toggled"].as_str() {
            flags.push(format!("marcado={toggled}"));
        }
        let shown = if name.chars().all(|c| ('\u{E000}'..='\u{F8FF}').contains(&c)) && !name.is_empty() {
            "(ícone)".to_owned()
        } else if name.is_empty() {
            "(sem nome acessível)".to_owned()
        } else {
            format!("\"{name}\"")
        };
        let _ = writeln!(text, "- {role} | {shown} | {} | {} | {}", rect(node), actions.join(","), flags.join(","));
    }
    let page_path = out.join("pages").join(format!("{id}.md"));
    if let Ok(page) = std::fs::read_to_string(&page_path) {
        let _ = writeln!(text, "\nPÁGINA ATUAL (atualize-a; a captura ou a lista de elementos mudou):\n{page}");
    }
    let reference = id.splitn(3, '.').nth(2).map(|rest| format!("pt-BR.dark.{rest}"));
    if let Some(reference) = reference.filter(|reference| reference != id) {
        if let Ok(page) = std::fs::read_to_string(out.join("pages").join(format!("{reference}.md"))) {
            let _ = writeln!(text, "\nPÁGINA DE REFERÊNCIA ({reference}); descreva só o que muda nesta variante, mas liste todos os elementos:\n{page}");
        }
    }
    Ok(text)
}

/// Extracts the page from the model answer, stamps the front matter and saves it.
pub fn accept(out: &Path, id: &str, answer: &str, model: &str) -> Result<(), String> {
    let manifest = manifest(out)?;
    let state = find_state(&manifest, id)?;
    let clean = strip_ansi(answer);
    let start = clean.rfind("<<<PAGINA").ok_or_else(|| "answer has no <<<PAGINA marker".to_owned())?;
    let body = &clean[start + "<<<PAGINA".len()..];
    let end = body.find("PAGINA>>>").ok_or_else(|| "answer has no PAGINA>>> marker".to_owned())?;
    let body = body[..end].trim();
    if body.lines().count() < 8 || !body.contains('#') {
        return Err("page is too short".to_owned());
    }
    let body = strip_front_matter(body);
    let page = format!(
        "---\nstate: {id}\ncapture_sha: {}\nmodel: {model}\n---\n\n![Captura](../{})\n\n{body}\n",
        state["capture_sha"].as_str().unwrap_or(""),
        state["png"].as_str().unwrap_or("")
    );
    write(&out.join("pages").join(format!("{id}.md")), &page)
}

fn strip_front_matter(body: &str) -> &str {
    if let Some(rest) = body.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            return rest[end + 4..].trim_start();
        }
    }
    body
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Writes README.md (one row per state) and PROBLEMAS.md (every "Problemas" section).
fn index(out: &Path) -> Result<(), String> {
    let manifest = manifest(out)?;
    let todo: Value = read(&out.join("todo.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    let mut readme = String::from("# Atlas da Forja\n\nCada tela da Forja, capturada pelo `cargo xtask atlas capture` e documentada pelo agente do atlas. Gerado automaticamente.\n\n");
    let _ = writeln!(
        readme,
        "**Progresso:** {}/{} telas documentadas · elementos citados {}/{} · completo: {}\n",
        todo["states_done"], todo["states_total"], todo["widgets_covered"], todo["widgets_total"], todo["complete"]
    );
    readme.push_str("| Estado | Página | Captura |\n|---|---|---|\n");
    let mut problems = String::from("# Problemas encontrados pelo atlas\n\nJuntado das seções \"Problemas encontrados\" de cada página. Gerado automaticamente.\n\n");
    for state in manifest["states"].as_array().into_iter().flatten() {
        let id = state["state"].as_str().unwrap_or("");
        let page_path = out.join("pages").join(format!("{id}.md"));
        let page_cell = match state["same_as"].as_str() {
            Some(other) => format!("igual a [{other}](pages/{other}.md)"),
            None if page_path.exists() => format!("[página](pages/{id}.md)"),
            None => "—".to_owned(),
        };
        let _ = writeln!(readme, "| `{id}` | {page_cell} | [png]({}) |", state["png"].as_str().unwrap_or(""));
        if let Ok(page) = std::fs::read_to_string(&page_path) {
            if let Some(section) = section(&page, "Problemas encontrados") {
                if section.lines().any(|line| line.trim_start().starts_with("- ")) {
                    let _ = writeln!(problems, "## [{id}](pages/{id}.md)\n\n{section}\n");
                }
            }
        }
    }
    write(&out.join("README.md"), &readme)?;
    write(&out.join("PROBLEMAS.md"), &problems)
}

fn section<'a>(page: &'a str, title: &str) -> Option<&'a str> {
    let marker = format!("## {title}");
    let start = page.find(&marker)? + marker.len();
    let rest = &page[start..];
    let end = rest.find("\n## ").unwrap_or(rest.len());
    Some(rest[..end].trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_removes_color_codes() {
        assert_eq!(strip_ansi("\u{1b}[0m> ok\u{1b}[32m!"), "> ok!");
    }

    #[test]
    fn section_stops_at_next_heading() {
        let page = "# T\n## Problemas encontrados\n- a\n- b\n## Outra\nx";
        assert_eq!(section(page, "Problemas encontrados"), Some("- a\n- b"));
    }

    #[test]
    fn front_matter_from_model_is_replaced() {
        assert_eq!(strip_front_matter("---\nstate: x\n---\n# T"), "# T");
        assert_eq!(strip_front_matter("# T"), "# T");
    }
}
