//! Módulo `editor` da Forja.
//!
//! Dono do comando `file.open` e assinante de `file.open_requested`. Mantém
//! abas de arquivo na bancada, destaque com `syntect`/`two-face`, localizar e
//! substituir, salvamento atômico, formatação e prévia delegada ao módulo
//! `viewers`.
//!
//! Contratos em `MODULE.md` e em `design/contracts.md`.

mod commands;
mod edit;
mod fonts;
mod highlight;
mod model;
mod state;
mod symbols;
mod view;
mod view_error;

use crate::state::Shared;
use forja_contracts::{names, FileOpenRequested};
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::{
    Ctx, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot,
};
use serde_json::{json, Value};

#[forja_kernel::module]
static EDITOR: fn() -> Box<dyn Module> = || Box::new(EditorModule::new());

pub struct EditorModule {
    state: Shared,
}

impl EditorModule {
    pub fn new() -> Self {
        Self {
            state: Shared::new(),
        }
    }
}

impl Default for EditorModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for EditorModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("editor", "editor.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        r.contract_namespace("file");
        register_settings(r);

        r.contract_event::<FileOpenRequested>(names::FILE_OPEN_REQUESTED);
        let event_state = self.state.clone();
        r.subscribe(move |cx, event: &FileOpenRequested| {
            let line = event.line.map(i64::from);
            commands::open_path(cx, &event_state, &event.path, line, false);
        });

        register_commands(r, &self.state);
        edit::register(r, &self.state);
        register_rpc(r, &self.state);

        for key in used_keys() {
            r.uses_i18n(key);
        }

        let view_state = self.state.clone();
        r.view(
            Slot::BenchTab,
            "editor.tab",
            "editor.tab.title",
            bench_tab_order(16),
            move || Box::new(view::EditorView::new(view_state.clone())),
        );
    }
}

fn register_commands(r: &mut Registrar<'_>, shared: &Shared) {
    let open = shared.clone();
    r.command("file.open", "editor.command.open", move |cx, args| {
        let path = args.required_str("path")?.to_owned();
        let line = args.i64("line");
        commands::open_path(cx, &open, &path, line, true);
        Ok(json!({}))
    });

    let active = shared.clone();
    r.command("editor.active_file", "editor.command.active_file", move |_cx, _args| {
        Ok(json!({ "path": active.active_tab().map(|tab| tab.path) }))
    });

    let save = shared.clone();
    r.command("editor.save", "editor.command.save", move |cx, args| {
        let path = commands::active_path(cx, &save, &args)?;
        let root = commands::current_bench_root(cx)
            .ok_or_else(|| forja_kernel::KernelError::Message(cx.t("editor.error.no-bench")))?;
        commands::save(cx, &save, &path, root);
        Ok(json!({ "pending": true }))
    });

    let reload = shared.clone();
    r.command("editor.reload", "editor.command.reload", move |cx, args| {
        let path = commands::active_path(cx, &reload, &args)?;
        commands::reload(cx, &reload, &path);
        Ok(json!({ "pending": true }))
    });

    let compare = shared.clone();
    r.command("editor.compare", "editor.command.compare", move |cx, args| {
        let path = commands::active_path(cx, &compare, &args)?;
        commands::compare(cx, &compare, &path);
        Ok(json!({ "pending": true }))
    });

    let preview = shared.clone();
    r.command("editor.preview", "editor.command.preview", move |cx, args| {
        let path = commands::active_path(cx, &preview, &args)?;
        commands::preview(cx, &preview, &path);
        Ok(json!({ "preview": true }))
    });

    let format = shared.clone();
    r.command("editor.format", "editor.command.format", move |cx, args| {
        let path = commands::active_path(cx, &format, &args)?;
        commands::format_active(cx, &format, &path);
        Ok(json!({ "pending": true }))
    });

    r.command(
        "editor.open_external",
        "editor.command.open_external",
        move |cx, args| {
            let path = args.required_str("path")?;
            commands::open_external(cx, path);
            Ok(json!({ "pending": true }))
        },
    );

    let close = shared.clone();
    r.command("editor.close", "editor.command.close", move |_cx, args| {
        let index = args
            .str("path")
            .and_then(|path| {
                close
                    .with(|state| state.tabs.iter().position(|tab| tab.path == path))
                    .flatten()
            })
            .unwrap_or_else(|| close.active_index());
        close.close(index);
        Ok(json!({ "closed": true }))
    });

    let find = shared.clone();
    r.command("editor.find", "editor.command.find", move |cx, args| {
        commands::find(cx, &find, &args)
    });

    let replace_open = shared.clone();
    r.command("editor.replace", "editor.command.replace_all", move |cx, args| {
        let mut args = args;
        args.insert("replace", json!(true));
        commands::find(cx, &replace_open, &args)
    });

    let replace = shared.clone();
    r.command("editor.replace.all", "editor.command.replace_all", move |cx, args| {
        commands::replace_all(cx, &replace, &args)
    });

    let next = shared.clone();
    r.command("editor.find.next", "editor.command.find_next", move |cx, args| {
        let _ = commands::active_path(cx, &next, &args)?;
        let index = next.with(|state| {
            state.find.open = true;
            state.find.next();
            let current = state.find.current;
            state.pending_match = true;
            (current, state.find.matches.len())
        });
        Ok(json!({ "index": index.map(|pair| pair.0), "matches": index.map(|pair| pair.1) }))
    });

    let previous = shared.clone();
    r.command(
        "editor.find.previous",
        "editor.command.find_previous",
        move |cx, args| {
            let _path = commands::active_path(cx, &previous, &args)?;
            let index = previous.with(|state| {
                state.find.open = true;
                state.find.previous();
                let current = state.find.current;
                state.pending_match = true;
                (current, state.find.matches.len())
            });
            Ok(json!({
                "index": index.map(|pair| pair.0),
                "matches": index.map(|pair| pair.1)
            }))
        },
    );

    let goto = shared.clone();
    r.command("editor.goto.line", "editor.command.goto", move |_cx, args| {
        if let Some(line) = args.i64("line") {
            goto.with(|state| state.pending_line = Some(line.max(1) as usize));
        }
        Ok(json!({ "open": true }))
    });

    let wrap = shared.clone();
    r.command("editor.wrap.toggle", "editor.command.wrap", move |cx, args| {
        let path = commands::active_path(cx, &wrap, &args)?;
        let value = commands::wrap_toggle(cx, &wrap, &path);
        Ok(json!({ "wrap": value }))
    });

    register_line_commands(r, shared);
    register_external_check(r, shared);
}

fn register_line_commands(r: &mut Registrar<'_>, shared: &Shared) {
    let indent = shared.clone();
    r.command("editor.indent", "editor.command.indent", move |cx, args| {
        let path = commands::active_path(cx, &indent, &args)?;
        let unit = indent_unit(cx, &indent, &path);
        commands::edit_active(
            cx,
            &indent,
            &path,
            move |text, line| Some(model::change_indent(text, line, line, &unit, false)),
            "editor.status.indented",
        );
        Ok(json!({}))
    });

    let outdent = shared.clone();
    r.command("editor.outdent", "editor.command.outdent", move |cx, args| {
        let path = commands::active_path(cx, &outdent, &args)?;
        let unit = indent_unit(cx, &outdent, &path);
        commands::edit_active(
            cx,
            &outdent,
            &path,
            move |text, line| Some(model::change_indent(text, line, line, &unit, true)),
            "editor.status.outdented",
        );
        Ok(json!({}))
    });

    let up = shared.clone();
    r.command("editor.move_line.up", "editor.command.move_up", move |cx, args| {
        let path = commands::active_path(cx, &up, &args)?;
        commands::edit_active(cx, &up, &path, |text, line| Some(model::move_line(text, line, true)), "editor.status.moved");
        Ok(json!({}))
    });

    let down = shared.clone();
    r.command("editor.move_line.down", "editor.command.move_down", move |cx, args| {
        let path = commands::active_path(cx, &down, &args)?;
        commands::edit_active(cx, &down, &path, |text, line| Some(model::move_line(text, line, false)), "editor.status.moved");
        Ok(json!({}))
    });

    let duplicate = shared.clone();
    r.command("editor.duplicate_line", "editor.command.duplicate", move |cx, args| {
        let path = commands::active_path(cx, &duplicate, &args)?;
        commands::edit_active(cx, &duplicate, &path, |text, line| Some(model::duplicate_line(text, line)), "editor.status.duplicated");
        Ok(json!({}))
    });

    let delete = shared.clone();
    r.command("editor.delete_line", "editor.command.delete", move |cx, args| {
        let path = commands::active_path(cx, &delete, &args)?;
        commands::edit_active(cx, &delete, &path, |text, line| Some(model::delete_line(text, line)), "editor.status.deleted");
        Ok(json!({}))
    });

    let comment = shared.clone();
    r.command("editor.comment.toggle", "editor.command.comment", move |cx, args| {
        let path = commands::active_path(cx, &comment, &args)?;
        let prefix = commands::comment_prefix(&path);
        commands::edit_active(
            cx,
            &comment,
            &path,
            move |text, line| Some(model::toggle_comment(text, line, line, prefix)),
            "editor.status.comment",
        );
        Ok(json!({}))
    });
}

fn register_external_check(r: &mut Registrar<'_>, shared: &Shared) {
    let check = shared.clone();
    r.command(
        "editor.check_external",
        "editor.command.check_external",
        move |cx, _args| {
            commands::check_external(cx, &check);
            Ok(json!({ "pending": true }))
        },
    );
}

fn register_rpc(r: &mut Registrar<'_>, shared: &Shared) {
    let open = shared.clone();
    r.rpc(
        Method::new("file.open", "editor.method.open.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("path", ParamKind::String).alias("caminho"))
            .param(ParamSchema::optional("line", ParamKind::Integer).alias("linha"))
            .param(ParamSchema::optional("column", ParamKind::Integer).alias("coluna"))
            .result(ResultSchema::new("editor.method.open.result")),
        move |cx, params| {
            let path = required(&params, "path")?;
            let line = params.get("line").and_then(Value::as_i64);
            commands::open_path(cx, &open, path, line, true);
            Ok(json!({}))
        },
    );

    let save = shared.clone();
    r.rpc(
        Method::new("editor.save", "editor.method.save.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::optional("path", ParamKind::String).alias("caminho"))
            .result(ResultSchema::new("editor.method.save.result")),
        move |cx, params| {
            let path = params
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| save.active_tab().map(|tab| tab.path))
                .ok_or_else(|| RpcError::invalid_params("no open file"))?;
            let root = commands::current_bench_root(cx)
                .ok_or_else(|| RpcError::invalid_params("no active bench"))?;
            commands::save(cx, &save, &path, root);
            Ok(json!({ "pending": true }))
        },
    );

    let preview = shared.clone();
    r.rpc(
        Method::new("editor.preview", "editor.method.preview.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::required("path", ParamKind::String).alias("caminho"))
            .result(ResultSchema::new("editor.method.preview.result")),
        move |cx, params| {
            let path = required(&params, "path")?;
            commands::preview(cx, &preview, path);
            Ok(json!({ "preview": true }))
        },
    );
}

fn register_settings(r: &mut Registrar<'_>) {
    use SettingKind::*;
    let settings = [
        ("editor.font.family", Choice { options: choices(&["inherit", "cascadia_code", "cascadia_mono", "consolas", "jetbrains_mono"]) }, SettingValue::Text("inherit".into())),
        ("editor.font.size", Integer { min: Some(8), max: Some(32) }, SettingValue::Integer(13)),
        ("editor.line.height", Number { min: Some(1.0), max: Some(1.8) }, SettingValue::Number(1.4)),
        ("editor.gutter.line_numbers", Bool, SettingValue::Bool(true)),
        ("editor.indent.auto_detect", Bool, SettingValue::Bool(true)),
        ("editor.indent.size", Integer { min: Some(1), max: Some(8) }, SettingValue::Integer(4)),
        ("editor.indent.tabs", Choice { options: choices(&["spaces", "tabs"]) }, SettingValue::Text("spaces".into())),
        ("editor.wrap.mode", Choice { options: choices(&["off", "column", "window"]) }, SettingValue::Text("off".into())),
        ("editor.wrap.column", Integer { min: Some(40), max: Some(200) }, SettingValue::Integer(100)),
        ("editor.highlight.current_line", Bool, SettingValue::Bool(true)),
        ("editor.view.minimap", Bool, SettingValue::Bool(false)),
        ("editor.brackets.guides", Bool, SettingValue::Bool(true)),
        ("editor.whitespace.show", Choice { options: choices(&["never", "selection", "always"]) }, SettingValue::Text("selection".into())),
        ("editor.save.auto", Choice { options: choices(&["off", "delay", "focus_change"]) }, SettingValue::Text("off".into())),
        ("editor.save.delay_ms", Integer { min: Some(500), max: Some(10_000) }, SettingValue::Integer(1_500)),
        ("editor.save.format", Bool, SettingValue::Bool(false)),
        ("editor.save.trim_trailing", Bool, SettingValue::Bool(false)),
        ("editor.save.final_newline", Bool, SettingValue::Bool(true)),
        ("editor.files.encoding", Choice { options: choices(&["auto", "utf8", "utf8_bom", "utf16le"]) }, SettingValue::Text("auto".into())),
        ("editor.files.eol", Choice { options: choices(&["auto", "lf", "crlf"]) }, SettingValue::Text("auto".into())),
        ("editor.files.large_mb", Integer { min: Some(1), max: Some(100) }, SettingValue::Integer(10)),
        ("editor.search.regex", Bool, SettingValue::Bool(false)),
        ("editor.search.case_sensitive", Bool, SettingValue::Bool(false)),
        ("editor.search.whole_word", Bool, SettingValue::Bool(false)),
        ("editor.markdown.preview", Bool, SettingValue::Bool(true)),
    ];
    for (key, kind, default) in settings {
        let slug = key.replace('.', "-");
        r.setting(SettingSpec::new(
            key,
            kind,
            default,
            format!("setting-{slug}-label"),
            format!("setting-{slug}-help"),
        ));
    }
}

fn choices(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn indent_unit(cx: &Ctx<'_>, shared: &Shared, path: &str) -> String {
    let auto_detect = cx
        .setting("editor.indent.auto_detect")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true);
    if auto_detect {
        if let Some(detected) = shared
            .tab(path)
            .and_then(|tab| model::detect_indent(&tab.content))
        {
            return detected;
        }
    }
    let tabs = cx
        .setting("editor.indent.tabs")
        .and_then(SettingValue::as_str)
        == Some("tabs");
    if tabs {
        "\t".to_owned()
    } else {
        let size = cx
            .setting("editor.indent.size")
            .and_then(SettingValue::as_i64)
            .unwrap_or(4)
            .clamp(1, 8) as usize;
        " ".repeat(size)
    }
}

fn required<'a>(
    params: &'a serde_json::Map<String, Value>,
    name: &str,
) -> Result<&'a str, RpcError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_params(format!("missing `{name}`")))
}

fn bench_tab_order(default: i32) -> i32 {
    match std::env::var("FORJA_BENCH_TAB") {
        Ok(module) if module == "editor" => 1,
        _ => default,
    }
}

fn used_keys() -> Vec<&'static str> {
    vec![
        "editor.name",
        "editor.tab.title",
        "editor.tab.close",
        "editor.command.open",
        "editor.command.active_file",
        "editor.command.save",
        "editor.command.reload",
        "editor.command.compare",
        "editor.command.preview",
        "editor.command.format",
        "editor.command.open_external",
        "editor.command.close",
        "editor.command.find",
        "editor.command.replace_all",
        "editor.command.find_next",
        "editor.command.find_previous",
        "editor.command.goto",
        "editor.command.wrap",
        "editor.command.indent",
        "editor.command.outdent",
        "editor.command.move_up",
        "editor.command.move_down",
        "editor.command.duplicate",
        "editor.command.delete",
        "editor.command.comment",
        "editor.command.check_external",
        "editor.command.undo",
        "editor.command.redo",
        "editor.command.cut",
        "editor.command.copy",
        "editor.command.paste",
        "editor.command.select_all",
        "editor.command.quick_fix",
        "editor.command.quick_fix_apply",
        "editor.command.rename",
        "editor.command.goto_definition",
        "editor.command.peek_definition",
        "editor.command.completion",
        "editor.command.completion_apply",
        "editor.command.parameter_hint",
        "editor.error.no-tab",
        "editor.error.no-bench",
        "editor.view.empty",
        "editor.action.save",
        "editor.action.find",
        "editor.action.replace",
        "editor.action.goto",
        "editor.action.format",
        "editor.action.wrap-on",
        "editor.action.wrap-off",
        "editor.action.preview",
        "editor.action.edit",
        "editor.action.goto-go",
        "editor.action.rename",
        "editor.action.rename-apply",
        "editor.action.completion",
        "editor.goto.label",
        "editor.banner.external",
        "editor.banner.reload",
        "editor.banner.compare",
        "editor.banner.hint",
        "editor.find.hint",
        "editor.find.case",
        "editor.find.word",
        "editor.find.regex",
        "editor.find.replace-hint",
        "editor.find.replace-all",
        "editor.preview.loading",
        "editor.compare.title",
        "editor.compare.hint",
        "editor.status.line",
        "editor.status.matches",
        "editor.status.unsaved",
        "editor.status.saved",
        "editor.status.reloaded",
        "editor.status.viewer-missing",
        "editor.status.format-unsupported",
        "editor.status.formatted",
        "editor.status.format-failed",
        "editor.status.line-out-of-range",
        "editor.status.replaced",
        "editor.status.wrap-on",
        "editor.status.wrap-off",
        "editor.status.indented",
        "editor.status.outdented",
        "editor.status.moved",
        "editor.status.duplicated",
        "editor.status.deleted",
        "editor.status.comment",
        "editor.status.undo",
        "editor.status.undo-empty",
        "editor.status.redo",
        "editor.status.redo-empty",
        "editor.status.copied",
        "editor.status.cut",
        "editor.status.no-selection",
        "editor.status.copy-unavailable",
        "editor.status.paste-unavailable",
        "editor.status.no-symbol",
        "editor.status.definition",
        "editor.status.definition-missing",
        "editor.status.renamed",
        "editor.status.no-completions",
        "editor.status.no-call",
        "editor.status.quick-fixed",
        "editor.status.preview-off",
        "editor.peek.title",
        "editor.peek.line",
        "editor.peek.close",
        "editor.completion.title",
        "editor.completion.close",
        "editor.quick-fix.title",
        "editor.quick-fix.trim",
        "editor.quick-fix.newline",
        "editor.quick-fix.spaces",
        "editor.quick-fix.close",
        "editor.parameter.title",
        "editor.parameter.line",
        "editor.parameter.close",
        "editor.rename.title",
        "editor.rename.hint",
        "editor.method.open.description",
        "editor.method.open.result",
        "editor.method.save.description",
        "editor.method.save.result",
        "editor.method.preview.description",
        "editor.method.preview.result",
    ]
}

#[cfg(test)]
mod a11y_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Args, Module, ModuleManifest, Registrar, Runtime, RuntimeOptions};
    use serde_json::Map;
    use std::path::Path;
    use std::sync::Arc;

    struct TempGuard(std::path::PathBuf);

    impl std::ops::Deref for TempGuard {
        type Target = std::path::PathBuf;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_dir() -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-editor-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir)
    }

    fn runtime(dir: &Path) -> Runtime {
        Runtime::new(
            vec![Box::new(EditorModule::new())],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("editor registers cleanly")
    }

    struct FakeBench {
        path: String,
    }

    impl Module for FakeBench {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("bench", "bench.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            let path = self.path.clone();
            r.command("bench.current", "bench.command.current", move |_cx, _args| {
                Ok(json!({ "path": path }))
            });
        }
    }

    fn wait_until(runtime: &mut Runtime, done: impl Fn() -> bool) {
        let (sender, receiver) = std::sync::mpsc::channel();
        runtime.set_wake(Arc::new(move || { let _ = sender.send(()); }));
        loop {
            runtime.poll_tasks();
            if done() { return; }
            receiver.recv_timeout(std::time::Duration::from_secs(60)).expect("task did not signal completion");
        }
    }

    #[test]
    fn contract_holds_and_settings_match_the_catalog() {
        let dir = temp_dir();
        let runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let keys: Vec<&str> = runtime
            .registry()
            .settings()
            .iter()
            .filter(|spec| spec.key.starts_with("editor."))
            .map(|spec| spec.key.as_str())
            .collect();
        assert!(keys.contains(&"editor.save.final_newline"));
        assert!(keys.contains(&"editor.search.whole_word"));
        assert!(keys.len() >= 24);
    }

    #[test]
    fn file_open_loads_into_a_tab_and_saves_atomically() {
        let dir = temp_dir();
        let path = dir.join("nota.txt");
        std::fs::write(&path, "um\r\ndois\r\n").unwrap();
        let module = EditorModule::new();
        let shared = module.state.clone();
        let mut runtime = Runtime::new(
            vec![
                Box::new(module),
                Box::new(FakeBench { path: dir.display().to_string() }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .unwrap();
        let mut args = Args::new();
        args.insert("path", json!(path));
        runtime.run_command("file.open", args).unwrap();
        wait_until(&mut runtime, || shared.active_tab().is_some());
        {
            let tab = shared.active_tab().unwrap();
            assert_eq!(tab.content, "um\ndois\n");
            assert_eq!(tab.eol, model::Eol::Crlf);
        }
        shared.set_content(&path.to_string_lossy(), "um\ndois\ntres\n".to_owned());
        let mut args = Args::new();
        args.insert("path", json!(path));
        runtime.run_command("editor.save", args).unwrap();
        wait_until(&mut runtime, || !shared.active_tab().unwrap().dirty);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes, b"um\r\ndois\r\ntres\r\n");
    }

    #[test]
    fn file_open_rpc_accepts_the_contract_shape() {
        let dir = temp_dir();
        let path = dir.join("a.rs");
        std::fs::write(&path, "fn main() {}\n").unwrap();
        let mut runtime = runtime(&dir);
        let value = runtime
            .call_method(
                "file.open",
                Map::from_iter([("path".to_owned(), json!(path))]),
            )
            .unwrap();
        assert!(value.is_object());
    }

    #[test]
    fn line_deletion_can_be_undone_and_redone() {
        let dir = temp_dir();
        let module = EditorModule::new();
        let shared = module.state.clone();
        let mut runtime = runtime_with_module(&dir, module);
        let path = "C:\\temp\\line.txt";
        let original = "first\nsecond\n";
        shared.open(path.to_owned(), original.to_owned(), model::Eol::Lf, model::Encoding::Utf8, true);
        runtime.run_command("editor.delete_line", Args::new()).unwrap();
        assert_ne!(shared.active_tab().unwrap().content, original);
        runtime.run_command("editor.undo", Args::new()).unwrap();
        let restored = shared.active_tab().unwrap();
        assert_eq!(restored.content, original);
        assert!(!restored.dirty);
        runtime.run_command("editor.redo", Args::new()).unwrap();
        assert_ne!(shared.active_tab().unwrap().content, original);
    }

    #[test]
    fn undo_and_rename_commands_act_on_the_open_buffer() {
        let dir = temp_dir();
        let module = EditorModule::new();
        let shared = module.state.clone();
        let mut runtime = runtime_with_module(&dir, module);
        shared.open(
            "C:\\temp\\nota.rs".to_owned(),
            "fn soma() {}\nlet total = soma();\n".to_owned(),
            model::Eol::Lf,
            model::Encoding::Utf8,
            true,
        );
        shared.record_undo("C:\\temp\\nota.rs", "fn soma() {}\n".to_owned());
        shared.set_content("C:\\temp\\nota.rs", "fn soma() {}\nlet total = soma();\n".to_owned());
        let mut args = Args::new();
        args.insert("path", json!("C:\\temp\\nota.rs"));
        runtime.run_command("editor.undo", args).unwrap();
        assert_eq!(
            shared.active_tab().unwrap().content,
            "fn soma() {}\n"
        );
        let mut rename = Args::new();
        rename.insert("word", json!("soma"));
        rename.insert("replacement", json!("somar"));
        let value = runtime.run_command("editor.rename", rename).unwrap();
        assert_eq!(value.get("renamed").and_then(Value::as_u64), Some(1));
        assert_eq!(
            shared.active_tab().unwrap().content,
            "fn somar() {}\n"
        );
    }

    #[test]
    fn definition_and_completion_commands_use_the_caret() {
        let dir = temp_dir();
        let module = EditorModule::new();
        let shared = module.state.clone();
        let mut runtime = runtime_with_module(&dir, module);
        let text = "fn soma(a: i32) -> i32 {\n  a\n}\nlet total = soma(1, 2);\n";
        shared.open(
            "C:\\temp\\lib.rs".to_owned(),
            text.to_owned(),
            model::Eol::Lf,
            model::Encoding::Utf8,
            true,
        );
        shared.set_cursor(text.find("soma").unwrap(), None);
        let value = runtime
            .run_command("editor.goto.definition", Args::new())
            .unwrap();
        assert_eq!(value.get("line").and_then(Value::as_u64), Some(1));
        assert_eq!(shared.with(|state| state.pending_line).flatten(), Some(1));

        shared.set_cursor(text.find("tot").unwrap() + 3, None);
        let value = runtime.run_command("editor.completion", Args::new()).unwrap();
        assert!(value.get("candidates").and_then(Value::as_u64).unwrap_or(0) >= 1);
        let candidates = shared
            .with(|state| state.completion.clone())
            .flatten()
            .unwrap();
        assert!(candidates.candidates.iter().any(|word| word == "total"));
    }

    #[test]
    fn cut_without_a_window_keeps_the_buffer_and_selection() {
        let dir = temp_dir();
        let module = EditorModule::new();
        let shared = module.state.clone();
        let mut runtime = runtime_with_module(&dir, module);
        shared.open(
            "C:\\temp\\nota.txt".to_owned(),
            "abc\n".to_owned(),
            model::Eol::Lf,
            model::Encoding::Utf8,
            true,
        );
        shared.set_cursor(1, Some((0, 2)));
        runtime.run_command("editor.cut", Args::new()).unwrap();
        assert_eq!(shared.active_tab().unwrap().content, "abc\n");
        assert!(shared
            .status()
            .unwrap()
            .contains("clipboard"));
    }

    #[test]
    fn markdown_preview_respects_the_catalog_toggle() {
        let dir = temp_dir();
        let module = EditorModule::new();
        let shared = module.state.clone();
        let mut runtime = runtime_with_module(&dir, module);
        shared.open(
            "C:\\temp\\doc.md".to_owned(),
            "# Título\n".to_owned(),
            model::Eol::Lf,
            model::Encoding::Utf8,
            true,
        );
        runtime
            .set_setting("editor.markdown.preview", SettingValue::Bool(false))
            .unwrap();
        runtime.run_command("editor.preview", Args::new()).unwrap();
        assert!(shared.active_tab().unwrap().preview.is_none());
        runtime
            .set_setting("editor.markdown.preview", SettingValue::Bool(true))
            .unwrap();
    }

    fn runtime_with_module(dir: &Path, module: EditorModule) -> Runtime {
        Runtime::new(
            vec![Box::new(module)],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .expect("editor registers cleanly")
    }

    #[test]
    fn save_refuses_an_open_file_outside_the_active_bench() {
        let dir = temp_dir();
        let bench = dir.join("bench");
        std::fs::create_dir_all(&bench).unwrap();
        let outside = dir.join("outside.txt");
        std::fs::write(&outside, "preserve me\n").unwrap();
        let module = EditorModule::new();
        let shared = module.state.clone();
        let mut runtime = Runtime::new(
            vec![
                Box::new(module),
                Box::new(FakeBench { path: bench.display().to_string() }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .unwrap();
        let mut open = Args::new();
        open.insert("path", json!(outside.display().to_string()));
        runtime.run_command("file.open", open).unwrap();
        wait_until(&mut runtime, || shared.active_tab().is_some());
        shared.set_content(&outside.display().to_string(), "overwrite\n".to_owned());
        let mut save = Args::new();
        save.insert("path", json!(outside.display().to_string()));
        runtime.run_command("editor.save", save).unwrap();
        wait_until(&mut runtime, || shared.status().is_some());
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "preserve me\n");
        assert!(shared.status().unwrap().contains("inside the active bench"));
    }
}

