//! Módulo `skill` da Forja: descobre, instala, copia e remove skills (pastas com `SKILL.md`) da
//! própria Forja e dos ajudantes, e mostra a página "Skills" (aba de bancada).
//!
//! Registra os comandos `skill.refresh`, `skill.install`, `skill.copy` e `skill.remove`, os
//! métodos RPC `skill.list` (read) e `skill.install` (write), e a aba `skill.page`. Contrato em
//! `design/contracts.md`, seção "Skills".

mod actions;
mod commands;
mod discovery;
mod fixtures;
mod frontmatter;
mod install;
mod model;
mod sources;
mod state;
mod view;

#[cfg(test)]
mod tests;

use crate::state::Shared;
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::ui_contract::View;
use forja_kernel::{Args, Module, ModuleManifest, Registrar, Slot};
use serde_json::json;

#[forja_kernel::module]
static SKILL: fn() -> Box<dyn Module> = || Box::new(SkillModule::new());

/// Dono do estado de skills. A aba e os comandos compartilham o `Shared`.
pub struct SkillModule {
    shared: Shared,
}

impl SkillModule {
    pub fn new() -> Self {
        Self {
            shared: Shared::new(),
        }
    }
}

impl Default for SkillModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for SkillModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("skill", "skill.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        for key in actions::USED_KEYS.iter().chain(view::USED_KEYS).copied() {
            r.uses_i18n(key);
        }

        let shared = self.shared.clone();
        r.command(
            "skill.refresh",
            "skill.command.refresh",
            move |cx, _args| commands::refresh(cx, &shared),
        );

        let shared = self.shared.clone();
        r.command("skill.install", "skill.command.install", move |cx, args| {
            commands::install(cx, &shared, &args)
        });

        let shared = self.shared.clone();
        r.command("skill.copy", "skill.command.copy", move |cx, args| {
            commands::copy(cx, &shared, &args)
        });

        let shared = self.shared.clone();
        r.command("skill.remove", "skill.command.remove", move |cx, args| {
            commands::remove(cx, &shared, &args)
        });

        let shared = self.shared.clone();
        r.rpc(
            Method::new("skill.list", "skill.method.list.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("skill.method.list.result").schema(json!({
                    "type": "object",
                    "properties": {
                        "roots": { "type": "array" },
                        "skills": { "type": "array" },
                        "project": { "type": ["string", "null"] },
                        "scanned": { "type": "boolean" },
                        "scanning": { "type": "boolean" }
                    },
                    "required": ["roots", "skills"]
                }))),
            move |cx, _params| {
                // Lista pronta agora; se ainda não foi lida, começa a leitura em segundo plano.
                actions::refresh(cx, &shared);
                Ok(commands::snapshot(&shared))
            },
        );

        let shared = self.shared.clone();
        r.rpc(
            Method::new("skill.install", "skill.method.install.description")
                .effect(MethodEffect::Write)
                .param(
                    ParamSchema::required("source", ParamKind::String)
                        .describe("Pasta com SKILL.md, ou arquivo .zip."),
                )
                .param(
                    ParamSchema::optional("kind", ParamKind::String)
                        .describe("folder (padrão) ou zip."),
                )
                .param(
                    ParamSchema::required("owner", ParamKind::String)
                        .describe("Id do dono: forja ou o id de um ajudante."),
                )
                .param(
                    ParamSchema::required("scope", ParamKind::String).describe("user ou project."),
                )
                .param(
                    ParamSchema::optional("overwrite", ParamKind::Boolean).describe(
                        "Substitui a skill de mesmo nome; a versão antiga vai para a lixeira.",
                    ),
                )
                .param(
                    ParamSchema::optional("consent", ParamKind::Boolean)
                        .describe("Obrigatório para escrever fora da Forja."),
                )
                .result(ResultSchema::new("skill.method.install.result")),
            move |cx, params| {
                let args = Args::from_map(params);
                commands::install(cx, &shared, &args)
                    .map_err(|error| RpcError::app(error.to_string()))
            },
        );

        let shared = self.shared.clone();
        r.view(Slot::Rail, "skill.page", "skill.tab.title", 60, move || {
            Box::new(view::SkillView::new(shared.clone())) as Box<dyn View>
        });
        r.view_glyph("skill.page", forja_ui_kit::SKILLS.id());
    }
}
