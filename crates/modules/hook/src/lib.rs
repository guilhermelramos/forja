//! Módulo `hook`: ponte do binário `forja-hook` para o módulo helper.
//!
//! O binário chama `hook.event` com o payload lido do stdin. Este módulo só
//! repassa o payload para `helper.hook`; assim o nome do método pertence ao
//! módulo que o registra e o helper continua dono do status.

use forja_kernel::forja_protocol::{Method, MethodEffect, ParamKind, ParamSchema, ResultSchema};
use forja_kernel::{Args, Module, ModuleManifest, Registrar};
use serde_json::json;

#[forja_kernel::module]
static HOOK: fn() -> Box<dyn Module> = || Box::new(HookModule);

/// Módulo ponte.
pub struct HookModule;

impl Module for HookModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("hook", "hook.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(include_str!("../i18n/en.ftl"), include_str!("../i18n/pt-BR.ftl"));
        r.keymap(include_str!("../keymap.toml"));
        r.rpc(
            Method::new("hook.event", "hook.method.event.description")
                .effect(MethodEffect::Write)
                .param(ParamSchema::required("payload", ParamKind::Json))
                .result(ResultSchema::new("hook.method.event.result").schema(json!({
                    "type": "object"
                }))),
            |cx, params| {
                // Sem o helper, o hook é um no-op silencioso.
                let _ = cx.run_command("helper.hook", Args::from_map(params));
                Ok(json!({}))
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use serde_json::Map;

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-hook-module-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn contract_and_rpc_degrade_without_helper() {
        let dir = temp_dir("rpc");
        let mut runtime = Runtime::new(
            vec![Box::new(HookModule)],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("hook module registers");
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let value = runtime
            .call_method(
                "hook.event",
                Map::from_iter([("payload".to_owned(), json!({"hook_event_name": "Stop"}))]),
            )
            .expect("hook event degrades");
        assert_eq!(value, json!({}));
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }
}
