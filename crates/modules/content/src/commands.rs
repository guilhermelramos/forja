//! Comandos e métodos RPC do módulo `content`.

use crate::model::{ContentCommand, ContentRecipe};
use crate::state::{self, ContentCore};
use forja_kernel::{Args, Ctx, KernelError, Registrar};
use forja_kernel::forja_protocol::{Method, MethodEffect, ParamKind, ParamSchema, ResultSchema};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Registra comandos, métodos RPC e o contrato de conteúdo.
pub fn register(r: &mut Registrar<'_>, core: &Arc<ContentCore>) {
    let list_core = Arc::clone(core);
    r.command("content.commands.list", "content.command.list.title", move |cx, args| {
        commands_list(&list_core, cx, &args)
    });
    let run_core = Arc::clone(core);
    r.command("content.run", "content.command.run.title", move |cx, args| {
        run(&run_core, cx, &args)
    });
    let reload_core = Arc::clone(core);
    r.command("content.reload", "content.command.reload.title", move |cx, _| {
        let project = state::current_project(cx);
        state::load(&reload_core, cx, project);
        Ok(json!({ "reloaded": true, "revision": revision(&reload_core) }))
    });
    let themes_core = Arc::clone(core);
    r.command("content.themes.list", "content.command.themes.title", move |cx, args| {
        ensure_project(&themes_core, cx, &args)?;
        let locale = locale(cx);
        let themes = themes_core
            .with(|state| {
                state
                    .themes
                    .values()
                    .map(|theme| {
                        json!({
                            "id": theme.id,
                            "name": theme.name,
                            "kind": theme.kind,
                            "toml": theme.toml,
                            "layer": theme.layer,
                            "source_path": theme.source_path,
                            "content_hash": theme.content_hash,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(json!({
            "revision": revision(&themes_core),
            "locale": locale,
            "themes": themes,
        }))
    });
    let ui_core = Arc::clone(core);
    r.command("content.themes.ui", "content.command.themes.title", move |cx, args| {
        ensure_project(&ui_core, cx, &args)?;
        let mode = args.str("mode").unwrap_or("dark");
        let toml = ui_core
            .with(|state| {
                let mut selected: Option<String> = None;
                for theme in state.themes.values().filter(|theme| theme.kind == "interface") {
                    if selected.is_none() {
                        selected = Some(theme.toml.clone());
                    }
                    if theme.toml.contains(&format!("mode = \"{mode}\"")) {
                        selected = Some(theme.toml.clone());
                        break;
                    }
                }
                selected
            })
            .flatten();
        Ok(json!({ "revision": revision(&ui_core), "toml": toml }))
    });
    let recipes_core = Arc::clone(core);
    r.command("content.recipes.list", "content.command.recipes.title", move |cx, args| {
        ensure_project(&recipes_core, cx, &args)?;
        let recipes = recipes_core
            .with(|state| {
                state
                    .recipes
                    .values()
                    .map(|recipe| {
                        json!({
                            "id": recipe.id,
                            "title": recipe.title,
                            "command": recipe.command,
                            "scope": recipe.scope,
                            "enter": recipe.enter,
                            "directory": recipe.directory,
                            "manifest": recipe.manifest,
                            "layer": recipe.layer,
                            "source_path": recipe.source_path,
                            "content": recipe.content,
                            "content_hash": recipe.content_hash,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(json!({
            "revision": revision(&recipes_core),
            "recipes": recipes,
        }))
    });
    let templates_core = Arc::clone(core);
    r.command("content.templates.list", "content.command.templates.title", move |cx, _| {
        state::ensure(&templates_core, cx, false);
        let templates = templates_core
            .with(|state| {
                state
                    .templates
                    .values()
                    .map(|template| {
                        json!({
                            "id": template.id,
                            "name": template.name,
                            "use": template.use_kind,
                            "layer": template.layer,
                            "source_path": template.source_path,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(json!({ "templates": templates }))
    });
    let get_core = Arc::clone(core);
    r.command("content.templates.get", "content.command.templates.title", move |cx, args| {
        let id = args.required_str("id")?.to_owned();
        state::ensure(&get_core, cx, false);
        let body = get_core
            .with(|state| state.templates.get(&id).map(|template| template.body.clone()))
            .flatten()
            .ok_or_else(|| KernelError::message(format!("unknown template `{id}`")))?;
        Ok(json!({ "id": id, "body": body }))
    });
    let render_core = Arc::clone(core);
    r.command("content.templates.render", "content.command.templates.title", move |cx, args| {
        let id = args.required_str("id")?.to_owned();
        state::ensure(&render_core, cx, false);
        let body = render_core
            .with(|state| state.templates.get(&id).map(|template| template.body.clone()))
            .flatten()
            .ok_or_else(|| KernelError::message(format!("unknown template `{id}`")))?;
        let fields = args.get("fields").and_then(Value::as_object);
        let mut text = body;
        if let Some(fields) = fields {
            for (key, value) in fields {
                if let Some(value) = value.as_str() {
                    text = text.replace(&format!("{{{{{key}}}}}"), value);
                }
            }
        }
        Ok(json!({ "id": id, "text": text }))
    });

    register_rpc(r, core);
}

fn register_rpc(r: &mut Registrar<'_>, core: &Arc<ContentCore>) {
    let list_core = Arc::clone(core);
    r.rpc(
        Method::new("content.commands.list", "content.method.commands.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(
                ParamSchema::optional("scope", ParamKind::String)
                    .describe("anvil, chat or all (default all)."),
            )
            .result(ResultSchema::new("content.method.commands.result")),
        move |cx, params| {
            commands_list(&list_core, cx, &Args::from_map(params))
                .map_err(|error| forja_kernel::forja_protocol::RpcError::app(error.to_string()))
        },
    );
    let reload_core = Arc::clone(core);
    r.rpc(
        Method::new("content.reload", "content.method.reload.description")
            .effect(MethodEffect::Write)
            .idempotent(true)
            .result(ResultSchema::new("content.method.reload.result")),
        move |cx, _| {
            let project = state::current_project(cx);
            state::load(&reload_core, cx, project);
            Ok(json!({ "reloaded": true, "revision": revision(&reload_core) }))
        },
    );
    let themes_core = Arc::clone(core);
    r.rpc(
        Method::new("content.themes.list", "content.method.themes.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::optional("project_path", ParamKind::String))
            .result(ResultSchema::new("content.method.themes.result")),
        move |cx, params| {
            ensure_project(&themes_core, cx, &Args::from_map(params))
                .map_err(|error| forja_kernel::forja_protocol::RpcError::app(error.to_string()))?;
            let themes = themes_core
                .with(|state| {
                    state
                        .themes
                        .values()
                        .map(|theme| json!({
                            "id": theme.id,
                            "name": theme.name,
                            "kind": theme.kind,
                            "toml": theme.toml,
                            "layer": theme.layer,
                            "source_path": theme.source_path,
                            "content_hash": theme.content_hash,
                        }))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Ok(json!({ "revision": revision(&themes_core), "themes": themes }))
        },
    );
    let templates_core = Arc::clone(core);
    r.rpc(
        Method::new("content.templates.list", "content.method.templates.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .result(ResultSchema::new("content.method.templates.result")),
        move |cx, _| {
            state::ensure(&templates_core, cx, false);
            let templates = templates_core
                .with(|state| {
                    state
                        .templates
                        .values()
                        .map(|template| json!({
                            "id": template.id,
                            "name": template.name,
                            "use": template.use_kind,
                            "layer": template.layer,
                            "source_path": template.source_path,
                        }))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Ok(json!({ "templates": templates }))
        },
    );
    let render_core = Arc::clone(core);
    r.rpc(
        Method::new("content.templates.render", "content.method.templates.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::required("id", ParamKind::String))
            .param(ParamSchema::optional("fields", ParamKind::Json))
            .result(ResultSchema::new("content.method.templates.result")),
        move |cx, params| {
            let id = params
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| forja_kernel::forja_protocol::RpcError::invalid_params("id is required"))?;
            state::ensure(&render_core, cx, false);
            let body = render_core
                .with(|state| state.templates.get(id).map(|template| template.body.clone()))
                .flatten()
                .ok_or_else(|| forja_kernel::forja_protocol::RpcError::app(format!("unknown template `{id}`")))?;
            let mut text = body;
            if let Some(fields) = params.get("fields").and_then(Value::as_object) {
                for (key, value) in fields {
                    if let Some(value) = value.as_str() {
                        text = text.replace(&format!("{{{{{key}}}}}"), value);
                    }
                }
            }
            Ok(json!({ "id": id, "text": text }))
        },
    );
    let recipes_core = Arc::clone(core);
    r.rpc(
        Method::new("content.recipes.list", "content.method.recipes.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::optional("project_path", ParamKind::String))
            .result(ResultSchema::new("content.method.recipes.result")),
        move |cx, params| {
            ensure_project(&recipes_core, cx, &Args::from_map(params))
                .map_err(|error| forja_kernel::forja_protocol::RpcError::app(error.to_string()))?;
            let recipes = recipes_core
                .with(|state| {
                    state
                        .recipes
                        .values()
                        .map(|recipe| json!({
                            "id": recipe.id,
                            "title": recipe.title,
                            "command": recipe.command,
                            "scope": recipe.scope,
                            "enter": recipe.enter,
                            "directory": recipe.directory,
                            "manifest": recipe.manifest,
                            "layer": recipe.layer,
                            "source_path": recipe.source_path,
                            "content": recipe.content,
                            "content_hash": recipe.content_hash,
                        }))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Ok(json!({ "revision": revision(&recipes_core), "recipes": recipes }))
        },
    );
}

fn revision(core: &ContentCore) -> u64 {
    core.with(|state| state.revision).unwrap_or(0)
}

fn locale(cx: &Ctx<'_>) -> String {
    cx.i18n().locale().id().to_owned()
}

fn ensure_project(core: &ContentCore, cx: &mut Ctx<'_>, args: &Args) -> forja_kernel::Result<()> {
    if let Some(path) = args.str("project_path").map(PathBuf::from) {
        let current = core.with(|state| state.project.clone()).flatten();
        if current.as_deref() != Some(path.as_path()) {
            state::load(core, cx, Some(path));
        }
        Ok(())
    } else {
        state::ensure(core, cx, false);
        Ok(())
    }
}

/// Lista de comandos com o estado de trust de cada um.
pub fn commands_list(core: &ContentCore, cx: &mut Ctx<'_>, args: &Args) -> forja_kernel::Result<Value> {
    ensure_project(core, cx, args)?;
    let scope = args.str("scope").unwrap_or("all");
    let locale = locale(cx);
    let mut rows = Vec::new();
    let commands = core
        .with(|state| state.commands.values().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    for mut command in commands {
        if scope != "all" && !command.scope.iter().any(|value| value == scope) {
            continue;
        }
        if let Some(reason) = blocked_reason(cx, &command) {
            command.blocked = true;
            command.block_reason = Some(reason);
        }
        rows.push(json!({
            "id": command.id,
            "name": command.name,
            "title": command.title,
            "description": command.description,
            "scope": command.scope,
            "run": command.run,
            "helper": command.helper,
            "permissions": command.permissions,
            "context": command.context,
            "shortcut": command.shortcut,
            "arguments": command.arguments,
            "body": command.body,
            "layer": command.layer,
            "source_path": command.source_path,
            "content_hash": command.content_hash,
            "blocked": command.blocked,
            "block_reason": command.block_reason,
            "title_text": command.title.resolve(&locale),
        }));
    }
    rows.sort_by(|a, b| {
        let left = a.get("title_text").and_then(Value::as_str).unwrap_or("");
        let right = b.get("title_text").and_then(Value::as_str).unwrap_or("");
        left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase())
    });
    let diagnostics = core
        .with(|state| state.diagnostics.clone())
        .unwrap_or_default();
    Ok(json!({
        "revision": revision(core),
        "commands": rows,
        "diagnostics": diagnostics,
    }))
}

fn blocked_reason(cx: &mut Ctx<'_>, command: &ContentCommand) -> Option<String> {
    if !command.is_risky() {
        return None;
    }
    let mut args = Args::new();
    args.insert("kind", json!("command"));
    args.insert("source_path", json!(command.source_path));
    args.insert("content_hash", json!(command.content_hash));
    match cx.run_command("trust.check", args) {
        Ok(value) if value.get("approved").and_then(Value::as_bool) == Some(true) => None,
        _ => Some("trust".to_owned()),
    }
}

/// Executa um comando de conteúdo.
pub fn run(core: &ContentCore, cx: &mut Ctx<'_>, args: &Args) -> forja_kernel::Result<Value> {
    state::ensure(core, cx, false);
    let id = args.required_str("id")?;
    let id = if id.starts_with("cmd.") {
        id.to_owned()
    } else {
        format!("cmd.{id}")
    };
    let command = core
        .with(|state| state.commands.get(&id).cloned())
        .flatten()
        .ok_or_else(|| KernelError::message(cx.t_args("content.error.unknown-command", &[("id", &id)])))?;
    if let Some(reason) = blocked_reason(cx, &command) {
        let _ = reason;
        request_trust(
            cx,
            "command",
            &command.source_path,
            &command.content,
            format!("command {} from a project folder", command.id),
        );
        return Err(KernelError::message(
            cx.t_args("content.error.trust", &[("id", &id)]),
        ));
    }
    let mut arguments = BTreeMap::new();
    if let Some(values) = args.get("arguments").and_then(Value::as_object) {
        for (key, value) in values {
            if let Some(value) = value.as_str() {
                arguments.insert(key.clone(), value.to_owned());
            }
        }
    }
    let context = build_context(cx, &command);
    let (text, warnings) = command.render(&arguments, &context);
    let result = match command.run.as_str() {
        "terminal" => {
            send_terminal(cx, &text, true)?;
            json!({ "started": true })
        }
        "recipe" => {
            let recipe_id = command.recipe.clone().ok_or_else(|| {
                KernelError::message(format!("command `{}` has no `recipe` id", command.id))
            })?;
            let recipe = core
                .with(|state| state.recipes.get(&recipe_id).cloned())
                .flatten()
                .ok_or_else(|| KernelError::message(format!("unknown recipe `{recipe_id}`")))?;
            ensure_recipe_trust(cx, &recipe)?;
            send_terminal(cx, &recipe.command, recipe.enter)?;
            json!({ "started": true, "recipe": recipe.id })
        }
        _ => {
            let bench = state::current_project(cx);
            let mut launch = Args::new();
            if let Some(helper) = &command.helper {
                launch.insert("profile_id", json!(helper));
            }
            if let Some(bench) = &bench {
                launch.insert("bench_path", json!(bench.display().to_string()));
            }
            launch.insert("prompt", json!(text));
            launch.insert("permission", json!(command.permissions));
            let response = cx.run_command("helper.launch", launch)?;
            json!({ "started": true, "helper": response })
        }
    };
    Ok(json!({
        "id": command.id,
        "run": command.run,
        "warnings": warnings,
        "result": result,
    }))
}

fn ensure_recipe_trust(cx: &mut Ctx<'_>, recipe: &ContentRecipe) -> forja_kernel::Result<()> {
    if recipe.layer != crate::model::Layer::Project {
        return Ok(());
    }
    let mut args = Args::new();
    args.insert("kind", json!("recipe"));
    args.insert("source_path", json!(recipe.source_path));
    args.insert("content_hash", json!(recipe.content_hash));
    let approved = cx
        .run_command("trust.check", args)
        .ok()
        .and_then(|value| value.get("approved").and_then(Value::as_bool))
        .unwrap_or(false);
    if approved {
        Ok(())
    } else {
        request_trust(
            cx,
            "recipe",
            &recipe.source_path,
            &recipe.content,
            format!("recipe {} from a project folder", recipe.id),
        );
        Err(KernelError::message(format!(
            "[trust] recipe `{}` needs approval",
            recipe.id
        )))
    }
}

/// Asks `trust` to open the approval for one content file.
fn request_trust(cx: &mut Ctx<'_>, kind: &str, path: &str, content: &str, summary: String) {
    let review = cx
        .setting("content.trust.review_on_change")
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    if !review {
        return;
    }
    let mut args = Args::new();
    args.insert("kind", json!(kind));
    args.insert("source_path", json!(path));
    args.insert("content", json!(content));
    args.insert("summary", json!(summary));
    let _ = cx.run_command("trust.request", args);
}

fn send_terminal(cx: &mut Ctx<'_>, text: &str, enter: bool) -> forja_kernel::Result<Value> {
    let bench = state::current_project(cx);
    let mut list = Args::new();
    if let Some(bench) = &bench {
        list.insert("bench_path", json!(bench.display().to_string()));
    }
    let mut terminal_id = cx
        .run_command("terminal.list", list)
        .ok()
        .and_then(|value| {
            value
                .get("terminals")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("terminal_id"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    if terminal_id.is_none() {
        let mut open = Args::new();
        if let Some(bench) = &bench {
            open.insert("cwd", json!(bench.display().to_string()));
        }
        let opened = cx.run_command("terminal.open", open)?;
        terminal_id = opened
            .get("terminal_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
    }
    let Some(terminal_id) = terminal_id else {
        return Err(KernelError::message("no terminal is available"));
    };
    let mut send = Args::new();
    send.insert("terminal_id", json!(terminal_id));
    send.insert("text", json!(text));
    send.insert("enter", json!(enter));
    cx.run_command("terminal.send", send)
}

fn build_context(cx: &mut Ctx<'_>, command: &ContentCommand) -> BTreeMap<String, String> {
    let mut context = BTreeMap::new();
    let bench = cx.run_command("bench.current", Args::new()).ok();
    if let Some(bench) = &bench {
        let name = bench.get("name").and_then(Value::as_str).unwrap_or_default();
        let branch = bench.get("branch").and_then(Value::as_str).unwrap_or_default();
        let path = bench.get("path").and_then(Value::as_str).unwrap_or_default();
        context.insert("bench.name".to_owned(), name.to_owned());
        context.insert("bench.branch".to_owned(), branch.to_owned());
        context.insert("bench.path".to_owned(), path.to_owned());
    }
    for source in &command.context {
        match source.as_str() {
            "diff" => {
                if let Some(path) = bench.as_ref().and_then(|value| value.get("path")).and_then(Value::as_str) {
                    let mut args = Args::new();
                    args.insert("path", json!(path));
                    if let Ok(changes) = cx.run_command("git.changes", args) {
                        let summary = changes.get("summary").cloned().unwrap_or(Value::Null);
                        context.insert("diff.summary".to_owned(), summary.to_string());
                    }
                }
            }
            "bench" => {
                if let Some(bench) = &bench {
                    context.insert("bench".to_owned(), bench.to_string());
                }
            }
            value if value.starts_with("files:") => {
                let glob = value.trim_start_matches("files:");
                let root = bench
                    .as_ref()
                    .and_then(|value| value.get("path"))
                    .and_then(Value::as_str)
                    .map(PathBuf::from);
                if let Some(root) = root {
                    context.insert(
                        format!("files:{glob}"),
                        read_glob(&root, glob, 12_000).unwrap_or_default(),
                    );
                }
            }
            _ => {}
        }
    }
    context
}

fn read_glob(root: &Path, glob: &str, limit: usize) -> Option<String> {
    let mut text = String::new();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .max_depth(Some(12))
        .build();
    for entry in walker.flatten() {
        if text.len() >= limit {
            break;
        }
        let path = entry.path();
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        if !glob_match(glob, &relative) {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(path) {
            let remaining = limit.saturating_sub(text.len());
            text.push_str(&format!("--- {relative}\n"));
            text.push_str(&content.chars().take(remaining).collect::<String>());
            text.push('\n');
        }
    }
    Some(text)
}

/// Correspondência simples de glob com `**`, `*` e `?`.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    fn inner(pattern: &[u8], text: &[u8]) -> bool {
        if pattern.is_empty() {
            return text.is_empty();
        }
        match pattern[0] {
            b'*' => {
                if pattern.get(1) == Some(&b'*') {
                    let rest = pattern.get(2..).unwrap_or_default();
                    let rest = rest.strip_prefix(b"/").unwrap_or(rest);
                    return (0..=text.len()).any(|index| inner(rest, &text[index..]));
                }
                let mut index = 0;
                loop {
                    if inner(&pattern[1..], &text[index..]) {
                        return true;
                    }
                    if index >= text.len() || text[index] == b'/' {
                        return false;
                    }
                    index += 1;
                }
            }
            b'?' => !text.is_empty() && inner(&pattern[1..], &text[1..]),
            byte => !text.is_empty() && text[0] == byte && inner(&pattern[1..], &text[1..]),
        }
    }
    inner(pattern.as_bytes(), text.as_bytes())
}

/// Hash do arquivo exato de um comando.
#[cfg(test)]
fn command_hash(command: &ContentCommand) -> String {
    crate::model::sha256_hex(&command.content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matches_recursive_patterns() {
        assert!(glob_match("**/*.rs", "src/main.rs"));
        assert!(glob_match("**/*.rs", "main.rs"));
        assert!(glob_match("src/*.rs", "src/main.rs"));
        assert!(!glob_match("src/*.rs", "src/deep/main.rs"));
        assert!(!glob_match("**/*.ts", "src/main.rs"));
    }

    #[test]
    fn command_hash_is_the_file_hash() {
        let text = "+++\ntitle = \"x\"\n+++\nbody\n";
        let command = crate::model::parse_command(Path::new("x.md"), crate::model::Layer::User, text).unwrap();
        assert_eq!(command_hash(&command), crate::model::sha256_hex(text));
    }
}
