//! Módulo `viewers`: visualização somente leitura por extensão.
//!
//! O editor chama `viewers.preview` para obter o modelo do documento e desenha
//! a prévia na própria aba. Este módulo também mantém uma aba própria e escuta
//! `file.open_requested` para prévias abertas por links, busca ou diferenças.

mod model;
mod state;
mod view;
mod view_error;

use forja_contracts::names;
use forja_contracts::FileOpenRequested;
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::{Module, ModuleManifest, Registrar, Slot};
use serde_json::{json, Value};
use state::Shared;

#[forja_kernel::module]
static VIEWERS: fn() -> Box<dyn Module> =
    || Box::new(ViewersModule { state: Shared::new() });

pub struct ViewersModule {
    state: Shared,
}

impl Module for ViewersModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("viewers", "viewers.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(include_str!("../i18n/en.ftl"), include_str!("../i18n/pt-BR.ftl"));
        r.keymap(include_str!("../keymap.toml"));

        r.contract_event::<FileOpenRequested>(names::FILE_OPEN_REQUESTED);
        let event_state = self.state.clone();
        r.subscribe(move |_cx, event: &FileOpenRequested| {
            if model::viewer_for(&event.path).is_some() {
                event_state.set_request(event.path.clone());
            }
        });

        let list_state = self.state.clone();
        r.command("viewers.list", "viewers.command.list", move |_cx, _| {
            list_state.set_notice(None);
            Ok(json!({ "viewers": model::descriptors() }))
        });

        let preview_state = self.state.clone();
        r.command("viewers.preview", "viewers.command.preview", move |_cx, args| {
            let path = args.required_str("path")?;
            let document = model::preview(path);
            preview_state.set_notice(None);
            Ok(serde_json::to_value(document)?)
        });

        let open_state = self.state.clone();
        r.command("viewers.open", "viewers.command.open", move |_cx, args| {
            let path = args.required_str("path")?;
            let viewer = model::viewer_for(path);
            if viewer.is_some() {
                open_state.set_request(path.to_owned());
            }
            Ok(json!({ "opened": viewer.is_some(), "viewer": viewer }))
        });

        let external_state = self.state.clone();
        r.command(
            "viewers.open_external",
            "viewers.command.open_external",
            move |cx, args| {
                let path = args.required_str("path")?.to_owned();
                external_state.set_notice(None);
                let state = external_state.clone();
                cx.spawn(
                    move || open_external(&path),
                    move |cx, result| {
                        if let Err(error) = result {
                            state.set_notice(Some(cx.t_args(
                                "viewers.notice.open-external",
                                &[("message", &error)],
                            )));
                        }
                    },
                );
                Ok(json!({ "pending": true }))
            },
        );

        let close_state = self.state.clone();
        r.command("viewers.close", "viewers.command.close", move |_cx, _| {
            close_state.clear();
            Ok(json!({ "closed": true }))
        });

        let rpc_state = self.state.clone();
        r.rpc(
            Method::new("viewers.list", "viewers.method.list.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("viewers.method.list.result")),
            move |_cx, _| {
                rpc_state.set_notice(None);
                Ok(json!({ "viewers": model::descriptors() }))
            },
        );

        let rpc_preview = self.state.clone();
        r.rpc(
            Method::new("viewers.preview", "viewers.method.preview.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .param(ParamSchema::required("path", ParamKind::String).alias("caminho"))
                .result(ResultSchema::new("viewers.method.preview.result")),
            move |_cx, params| {
                let path = params
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("missing `path`"))?;
                let document = model::preview(path);
                rpc_preview.set_notice(None);
                Ok(serde_json::to_value(document).unwrap_or(Value::Null))
            },
        );

        for key in used_keys() {
            r.uses_i18n(key);
        }

        let view_state = self.state.clone();
        r.view(
            Slot::BenchTab,
            "viewers.tab",
            "viewers.tab.title",
            bench_tab_order(34),
            move || Box::new(view::ViewerView::new(view_state.clone())),
        );
    }
}

fn bench_tab_order(default: i32) -> i32 {
    match std::env::var("FORJA_BENCH_TAB") {
        Ok(module) if module == "viewers" => 1,
        _ => default,
    }
}

fn open_external(path: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        let mut command = forja_kernel::process::command("cmd");
        command.args(["/C", "start", "", path]);
        command
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    #[cfg(not(windows))]
    {
        forja_kernel::process::command("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

fn used_keys() -> Vec<&'static str> {
    vec![
        "viewers.name",
        "viewers.tab.title",
        "viewers.command.list",
        "viewers.command.preview",
        "viewers.command.open",
        "viewers.command.open_external",
        "viewers.command.close",
        "viewers.viewer.empty",
        "viewers.action.open-external",
        "viewers.descriptor.markdown",
        "viewers.descriptor.table",
        "viewers.descriptor.notebook",
        "viewers.descriptor.image",
        "viewers.descriptor.pdf",
        "viewers.markdown.toc",
        "viewers.markdown.mermaid",
        "viewers.markdown.mermaid-hint",
        "viewers.callout.note",
        "viewers.callout.tip",
        "viewers.callout.important",
        "viewers.callout.warning",
        "viewers.callout.caution",
        "viewers.table.rows",
        "viewers.table.truncated",
        "viewers.notebook.cells",
        "viewers.notebook.kind.markdown",
        "viewers.notebook.kind.code",
        "viewers.notebook.kind.raw",
        "viewers.image.frames",
        "viewers.image.loading",
        "viewers.pdf.pages",
        "viewers.pdf.hint",
        "viewers.notice.open-external",
        "viewers.notice.preview",
        "viewers.method.list.description",
        "viewers.method.list.result",
        "viewers.method.preview.description",
        "viewers.method.preview.result",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use forja_kernel::Args;
    use forja_contracts::ViewerDocument;
    use serde_json::{json, Map};

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-viewers-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn runtime(dir: &std::path::Path) -> Runtime {
        Runtime::new(
            vec![Box::new(ViewersModule {
                state: Shared::new(),
            })],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("viewers registers cleanly")
    }

    #[test]
    fn contract_and_preview_acceptance() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let path = dir.join("nota.md");
        std::fs::write(&path, "# Título\n\n> [!NOTE]\n> Olá\n").unwrap();
        let value = runtime
            .call_method(
                "viewers.preview",
                Map::from_iter([("path".to_owned(), json!(path))]),
            )
            .unwrap();
        let document: ViewerDocument = serde_json::from_value(value).unwrap();
        assert_eq!(document.kind, "markdown");
        assert!(document.markdown.unwrap().toc.iter().any(|entry| entry.text == "Título"));
        let list = runtime.call_method("viewers.list", Map::new()).unwrap();
        assert!(list["viewers"].as_array().unwrap().len() >= 4);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn descriptors_cover_the_expected_extensions() {
        let list = model::descriptors();
        assert!(list.iter().any(|viewer| viewer.extensions.contains(&"ipynb".to_owned())));
        assert_eq!(model::viewer_for("a.tsv"), Some("table"));
    }

    #[test]
    fn open_command_marks_the_preview_request() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let path = dir.join("a.csv");
        std::fs::write(&path, "a,b\n1,2\n").unwrap();
        let value = runtime
            .run_command("viewers.open", Args::from_map(Map::from_iter([(
                "path".to_owned(),
                json!(path),
            )])))
            .unwrap();
        assert_eq!(value["opened"], true);
        let _ = std::fs::remove_dir_all(dir);
    }
}
