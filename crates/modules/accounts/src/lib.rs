//! Módulo `accounts` da Forja: várias contas por ajudante com home de
//! configuração separada, conta ativa e env injetado no `helper.launch`.

mod actions;
mod model;
mod profiles;
mod state;
mod view;

use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::{
    Args, Ctx, KernelError, Module, ModuleManifest, Registrar, SettingKind, SettingSpec,
    SettingValue, Slot,
};
use model::ProfileMeta;
use serde_json::{json, Map, Value};
use state::Shared;

#[forja_kernel::module]
static ACCOUNTS: fn() -> Box<dyn Module> = || Box::new(AccountsModule::new());

pub struct AccountsModule {
    state: Shared,
}

impl AccountsModule {
    pub fn new() -> Self {
        Self {
            state: Shared::new(),
        }
    }
}

impl Default for AccountsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for AccountsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("accounts", "accounts.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        r.setting(SettingSpec::new(
            "accounts.confirm.remove",
            SettingKind::Bool,
            SettingValue::Bool(true),
            "setting-accounts-confirm-remove-label",
            "setting-accounts-confirm-remove-help",
        ));

        let list = self.state.clone();
        r.command(
            "accounts.list",
            "accounts.command.list",
            move |cx, _args| Ok(list_json(cx, &list)),
        );

        let add = self.state.clone();
        r.command("accounts.add", "accounts.command.add", move |cx, args| {
            add_account(cx, &add, &args)
        });

        let select = self.state.clone();
        r.command(
            "accounts.select",
            "accounts.command.select",
            move |cx, args| {
                let profile_id = args.required_str("profile_id")?.to_owned();
                let id = args.required_str("id")?.to_owned();
                state::ensure_loaded(cx, &select);
                select
                    .with(|state| actions::select(&mut state.data, &profile_id, &id))
                    .unwrap_or_else(|| Err("accounts are locked".to_owned()))
                    .map_err(KernelError::Message)?;
                state::persist(cx, &select);
                Ok(json!({ "selected": id }))
            },
        );

        let remove = self.state.clone();
        r.command(
            "accounts.remove",
            "accounts.command.remove",
            move |cx, args| remove_account(cx, &remove, &args),
        );

        let env = self.state.clone();
        r.command("accounts.env", "accounts.command.env", move |cx, args| {
            let profile_id = args.required_str("profile_id")?.to_owned();
            state::ensure_loaded(cx, &env);
            let profiles = profiles::list();
            let value = env
                .with(|state| actions::env_for(&state.data, &profiles, &profile_id))
                .unwrap_or_default();
            Ok(json!({ "env": value }))
        });

        let roots = self.state.clone();
        r.command(
            "accounts.roots",
            "accounts.command.roots",
            move |cx, _args| {
                state::ensure_loaded(cx, &roots);
                let accounts = roots
                    .with(|state| actions::roots(&state.data))
                    .unwrap_or_default();
                Ok(json!({ "accounts": accounts }))
            },
        );

        register_rpc(r, &self.state);

        for key in used_keys() {
            r.uses_i18n(key);
        }

        let section = self.state.clone();
        r.view(
            Slot::SettingsSection,
            "accounts.settings",
            "accounts.settings.title",
            27,
            move || Box::new(view::AccountsSettings::new(section.clone())),
        );
    }
}

fn list_json(cx: &mut Ctx<'_>, shared: &Shared) -> Value {
    state::ensure_loaded(cx, shared);
    let mut installed = Map::new();
    if let Ok(value) = cx.run_command("helper.list", Args::new()) {
        if let Some(items) = value.get("profiles").and_then(Value::as_array) {
            for item in items {
                if let (Some(id), Some(executable)) = (
                    item.get("id").and_then(Value::as_str),
                    item.get("executable").and_then(Value::as_str),
                ) {
                    installed.insert(id.to_owned(), json!(executable));
                }
            }
        }
    }
    let data = shared.with(|state| state.data.clone()).unwrap_or_default();
    let profiles: Vec<Value> = profiles::list()
        .into_iter()
        .map(|profile| {
            json!({
                "profile_id": profile.id,
                "name": profile.name,
                "env": profile.env,
                "login": profile.login,
                "installed": installed.get(&profile.id).is_some(),
                "executable": installed.get(&profile.id).cloned(),
                "accounts": data.view(&profile.id),
            })
        })
        .collect();
    json!({ "profiles": profiles })
}

fn add_account(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> forja_kernel::Result<Value> {
    let profile_id = args.required_str("profile_id")?.to_owned();
    let label = args.str("label").unwrap_or("").to_owned();
    state::ensure_loaded(cx, shared);
    let profiles = profiles::list();
    let Some(meta) = profiles.iter().find(|meta| meta.id == profile_id).cloned() else {
        return Err(KernelError::Message(format!(
            "profile `{profile_id}` has no account home"
        )));
    };
    let account = shared
        .with(|state| actions::create_at(&mut state.data, &meta, &label, &actions::root_from(cx)))
        .unwrap_or_else(|| Err("accounts are locked".to_owned()))
        .map_err(KernelError::Message)?;
    state::persist(cx, shared);
    let terminal_id = open_login(cx, &meta, &account);
    Ok(json!({ "account": account, "terminal_id": terminal_id }))
}

fn remove_account(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> forja_kernel::Result<Value> {
    let profile_id = args.required_str("profile_id")?.to_owned();
    let id = args.required_str("id")?.to_owned();
    let ask = cx
        .setting("accounts.confirm.remove")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true);
    if ask && !args.bool("confirm").unwrap_or(false) {
        return Err(KernelError::Message(
            "confirmation is required to remove the account".to_owned(),
        ));
    }
    state::ensure_loaded(cx, shared);
    shared
        .with(|state| {
            actions::remove_at(
                &mut state.data,
                &profile_id,
                &id,
                true,
                &actions::root_from(cx),
            )
        })
        .unwrap_or_else(|| Err("accounts are locked".to_owned()))
        .map_err(KernelError::Message)?;
    state::persist(cx, shared);
    Ok(json!({ "removed": id }))
}

fn open_login(cx: &mut Ctx<'_>, meta: &ProfileMeta, account: &model::Account) -> Option<String> {
    let helper = cx.run_command("helper.list", Args::new()).ok()?;
    let executable = helper
        .get("profiles")
        .and_then(Value::as_array)?
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(meta.id.as_str()))?
        .get("executable")
        .and_then(Value::as_str)?
        .to_owned();
    let mut args = Args::new();
    args.insert(
        "cwd",
        json!(forja_kernel::paths::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .display()
            .to_string()),
    );
    args.insert("command", json!(executable));
    args.insert("args", json!(meta.login.join(" ")));
    args.insert("env", json!(actions::account_env(meta, account)));
    args.insert("title", json!(format!("{} · {}", meta.name, account.label)));
    let opened = cx.run_command("terminal.open", args).ok()?;
    opened
        .get("terminal_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn register_rpc(r: &mut Registrar<'_>, shared: &Shared) {
    let list = shared.clone();
    r.rpc(
        Method::new("accounts.list", "accounts.method.list.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .result(ResultSchema::new("accounts.method.list.result")),
        move |cx, _params| Ok(list_json(cx, &list)),
    );

    let add = shared.clone();
    r.rpc(
        Method::new("accounts.add", "accounts.method.add.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("profile_id", ParamKind::String))
            .param(ParamSchema::optional("label", ParamKind::String))
            .result(ResultSchema::new("accounts.method.add.result")),
        move |cx, params| {
            let args = Args::from_map(params.clone());
            add_account(cx, &add, &args).map_err(rpc_error)
        },
    );

    let select = shared.clone();
    r.rpc(
        Method::new("accounts.select", "accounts.method.select.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("profile_id", ParamKind::String))
            .param(ParamSchema::required("id", ParamKind::String))
            .result(ResultSchema::new("accounts.method.select.result")),
        move |cx, params| {
            let profile_id = params
                .get("profile_id")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_params("missing `profile_id`"))?
                .to_owned();
            let id = params
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_params("missing `id`"))?
                .to_owned();
            state::ensure_loaded(cx, &select);
            select
                .with(|state| actions::select(&mut state.data, &profile_id, &id))
                .unwrap_or_else(|| Err("accounts are locked".to_owned()))
                .map_err(RpcError::app)?;
            state::persist(cx, &select);
            Ok(json!({ "selected": id }))
        },
    );

    let remove = shared.clone();
    r.rpc(
        Method::new("accounts.remove", "accounts.method.remove.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("profile_id", ParamKind::String))
            .param(ParamSchema::required("id", ParamKind::String))
            .param(ParamSchema::optional("confirm", ParamKind::Boolean))
            .result(ResultSchema::new("accounts.method.remove.result")),
        move |cx, params| {
            let args = Args::from_map(params.clone());
            remove_account(cx, &remove, &args).map_err(rpc_error)
        },
    );
}

fn rpc_error(error: KernelError) -> RpcError {
    RpcError::app(error.to_string())
}

fn used_keys() -> Vec<&'static str> {
    vec![
        "accounts.name",
        "accounts.settings.title",
        "accounts.settings.subtitle",
        "accounts.settings.empty",
        "accounts.add",
        "accounts.add-hint",
        "accounts.active",
        "accounts.remove",
        "accounts.confirm",
        "accounts.cancel",
        "accounts.not-installed",
        "accounts.notice-added",
        "accounts.notice-added-login",
        "accounts.notice-selected",
        "accounts.notice-removed",
        "accounts.command.list",
        "accounts.command.add",
        "accounts.command.select",
        "accounts.command.remove",
        "accounts.command.env",
        "accounts.command.roots",
        "accounts.method.list.description",
        "accounts.method.list.result",
        "accounts.method.add.description",
        "accounts.method.add.result",
        "accounts.method.select.description",
        "accounts.method.select.result",
        "accounts.method.remove.description",
        "accounts.method.remove.result",
        "setting-accounts-confirm-remove-label",
        "setting-accounts-confirm-remove-help",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use serde_json::Map;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-accounts-{}-{}",
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
            vec![Box::new(AccountsModule::new())],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("accounts registers cleanly")
    }

    #[test]
    fn contract_holds() {
        let dir = temp_dir();
        let runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn add_select_env_and_remove_through_commands() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let mut args = Args::new();
        args.insert("profile_id", json!("claude-code"));
        args.insert("label", json!("Ana"));
        let added = runtime.run_command("accounts.add", args).unwrap();
        let id = added["account"]["id"].as_str().unwrap().to_owned();
        assert!(value_has_account(
            &runtime.run_command("accounts.list", Args::new()).unwrap(),
            &id
        ));

        let mut args = Args::new();
        args.insert("profile_id", json!("claude-code"));
        args.insert("id", json!(id));
        let env = runtime.run_command("accounts.env", args).unwrap();
        assert!(env["env"]["CLAUDE_CONFIG_DIR"].as_str().is_some());

        let mut args = Args::new();
        args.insert("profile_id", json!("claude-code"));
        args.insert("id", json!(id));
        args.insert("confirm", json!(true));
        runtime.run_command("accounts.remove", args).unwrap();
        assert!(!value_has_account(
            &runtime.run_command("accounts.list", Args::new()).unwrap(),
            &id
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn removal_needs_confirmation() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let mut args = Args::new();
        args.insert("profile_id", json!("claude-code"));
        let added = runtime.run_command("accounts.add", args).unwrap();
        let id = added["account"]["id"].as_str().unwrap().to_owned();
        let mut args = Args::new();
        args.insert("profile_id", json!("claude-code"));
        args.insert("id", json!(id));
        assert!(runtime.run_command("accounts.remove", args).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rpc_lists_profiles() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let value = runtime
            .call_method("accounts.list", Map::new())
            .expect("accounts.list");
        assert!(value["profiles"].as_array().is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    fn value_has_account(value: &Value, id: &str) -> bool {
        value
            .get("profiles")
            .and_then(Value::as_array)
            .is_some_and(|profiles| {
                profiles.iter().any(|profile| {
                    profile
                        .get("accounts")
                        .and_then(Value::as_array)
                        .is_some_and(|accounts| {
                            accounts.iter().any(|account| {
                                account.get("id").and_then(Value::as_str) == Some(id)
                            })
                        })
                })
            })
    }
}
