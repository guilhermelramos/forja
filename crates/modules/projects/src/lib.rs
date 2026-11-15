//! Módulo `projects` da Forja.
//!
//! Registra as pastas que a Forja conhece (git ou não), a lateral de projetos e
//! os comandos/RPC `project.*`. As bancadas de cada projeto vêm do comando
//! `bench.list`; este módulo não conhece o crate `benches`.

mod model;
mod state;
mod view;
mod view_dialogs;

use forja_contracts::names;
use forja_contracts::{BenchActivated, ProjectActivated, ProjectAdded, ProjectRef, ProjectRemoved};
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::{
    Args, CommandField, CommandFieldKind, Module, ModuleManifest, Registrar, Slot,
};
use serde_json::{json, Map, Value};
use state::{CloneForm, Dialog, ProjectForm, Shared};

#[forja_kernel::module]
static PROJECTS: fn() -> Box<dyn Module> = || {
    Box::new(ProjectsModule {
        state: Shared::new(),
    })
};

pub struct ProjectsModule {
    state: Shared,
}

impl Module for ProjectsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("project", "project.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));

        r.contract_event::<ProjectAdded>(names::PROJECT_ADDED);
        r.contract_event::<ProjectActivated>(names::PROJECT_ACTIVATED);
        r.contract_event::<ProjectRemoved>(names::PROJECT_REMOVED);
        r.contract_event::<BenchActivated>(names::BENCH_ACTIVATED);

        let bench_sync_state = self.state.clone();
        r.subscribe::<BenchActivated, _>(move |cx, event| {
            bench_sync_state.ensure_loaded(cx);
            let _ = bench_sync_state.activate(cx, &event.0.project_id);
        });

        let current_state = self.state.clone();
        r.command("project.current", "project.command.current", move |cx, _| {
            current_state.ensure_loaded(cx);
            Ok(serde_json::to_value(current_state.active())?)
        });

        let switch_state = self.state.clone();
        r.command_asking(
            "project.switch",
            "project.command.switch",
            vec![CommandField {
                arg: "project_id",
                label_key: "project.ask.project",
                kind: CommandFieldKind::Choice {
                    command: "project.list",
                    list: "projects",
                    label: "name",
                    value: "id",
                    detail: Some("path"),
                },
            }],
            move |cx, args| {
                switch_state.ensure_loaded(cx);
                let id = args.required_str("project_id")?;
                let active = switch_state.activate(cx, id).map_err(kerr)?;
                Ok(serde_json::to_value(active)?)
            },
        );

        let list_state = self.state.clone();
        r.command("project.list", "project.command.list", move |cx, _| {
            list_state.ensure_loaded(cx);
            Ok(json!({ "projects": refs(&list_state) }))
        });

        let add_state = self.state.clone();
        r.command("project.add", "project.command.add", move |cx, args| {
            add_state.ensure_loaded(cx);
            let form = form_from_args(&args);
            let project = add_state.add(cx, &form).map_err(kerr)?;
            Ok(serde_json::to_value(state::reference(&project))?)
        });

        let remove_state = self.state.clone();
        r.command(
            "project.remove",
            "project.command.remove",
            move |cx, args| {
                remove_state.ensure_loaded(cx);
                let id = args.required_str("project_id")?;
                remove_state.remove(cx, id).map_err(kerr)?;
                Ok(json!({ "removed": true }))
            },
        );

        let add_dialog_state = self.state.clone();
        r.command(
            "project.add_dialog",
            "project.command.add_dialog",
            move |cx, _args| {
                add_dialog_state.ensure_loaded(cx);
                add_dialog_state.set_dialog(Dialog::Add(ProjectForm::default()));
                Ok(json!({ "pending": true }))
            },
        );

        let create_state = self.state.clone();
        r.command(
            "project.create",
            "project.command.create",
            move |cx, args| {
                spawn_create(cx, &create_state, form_from_args(&args)).map_err(kerr)?;
                Ok(json!({ "pending": true }))
            },
        );

        let clone_state = self.state.clone();
        r.command("project.clone", "project.command.clone", move |cx, args| {
            let url = args.required_str("url")?.to_owned();
            let destination = args.required_str("path")?.to_owned();
            let form = CloneForm {
                url,
                form: clone_form_from_args(&args, &destination),
            };
            spawn_clone(cx, &clone_state, form).map_err(kerr)?;
            Ok(json!({ "pending": true }))
        });

        let update_state = self.state.clone();
        r.command(
            "project.update",
            "project.command.update",
            move |cx, args| {
                update_state.ensure_loaded(cx);
                let id = args.required_str("project_id")?;
                let project = update_state
                    .update(
                        cx,
                        id,
                        args.str("name"),
                        args.str("icon"),
                        args.str("color"),
                    )
                    .map_err(kerr)?;
                Ok(serde_json::to_value(state::reference(&project))?)
            },
        );

        let rpc_list = self.state.clone();
        r.rpc(
            Method::new("project.list", "project.method.list.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(
                    ResultSchema::new("project.method.list.result").schema(json!({
                        "type": "object",
                        "properties": { "projects": { "type": "array" } },
                        "required": ["projects"]
                    })),
                ),
            move |cx, _| {
                rpc_list.ensure_loaded(cx);
                Ok(json!({ "projects": refs(&rpc_list) }))
            },
        );

        let rpc_add = self.state.clone();
        r.rpc(
            Method::new("project.add", "project.method.add.description")
                .effect(MethodEffect::Write)
                .param(ParamSchema::required("path", ParamKind::String).alias("caminho"))
                .param(ParamSchema::optional("name", ParamKind::String).alias("nome"))
                .param(ParamSchema::optional("icon", ParamKind::String).alias("icone"))
                .param(ParamSchema::optional("color", ParamKind::String).alias("cor"))
                .result(ResultSchema::new("project.method.add.result")),
            move |cx, params| {
                rpc_add.ensure_loaded(cx);
                let form = form_from_params(&params);
                let project = rpc_add.add(cx, &form).map_err(RpcError::app)?;
                Ok(serde_json::to_value(state::reference(&project)).unwrap_or(Value::Null))
            },
        );

        let rpc_remove = self.state.clone();
        r.rpc(
            Method::new("project.remove", "project.method.remove.description")
                .effect(MethodEffect::Write)
                .param(ParamSchema::required("project_id", ParamKind::String).alias("id"))
                .result(ResultSchema::new("project.method.remove.result")),
            move |cx, params| {
                rpc_remove.ensure_loaded(cx);
                let id = params
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("missing `project_id`"))?;
                rpc_remove.remove(cx, id).map_err(RpcError::app)?;
                Ok(json!({ "removed": true }))
            },
        );

        let rpc_create = self.state.clone();
        r.rpc(
            Method::new("project.create", "project.method.create.description")
                .effect(MethodEffect::Write)
                .param(ParamSchema::required("path", ParamKind::String).alias("caminho"))
                .param(ParamSchema::optional("name", ParamKind::String).alias("nome"))
                .param(ParamSchema::optional("icon", ParamKind::String))
                .param(ParamSchema::optional("color", ParamKind::String))
                .result(ResultSchema::new("project.method.create.result")),
            move |cx, params| {
                spawn_create(cx, &rpc_create, form_from_params(&params)).map_err(RpcError::app)?;
                Ok(json!({ "pending": true }))
            },
        );

        let rpc_clone = self.state.clone();
        r.rpc(
            Method::new("project.clone", "project.method.clone.description")
                .effect(MethodEffect::Write)
                .network(true)
                .param(ParamSchema::required("url", ParamKind::String).alias("repositorio"))
                .param(ParamSchema::required("path", ParamKind::String).alias("caminho"))
                .param(ParamSchema::optional("name", ParamKind::String))
                .param(ParamSchema::optional("icon", ParamKind::String))
                .param(ParamSchema::optional("color", ParamKind::String))
                .result(ResultSchema::new("project.method.clone.result")),
            move |cx, params| {
                let url = params
                    .get("url")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("missing `url`"))?
                    .to_owned();
                let form = CloneForm {
                    url,
                    form: clone_form_from_params(&params),
                };
                spawn_clone(cx, &rpc_clone, form).map_err(RpcError::app)?;
                Ok(json!({ "pending": true }))
            },
        );

        let rpc_update = self.state.clone();
        r.rpc(
            Method::new("project.update", "project.method.update.description")
                .effect(MethodEffect::Write)
                .param(ParamSchema::required("project_id", ParamKind::String))
                .param(ParamSchema::optional("name", ParamKind::String))
                .param(ParamSchema::optional("icon", ParamKind::String))
                .param(ParamSchema::optional("color", ParamKind::String))
                .result(ResultSchema::new("project.method.update.result")),
            move |cx, params| {
                rpc_update.ensure_loaded(cx);
                let id = params
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("missing `project_id`"))?;
                let project = rpc_update
                    .update(
                        cx,
                        id,
                        params.get("name").and_then(Value::as_str),
                        params.get("icon").and_then(Value::as_str),
                        params.get("color").and_then(Value::as_str),
                    )
                    .map_err(RpcError::app)?;
                Ok(serde_json::to_value(state::reference(&project)).unwrap_or(Value::Null))
            },
        );

        let rpc_current = self.state.clone();
        r.rpc(
            Method::new("project.current", "project.method.current.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("project.method.current.result")),
            move |cx, _| {
                rpc_current.ensure_loaded(cx);
                Ok(serde_json::to_value(rpc_current.active()).unwrap_or(Value::Null))
            },
        );

        let rpc_switch = self.state.clone();
        r.rpc(
            Method::new("project.switch", "project.method.switch.description")
                .effect(MethodEffect::Write)
                .param(
                    ParamSchema::required("project_id", ParamKind::String)
                        .alias("projeto")
                        .alias("project-id"),
                )
                .result(ResultSchema::new("project.method.switch.result")),
            move |cx, params| {
                rpc_switch.ensure_loaded(cx);
                let id = params
                    .get("project_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("missing `project_id`"))?;
                let active = rpc_switch.activate(cx, id).map_err(RpcError::app)?;
                Ok(serde_json::to_value(active).unwrap_or(Value::Null))
            },
        );

        for key in [
            "project.name",
            "project.command.add_dialog",
            "project.ask.project",
            "project.dialogs.title",
            "project.sidebar.title",
            "project.sidebar.empty",
            "project.sidebar.add",
            "project.sidebar.create",
            "project.sidebar.create-hint",
            "project.sidebar.clone",
            "project.sidebar.remove",
            "project.sidebar.edit",
            "project.sidebar.no-benches",
            "project.sidebar.benches-missing",
            "project.dialog.add.title",
            "project.dialog.create.title",
            "project.dialog.clone.title",
            "project.dialog.edit.title",
            "project.dialog.remove.title",
            "project.dialog.remove.body",
            "project.field.path",
            "project.field.new-path",
            "project.field.destination",
            "project.field.url",
            "project.field.name",
            "project.field.name-hint",
            "project.field.icon",
            "project.field.color",
            "project.action.choose",
            "project.action.add",
            "project.action.create",
            "project.action.clone",
            "project.action.save",
            "project.action.cancel",
            "project.action.remove",
            "project.status.creating",
            "project.status.cloning",
            "project.error.prefix",
        ] {
            r.uses_i18n(key);
        }

        let view_state = self.state.clone();
        r.view(
            Slot::Sidebar,
            "project.sidebar",
            "project.sidebar.title",
            10,
            move || Box::new(view::ProjectsView::new(view_state.clone())),
        );
        let dialog_state = self.state.clone();
        r.view(
            Slot::Modal,
            "project.dialogs",
            "project.dialogs.title",
            20,
            move || Box::new(view::ProjectsDialogs::new(dialog_state.clone())),
        );
    }
}

fn refs(state: &Shared) -> Vec<ProjectRef> {
    state.projects().iter().map(state::reference).collect()
}

fn form_from_args(args: &Args) -> model::NewProject {
    model::NewProject {
        path: args.str("path").unwrap_or_default().to_owned(),
        name: args.str("name").unwrap_or_default().to_owned(),
        icon: args.str("icon").unwrap_or(model::DEFAULT_ICON).to_owned(),
        color: args.str("color").unwrap_or(model::DEFAULT_COLOR).to_owned(),
    }
}

fn clone_form_from_args(args: &Args, destination: &str) -> state::ProjectForm {
    state::ProjectForm {
        path: destination.to_owned(),
        name: args.str("name").unwrap_or_default().to_owned(),
        icon: args.str("icon").unwrap_or(model::DEFAULT_ICON).to_owned(),
        color: args.str("color").unwrap_or(model::DEFAULT_COLOR).to_owned(),
        error: None,
    }
}

fn clone_form_from_params(params: &Map<String, Value>) -> state::ProjectForm {
    state::ProjectForm {
        path: params
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        name: params
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        icon: params
            .get("icon")
            .and_then(Value::as_str)
            .unwrap_or(model::DEFAULT_ICON)
            .to_owned(),
        color: params
            .get("color")
            .and_then(Value::as_str)
            .unwrap_or(model::DEFAULT_COLOR)
            .to_owned(),
        error: None,
    }
}

fn kerr(message: String) -> forja_kernel::KernelError {
    forja_kernel::KernelError::Message(message)
}

fn form_from_params(params: &Map<String, Value>) -> model::NewProject {
    model::NewProject {
        path: params
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        name: params
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        icon: params
            .get("icon")
            .and_then(Value::as_str)
            .unwrap_or(model::DEFAULT_ICON)
            .to_owned(),
        color: params
            .get("color")
            .and_then(Value::as_str)
            .unwrap_or(model::DEFAULT_COLOR)
            .to_owned(),
    }
}

fn spawn_create(
    cx: &mut forja_kernel::Ctx<'_>,
    shared: &Shared,
    form: model::NewProject,
) -> Result<(), String> {
    if form.path.trim().is_empty() {
        return Err("path is empty".to_owned());
    }
    let path = form.path.clone();
    let state = shared.clone();
    state.set_status(Some(cx.t("project.status.creating")));
    cx.spawn(
        move || state::create_folder(&path),
        move |cx, result| {
            state.set_status(None);
            match result {
                Ok(canonical) => {
                    let new = model::NewProject {
                        path: canonical.to_string_lossy().into_owned(),
                        name: form.name.clone(),
                        icon: form.icon.clone(),
                        color: form.color.clone(),
                    };
                    if let Err(error) = state.add(cx, &new) {
                        state.set_error(Some(error));
                    }
                }
                Err(error) => state.set_error(Some(error)),
            }
        },
    );
    Ok(())
}

fn spawn_clone(
    cx: &mut forja_kernel::Ctx<'_>,
    shared: &Shared,
    clone: CloneForm,
) -> Result<(), String> {
    if clone.url.trim().is_empty() {
        return Err("repository URL is required".to_owned());
    }
    if clone.form.path.trim().is_empty() {
        return Err("destination is required".to_owned());
    }
    let state = shared.clone();
    let CloneForm { url, form } = clone;
    state.set_status(Some(cx.t("project.status.cloning")));
    cx.spawn(
        move || state::clone_repository(&url, &form.path),
        move |cx, result| {
            state.set_status(None);
            match result {
                Ok(canonical) => {
                    let new = model::NewProject {
                        path: canonical.to_string_lossy().into_owned(),
                        name: form.name.clone(),
                        icon: form.icon.clone(),
                        color: form.color.clone(),
                    };
                    if let Err(error) = state.add(cx, &new) {
                        state.set_error(Some(error));
                    }
                }
                Err(error) => state.set_error(Some(error)),
            }
        },
    );
    Ok(())
}

#[cfg(test)]
mod tests;
