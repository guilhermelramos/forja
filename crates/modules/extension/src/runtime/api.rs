//! API `forja.*` vista pelo Lua. Toda função de host confere a capacidade.

use super::io::HostIo;
use super::panel;
use super::{
    CommandArgumentInfo, ExtCommandInfo, HostMessage, HostRequest, HostShared, SettingInfo,
};
use crate::manifest::Localized;
use mlua::{Function, Lua, LuaSerdeExt, Result as LuaResult, Table, Value};
use serde_json::Value as Json;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const IO_WAIT: Duration = Duration::from_secs(10);

/// Instala a tabela `forja` no estado Lua.
pub fn install(
    lua: &Lua,
    shared: Arc<Mutex<HostShared>>,
    io: HostIo,
    messages: Sender<HostMessage>,
) -> LuaResult<()> {
    let globals = lua.globals();
    for name in ["os", "io", "package", "debug", "dofile", "loadfile", "require"] {
        globals.set(name, Value::Nil)?;
    }
    if let Ok(string) = globals.get::<Table>("string") {
        let _ = string.set("dump", Value::Nil);
    }
    globals.set("__forja_actions", lua.create_table()?)?;
    globals.set("__forja_subs", lua.create_table()?)?;
    globals.set("__forja_sub_seq", lua.create_table()?)?;
    globals.set("__forja_loaded", lua.create_table()?)?;
    globals.set("__forja_command_run", lua.create_table()?)?;
    globals.set("__forja_command_available", lua.create_table()?)?;

    let print_shared = Arc::clone(&shared);
    globals.set(
        "print",
        lua.create_function(move |_, text: String| {
            if let Ok(mut state) = print_shared.lock() {
                state.log("info", &text);
            }
            Ok(())
        })?,
    )?;
    let load_shared = Arc::clone(&shared);
    let load_io = io.clone();
    globals.set(
        "load",
        lua.create_function(move |lua, (chunk, name, mode): (String, Option<String>, Option<String>)| {
            if mode.as_deref().is_some_and(|mode| mode.contains('b')) {
                return Err(mlua::Error::RuntimeError(
                    "[capability_denied] binary chunks are not accepted".to_owned(),
                ));
            }
            let name = name.unwrap_or_else(|| "=(load)".to_owned());
            let function = lua
                .load(&chunk)
                .set_name(name)
                .set_mode(mlua::chunk::ChunkMode::Text)
                .into_function()
                .map_err(|error| mlua::Error::RuntimeError(format!("[argument] {error}")))?;
            let _ = (&load_shared, &load_io);
            Ok(function)
        })?,
    )?;

    let forja = lua.create_table()?;
    forja.set("api", "1")?;
    let extension = lua.create_table()?;
    {
        let state = shared.lock().map_err(|_| mlua::Error::RuntimeError("state poisoned".to_owned()))?;
        extension.set("id", state.id.clone())?;
        extension.set("version", state.version.clone())?;
        forja.set("extension", extension)?;
    }

    forja.set("log", log_table(lua, Arc::clone(&shared))?)?;
    forja.set(
        "notify",
        lua.create_function({
            let shared = Arc::clone(&shared);
            let messages = messages.clone();
            move |_, options: Table| {
                let title = localized(options.get("title").ok()).resolve(
                    &shared.lock().map(|state| state.locale.clone()).unwrap_or_default(),
                ).to_owned();
                let text: String = options.get("text").unwrap_or_default();
                let level: String = options.get("level").unwrap_or_else(|_| "info".to_owned());
                if let Ok(mut state) = shared.lock() {
                    let now = Instant::now();
                    state
                        .notify_times
                        .retain(|at| now.duration_since(*at) < Duration::from_secs(60));
                    if state.notify_times.len() >= 10 {
                        return Err(mlua::Error::RuntimeError(
                            "[limit] more than 10 notifications per minute".to_owned(),
                        ));
                    }
                    state.notify_times.push(now);
                }
                let _ = messages.send(HostMessage::Notice { title, text, level });
                Ok(())
            }
        })?,
    )?;
    forja.set("i18n", i18n_table(lua, Arc::clone(&shared))?)?;
    forja.set("store", store_table(lua, Arc::clone(&shared), io.store_path.clone())?)?;
    forja.set("time", time_table(lua)?)?;
    forja.set(
        "project",
        project_table(lua, Arc::clone(&shared))?,
    )?;
    forja.set("bench", bench_table(lua, Arc::clone(&shared))?)?;
    forja.set("load", load_module(lua, Arc::clone(&shared), io.dir.clone())?)?;
    forja.set(
        "commands",
        commands_table(lua, Arc::clone(&shared))?,
    )?;
    forja.set("on", on_function(lua, Arc::clone(&shared))?)?;
    forja.set("ui", ui_table(lua, Arc::clone(&shared), messages.clone())?)?;
    forja.set(
        "settings",
        settings_table(lua, Arc::clone(&shared), io.settings_path.clone())?,
    )?;
    forja.set(
        "terminal",
        terminal_table(lua, Arc::clone(&shared), messages.clone())?,
    )?;
    forja.set("git", git_table(lua, Arc::clone(&shared), io.clone())?)?;
    forja.set("fs", fs_table(lua, Arc::clone(&shared), io.clone())?)?;
    forja.set(
        "http",
        http_table(lua, Arc::clone(&shared), io.clone())?,
    )?;
    globals.set("forja", forja)?;
    Ok(())
}

fn log_table(lua: &Lua, shared: Arc<Mutex<HostShared>>) -> LuaResult<Table> {
    let table = lua.create_table()?;
    for level in ["debug", "info", "warn", "error"] {
        let shared = Arc::clone(&shared);
        let level = level.to_owned();
        table.set(
            level.clone(),
            lua.create_function(move |_, text: String| {
                if let Ok(mut state) = shared.lock() {
                    state.log(&level, &text);
                }
                Ok(())
            })?,
        )?;
    }
    Ok(table)
}

fn i18n_table(lua: &Lua, shared: Arc<Mutex<HostShared>>) -> LuaResult<Table> {
    let table = lua.create_table()?;
    let translate_shared = Arc::clone(&shared);
    table.set(
        "t",
        lua.create_function(move |_, (key, args): (String, Option<Table>)| {
            let state = translate_shared
                .lock()
                .map_err(|_| mlua::Error::RuntimeError("state poisoned".to_owned()))?;
            let mut text = state
                .i18n
                .get(&key)
                .cloned()
                .unwrap_or_else(|| key.clone());
            drop(state);
            if let Some(args) = args {
                for pair in args.pairs::<String, Value>() {
                    let (name, value) = pair?;
                    let value = match value {
                        Value::String(text) => text.to_str().map(|text| text.to_string()).unwrap_or_default(),
                        Value::Integer(number) => number.to_string(),
                        Value::Number(number) => number.to_string(),
                        Value::Boolean(value) => value.to_string(),
                        _ => continue,
                    };
                    text = text.replace(&format!("{{{name}}}"), &value);
                }
            }
            Ok(text)
        })?,
    )?;
    table.set(
        "locale",
        lua.create_function(move |_, ()| {
            Ok(shared
                .lock()
                .map(|state| state.locale.clone())
                .unwrap_or_else(|_| "en".to_owned()))
        })?,
    )?;
    Ok(table)
}

fn store_table(
    lua: &Lua,
    shared: Arc<Mutex<HostShared>>,
    path: std::path::PathBuf,
) -> LuaResult<Table> {
    let table = lua.create_table()?;
    let get_shared = Arc::clone(&shared);
    table.set(
        "get",
        lua.create_function(move |lua, key: String| {
            let state = get_shared
                .lock()
                .map_err(|_| mlua::Error::RuntimeError("state poisoned".to_owned()))?;
            match state.store.get(&key) {
                Some(value) => lua.to_value(value),
                None => Ok(Value::Nil),
            }
        })?,
    )?;
    let set_shared = Arc::clone(&shared);
    let set_path = path.clone();
    table.set(
        "set",
        lua.create_function(move |_, (key, value): (String, Value)| {
            if key.len() > 128 {
                return Err(mlua::Error::RuntimeError(
                    "[limit] store key is longer than 128 characters".to_owned(),
                ));
            }
            let json = value_to_json(value)?;
            let mut state = set_shared
                .lock()
                .map_err(|_| mlua::Error::RuntimeError("state poisoned".to_owned()))?;
            let previous = state.store.insert(key.clone(), json);
            if serde_json::to_vec(&state.store).map(|bytes| bytes.len() > 1_048_576).unwrap_or(false) {
                match previous {
                    Some(previous) => {
                        state.store.insert(key, previous);
                    }
                    None => {
                        state.store.remove(&key);
                    }
                }
                return Err(mlua::Error::RuntimeError(
                    "[limit] store is larger than 1 MB".to_owned(),
                ));
            }
            persist(&set_path, &state.store);
            Ok(())
        })?,
    )?;
    let remove_shared = Arc::clone(&shared);
    table.set(
        "remove",
        lua.create_function(move |_, key: String| {
            let mut state = remove_shared
                .lock()
                .map_err(|_| mlua::Error::RuntimeError("state poisoned".to_owned()))?;
            state.store.remove(&key);
            persist(&path, &state.store);
            Ok(())
        })?,
    )?;
    Ok(table)
}

fn time_table(lua: &Lua) -> LuaResult<Table> {
    let table = lua.create_table()?;
    table.set(
        "now",
        lua.create_function(|_, ()| Ok(chrono::Local::now().to_rfc3339()))?,
    )?;
    table.set(
        "today",
        lua.create_function(|_, ()| Ok(chrono::Local::now().format("%Y-%m-%d").to_string()))?,
    )?;
    Ok(table)
}

fn project_table(lua: &Lua, shared: Arc<Mutex<HostShared>>) -> LuaResult<Table> {
    let table = lua.create_table()?;
    table.set(
        "path",
        lua.create_function(move |_, ()| {
            Ok(shared
                .lock()
                .ok()
                .and_then(|state| state.project.clone())
                .map(|path| path.display().to_string())
                .unwrap_or_default())
        })?,
    )?;
    Ok(table)
}

fn bench_table(lua: &Lua, shared: Arc<Mutex<HostShared>>) -> LuaResult<Table> {
    let table = lua.create_table()?;
    table.set(
        "current",
        lua.create_function(move |lua, ()| {
            let value = shared
                .lock()
                .ok()
                .and_then(|state| state.bench.clone())
                .unwrap_or(Json::Null);
            lua.to_value(&value)
        })?,
    )?;
    Ok(table)
}

fn load_module(lua: &Lua, shared: Arc<Mutex<HostShared>>, dir: std::path::PathBuf) -> LuaResult<Function> {
    lua.create_function(move |lua, path: String| {
        if !valid_lua_path(&path) {
            return Err(mlua::Error::RuntimeError(
                "[argument] forja.load accepts a relative .lua path inside the extension folder"
                    .to_owned(),
            ));
        }
        let loaded: Table = lua.globals().get("__forja_loaded")?;
        if let Ok(value) = loaded.get::<Value>(path.as_str()) {
            if !matches!(value, Value::Nil) {
                return Ok(value);
            }
        }
        if loaded.pairs::<Value, Value>().count() >= 16 {
            return Err(mlua::Error::RuntimeError(
                "[limit] forja.load accepts at most 16 files".to_owned(),
            ));
        }
        let file = dir.join(&path);
        let text = std::fs::read_to_string(&file)
            .map_err(|error| mlua::Error::RuntimeError(format!("[host] {error}")))?;
        if text.len() > 262_144 {
            return Err(mlua::Error::RuntimeError(
                "[limit] Lua file is larger than 256 KB".to_owned(),
            ));
        }
        let value = lua
            .load(&text)
            .set_name(file.display().to_string())
            .set_mode(mlua::chunk::ChunkMode::Text)
            .eval::<Value>()
            .map_err(|error| mlua::Error::RuntimeError(error.to_string()))?;
        loaded.set(path, value.clone())?;
        if let Ok(mut state) = shared.lock() {
            state.log("debug", &format!("loaded {}", file.display()));
        }
        Ok(value)
    })
}

fn commands_table(lua: &Lua, shared: Arc<Mutex<HostShared>>) -> LuaResult<Table> {
    let table = lua.create_table()?;
    table.set(
        "register",
        lua.create_function(move |lua, options: Table| {
            let mut state = shared
                .lock()
                .map_err(|_| mlua::Error::RuntimeError("state poisoned".to_owned()))?;
            if !state.capabilities.commands {
                return Err(mlua::Error::RuntimeError(
                    "[capability_denied] commands".to_owned(),
                ));
            }
            if !state.loading {
                return Err(mlua::Error::RuntimeError(
                    "[argument] commands can only be registered in init.lua".to_owned(),
                ));
            }
            if state.commands.len() >= 50 {
                return Err(mlua::Error::RuntimeError(
                    "[limit] an extension may register 50 commands".to_owned(),
                ));
            }
            let local_id: String = options.get("id")?;
            if local_id.trim().is_empty() {
                return Err(mlua::Error::RuntimeError(
                    "[argument] command needs an `id`".to_owned(),
                ));
            }
            let full_id = format!("ext.{}.{}", state.id, local_id);
            let title = localized(options.get("title").ok());
            if title.pt_br.is_empty() && title.en.is_empty() {
                return Err(mlua::Error::RuntimeError(
                    "[argument] command needs a `title`".to_owned(),
                ));
            }
            let run: Function = options.get("run")?;
            let run_table: Table = lua.globals().get("__forja_command_run")?;
            run_table.set(full_id.clone(), run)?;
            if let Ok(available) = options.get::<Function>("available") {
                let available_table: Table = lua.globals().get("__forja_command_available")?;
                available_table.set(full_id.clone(), available)?;
            }
            let mut arguments = Vec::new();
            if let Ok(list) = options.get::<Table>("arguments") {
                for item in list.sequence_values::<Table>() {
                    let item = item?;
                    let name: String = item.get("name")?;
                    let kind: String = item.get("type").unwrap_or_else(|_| "text".to_owned());
                    let required: bool = item.get("required").unwrap_or(false);
                    let default = item
                        .get::<Value>("default")
                        .ok()
                        .map(value_to_json)
                        .transpose()?
                        .unwrap_or(Json::Null);
                    arguments.push(CommandArgumentInfo {
                        name,
                        kind,
                        required,
                        default,
                    });
                }
            }
            let extension = state.id.clone();
            state.commands.push(ExtCommandInfo {
                id: full_id,
                extension,
                local_id,
                title,
                description: localized(options.get("description").ok()),
                shortcut: options.get("shortcut").ok(),
                arguments,
            });
            Ok(())
        })?,
    )?;
    Ok(table)
}

fn on_function(lua: &Lua, shared: Arc<Mutex<HostShared>>) -> LuaResult<Function> {
    lua.create_function(move |lua, (event, callback): (String, Function)| {
        let state = shared
            .lock()
            .map_err(|_| mlua::Error::RuntimeError("state poisoned".to_owned()))?;
        if !state.capabilities.events.iter().any(|item| item == &event) {
            return Err(mlua::Error::RuntimeError(format!(
                "[capability_denied] events: \"{event}\" was not declared"
            )));
        }
        if !state.loading {
            return Err(mlua::Error::RuntimeError(
                "[argument] forja.on can only be used in init.lua".to_owned(),
            ));
        }
        let subscriptions: Table = lua.globals().get("__forja_subs")?;
        if subscriptions.pairs::<Value, Value>().count() >= 64 {
            return Err(mlua::Error::RuntimeError(
                "[limit] an extension may have 64 subscriptions".to_owned(),
            ));
        }
        let sequence: Table = lua.globals().get("__forja_sub_seq")?;
        let next: u64 = sequence.get::<u64>("n").unwrap_or(0) + 1;
        sequence.set("n", next)?;
        let id = format!("{}:{}", state.id, next);
        drop(state);
        let subscription = lua.create_table()?;
        subscription.set("name", event)?;
        subscription.set("fn", callback)?;
        subscription.set("cancelled", false)?;
        subscriptions.set(id.clone(), subscription)?;
        let handle = lua.create_table()?;
        let cancel_subs = subscriptions.clone();
        let cancel_id = id.clone();
        handle.set(
            "cancel",
            lua.create_function(move |_, ()| {
                if let Ok(subscription) = cancel_subs.get::<Table>(cancel_id.clone()) {
                    let _ = subscription.set("cancelled", true);
                }
                Ok(())
            })?,
        )?;
        Ok(handle)
    })
}

fn ui_table(
    lua: &Lua,
    shared: Arc<Mutex<HostShared>>,
    messages: Sender<HostMessage>,
) -> LuaResult<Table> {
    let table = lua.create_table()?;
    for kind in ["text", "button", "list", "table", "form", "field", "group"] {
        let kind = kind.to_owned();
        table.set(
            kind.clone(),
            lua.create_function(move |_, options: Table| {
                options.set("type", kind.clone())?;
                Ok(options)
            })?,
        )?;
    }
    table.set(
        "divider",
        lua.create_function(|lua, ()| {
            let node = lua.create_table()?;
            node.set("type", "divider")?;
            Ok(node)
        })?,
    )?;
    let panel_shared = Arc::clone(&shared);
    let panel_messages = messages.clone();
    table.set(
        "panel",
        lua.create_function(move |lua, options: Table| {
            let state = panel_shared
                .lock()
                .map_err(|_| mlua::Error::RuntimeError("state poisoned".to_owned()))?;
            if !state.capabilities.ui_panel {
                return Err(mlua::Error::RuntimeError(
                    "[capability_denied] ui.panel".to_owned(),
                ));
            }
            let extension = state.id.clone();
            let locale = state.locale.clone();
            drop(state);
            let snapshot = panel::build(lua, &options, &extension, &locale)
                .map_err(mlua::Error::RuntimeError)?;
            if let Ok(mut state) = panel_shared.lock() {
                state.panels.insert(snapshot.id.clone(), snapshot.clone());
            }
            let _ = panel_messages.send(HostMessage::Panel(snapshot));
            Ok(())
        })?,
    )?;
    let open_shared = Arc::clone(&shared);
    let open_messages = messages.clone();
    table.set(
        "open_url",
        lua.create_function(move |_, url: String| {
            let state = open_shared
                .lock()
                .map_err(|_| mlua::Error::RuntimeError("state poisoned".to_owned()))?;
            if state.capabilities.ui_open_url.is_empty() {
                return Err(mlua::Error::RuntimeError(
                    "[capability_denied] ui.open_url".to_owned(),
                ));
            }
            if !state.user_action {
                return Err(mlua::Error::RuntimeError(
                    "[capability_denied] ui.open_url only runs from a user action".to_owned(),
                ));
            }
            let rest = url
                .strip_prefix("https://")
                .or_else(|| {
                    if is_localhost_url(&url) {
                        url.strip_prefix("http://")
                    } else {
                        None
                    }
                })
                .ok_or_else(|| {
                    mlua::Error::RuntimeError(
                        "[argument] ui.open_url accepts https or http://localhost".to_owned(),
                    )
                })?;
            let host = rest
                .split(['/', '?', '#'])
                .next()
                .and_then(|authority| authority.rsplit('@').next())
                .and_then(|authority| authority.split(':').next())
                .unwrap_or_default();
            let localhost = is_localhost_url(&url);
            if !localhost
                && !state
                    .capabilities
                    .ui_open_url
                    .iter()
                    .any(|allowed| allowed == host)
            {
                return Err(mlua::Error::RuntimeError(format!(
                    "[capability_denied] ui.open_url: host \"{host}\" was not approved"
                )));
            }
            drop(state);
            let (tx, rx) = channel();
            open_messages
                .send(HostMessage::Request(HostRequest::OpenUrl { url, reply: tx }))
                .map_err(|_| mlua::Error::RuntimeError("[internal] host is gone".to_owned()))?;
            match rx.recv_timeout(IO_WAIT) {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(error)) => Err(mlua::Error::RuntimeError(error)),
                Err(_) => Err(mlua::Error::RuntimeError(
                    "[time] open_url did not answer".to_owned(),
                )),
            }
        })?,
    )?;
    let remove_shared = Arc::clone(&shared);
    table.set(
        "remove_panel",
        lua.create_function(move |_, id: String| {
            if let Ok(mut state) = remove_shared.lock() {
                if !state.capabilities.ui_panel {
                    return Err(mlua::Error::RuntimeError(
                        "[capability_denied] ui.panel".to_owned(),
                    ));
                }
                state.panels.remove(&id);
            }
            Ok(())
        })?,
    )?;
    Ok(table)
}

fn settings_table(
    lua: &Lua,
    shared: Arc<Mutex<HostShared>>,
    path: std::path::PathBuf,
) -> LuaResult<Table> {
    let table = lua.create_table()?;
    let schema_shared = Arc::clone(&shared);
    let schema_path = path.clone();
    table.set(
        "schema",
        lua.create_function(move |_, list: Table| {
            let mut state = schema_shared
                .lock()
                .map_err(|_| mlua::Error::RuntimeError("state poisoned".to_owned()))?;
            if !state.capabilities.settings {
                return Err(mlua::Error::RuntimeError(
                    "[capability_denied] settings".to_owned(),
                ));
            }
            if !state.loading {
                return Err(mlua::Error::RuntimeError(
                    "[argument] settings.schema can only be used in init.lua".to_owned(),
                ));
            }
            for item in list.sequence_values::<Table>() {
                let item = item?;
                let name: String = item.get("name")?;
                let kind: String = item.get("type")?;
                if !matches!(kind.as_str(), "text" | "number" | "toggle" | "choice" | "list") {
                    return Err(mlua::Error::RuntimeError(format!(
                        "[argument] unknown setting type `{kind}`"
                    )));
                }
                let default = item
                    .get::<Value>("default")
                    .ok()
                    .map(value_to_json)
                    .transpose()?
                    .unwrap_or(Json::Null);
                let options = item
                    .get::<Table>("options")
                    .ok()
                    .map(|options| {
                        options
                            .sequence_values::<String>()
                            .filter_map(Result::ok)
                            .collect()
                    })
                    .unwrap_or_default();
                let (min, max) = item
                    .get::<Table>("range")
                    .ok()
                    .map(|range| {
                        (
                            range.get::<f64>("min").ok(),
                            range.get::<f64>("max").ok(),
                        )
                    })
                    .unwrap_or((None, None));
                let info = SettingInfo {
                    name: name.clone(),
                    kind,
                    default: default.clone(),
                    label: localized(item.get("label").ok()),
                    help: localized(item.get("help").ok()),
                    options,
                    min,
                    max,
                };
                state.settings.entry(name).or_insert(default);
                state.settings_schema.retain(|entry| entry.name != info.name);
                state.settings_schema.push(info);
            }
            persist(&schema_path, &state.settings);
            Ok(())
        })?,
    )?;
    let get_shared = Arc::clone(&shared);
    table.set(
        "get",
        lua.create_function(move |lua, name: String| {
            let state = get_shared
                .lock()
                .map_err(|_| mlua::Error::RuntimeError("state poisoned".to_owned()))?;
            match state.settings.get(&name) {
                Some(value) => lua.to_value(value),
                None => Ok(Value::Nil),
            }
        })?,
    )?;
    let set_shared = Arc::clone(&shared);
    let set_path = path.clone();
    table.set(
        "set",
        lua.create_function(move |_, (name, value): (String, Value)| {
            let json = value_to_json(value)?;
            let mut state = set_shared
                .lock()
                .map_err(|_| mlua::Error::RuntimeError("state poisoned".to_owned()))?;
            validate_setting(&state, &name, &json)?;
            state.settings.insert(name, json);
            persist(&set_path, &state.settings);
            Ok(())
        })?,
    )?;
    Ok(table)
}

fn terminal_table(
    lua: &Lua,
    shared: Arc<Mutex<HostShared>>,
    messages: Sender<HostMessage>,
) -> LuaResult<Table> {
    let table = lua.create_table()?;
    let list_shared = Arc::clone(&shared);
    let list_messages = messages.clone();
    table.set(
        "list",
        lua.create_function(move |lua, ()| {
            if !list_shared
                .lock()
                .map(|state| state.capabilities.terminal_send)
                .unwrap_or(false)
            {
                return Err(mlua::Error::RuntimeError(
                    "[capability_denied] terminal.send".to_owned(),
                ));
            }
            let value = request_terminal_list(&list_messages)?;
            lua.to_value(&value)
        })?,
    )?;
    let send_shared = Arc::clone(&shared);
    let send_messages = messages;
    table.set(
        "send",
        lua.create_function(move |_, (terminal, text, options): (String, String, Option<Table>)| {
            if !send_shared
                .lock()
                .map(|state| state.capabilities.terminal_send)
                .unwrap_or(false)
            {
                return Err(mlua::Error::RuntimeError(
                    "[capability_denied] terminal.send".to_owned(),
                ));
            }
            if text.len() > 4096 {
                return Err(mlua::Error::RuntimeError(
                    "[limit] terminal text is larger than 4096 bytes".to_owned(),
                ));
            }
            if text.chars().any(|ch| ch.is_control() && ch != '\t') {
                return Err(mlua::Error::RuntimeError(
                    "[argument] terminal text has control characters".to_owned(),
                ));
            }
            if let Ok(mut state) = send_shared.lock() {
                let now = Instant::now();
                state
                    .terminal_times
                    .retain(|at| now.duration_since(*at) < Duration::from_secs(60));
                if state.terminal_times.len() >= 10 {
                    return Err(mlua::Error::RuntimeError(
                        "[limit] more than 10 terminal sends per minute".to_owned(),
                    ));
                }
                state.terminal_times.push(now);
            }
            let enter = options
                .and_then(|options| options.get::<bool>("enter").ok())
                .unwrap_or(false);
            let (tx, rx) = channel();
            send_messages
                .send(HostMessage::Request(HostRequest::TerminalSend {
                    terminal,
                    text,
                    enter,
                    reply: tx,
                }))
                .map_err(|_| mlua::Error::RuntimeError("[internal] host is gone".to_owned()))?;
            match rx.recv_timeout(IO_WAIT) {
                Ok(Ok(value)) => {
                    let _ = value;
                    Ok(())
                }
                Ok(Err(error)) => Err(mlua::Error::RuntimeError(error)),
                Err(_) => Err(mlua::Error::RuntimeError("[time] terminal did not answer".to_owned())),
            }
        })?,
    )?;
    Ok(table)
}

fn git_table(
    lua: &Lua,
    shared: Arc<Mutex<HostShared>>,
    io: HostIo,
) -> LuaResult<Table> {
    let table = lua.create_table()?;
    let status_shared = Arc::clone(&shared);
    let status_io = io.clone();
    table.set(
        "status",
        lua.create_function(move |lua, ()| {
            let io = current_io(&status_shared, &status_io);
            let rows = io.git_status().map_err(mlua::Error::RuntimeError)?;
            let value: Vec<Json> = rows
                .into_iter()
                .map(|(path, state)| serde_json::json!({ "path": path, "state": state }))
                .collect();
            lua.to_value(&value)
        })?,
    )?;
    let branch_shared = Arc::clone(&shared);
    let branch_io = io.clone();
    table.set(
        "branch",
        lua.create_function(move |_, ()| {
            let io = current_io(&branch_shared, &branch_io);
            io.git_branch().map_err(mlua::Error::RuntimeError)
        })?,
    )?;
    let remote_shared = Arc::clone(&shared);
    let remote_io = io.clone();
    table.set(
        "remote_url",
        lua.create_function(move |_, name: String| {
            let io = current_io(&remote_shared, &remote_io);
            let value = io.git_remote_url(&name).map_err(mlua::Error::RuntimeError)?;
            Ok(value)
        })?,
    )?;
    let log_shared = Arc::clone(&shared);
    let log_io = io.clone();
    table.set(
        "log",
        lua.create_function(move |lua, options: Option<Table>| {
            let io = current_io(&log_shared, &log_io);
            let limit = options
                .as_ref()
                .and_then(|options| options.get::<usize>("limit").ok())
                .unwrap_or(50)
                .min(500);
            let since = options
                .as_ref()
                .and_then(|options| options.get::<String>("since").ok());
            let rows = io
                .git_log(limit, since.as_deref())
                .map_err(mlua::Error::RuntimeError)?;
            lua.to_value(&rows)
        })?,
    )?;
    let diff_shared = Arc::clone(&shared);
    let diff_io = io;
    table.set(
        "diff_stat",
        lua.create_function(move |lua, ()| {
            let io = current_io(&diff_shared, &diff_io);
            let value = io.git_diff_stat().map_err(mlua::Error::RuntimeError)?;
            lua.to_value(&value)
        })?,
    )?;
    Ok(table)
}

fn fs_table(
    lua: &Lua,
    shared: Arc<Mutex<HostShared>>,
    io: HostIo,
) -> LuaResult<Table> {
    let table = lua.create_table()?;
    let list_shared = Arc::clone(&shared);
    let list_io = io.clone();
    table.set(
        "list",
        lua.create_function(move |lua, pattern: String| {
            let io = current_io(&list_shared, &list_io);
            let rows = io.fs_list(&pattern).map_err(mlua::Error::RuntimeError)?;
            lua.to_value(&rows)
        })?,
    )?;
    let exists_shared = Arc::clone(&shared);
    let exists_io = io.clone();
    table.set(
        "exists",
        lua.create_function(move |_, path: String| {
            let io = current_io(&exists_shared, &exists_io);
            let target = io.fs_path(&path, false).map_err(mlua::Error::RuntimeError)?;
            Ok(target.exists())
        })?,
    )?;
    let read_shared = Arc::clone(&shared);
    let read_io = io.clone();
    table.set(
        "read",
        lua.create_function(move |_, path: String| {
            let io = current_io(&read_shared, &read_io);
            io.fs_read(&path).map_err(mlua::Error::RuntimeError)
        })?,
    )?;
    let write_shared = Arc::clone(&shared);
    let write_io = io;
    table.set(
        "write",
        lua.create_function(move |_, (path, text): (String, String)| {
            let io = current_io(&write_shared, &write_io);
            io.fs_write(&path, &text).map_err(mlua::Error::RuntimeError)
        })?,
    )?;
    Ok(table)
}

fn http_table(
    lua: &Lua,
    shared: Arc<Mutex<HostShared>>,
    io: HostIo,
) -> LuaResult<Table> {
    let table = lua.create_table()?;
    for method in ["get", "post"] {
        let shared = Arc::clone(&shared);
        let io = io.clone();
        let method = method.to_owned();
        table.set(
            method.clone(),
            lua.create_function(move |lua, (url, body, options): (String, Option<String>, Option<Table>)| {
                let mut state = shared
                    .lock()
                    .map_err(|_| mlua::Error::RuntimeError("state poisoned".to_owned()))?;
                if state.capabilities.http.is_empty() {
                    return Err(mlua::Error::RuntimeError(
                        "[capability_denied] http".to_owned(),
                    ));
                }
                let now = Instant::now();
                state.http_times.retain(|at| now.duration_since(*at) < Duration::from_secs(60));
                if state.http_times.len() >= 30 {
                    return Err(mlua::Error::RuntimeError(
                        "[limit] more than 30 HTTP requests per minute".to_owned(),
                    ));
                }
                state.http_times.push(now);
                drop(state);
                let mut headers = Vec::new();
                let mut timeout_ms = 5000;
                if let Some(options) = options {
                    timeout_ms = options.get::<u64>("timeout_ms").unwrap_or(5000);
                    if let Ok(list) = options.get::<Table>("headers") {
                        for pair in list.pairs::<String, String>() {
                            let (name, value) = pair?;
                            headers.push((name, value));
                        }
                    }
                }
                let value = io
                    .http(
                        &method,
                        &url,
                        if method == "post" { body.as_deref() } else { None },
                        &headers,
                        timeout_ms,
                    )
                    .map_err(mlua::Error::RuntimeError)?;
                lua.to_value(&value)
            })?,
        )?;
    }
    Ok(table)
}

fn current_io(shared: &Arc<Mutex<HostShared>>, io: &HostIo) -> HostIo {
    let mut io = io.clone();
    if let Ok(state) = shared.lock() {
        io.project = state.project.clone();
    }
    io
}

fn request_terminal_list(messages: &Sender<HostMessage>) -> LuaResult<Json> {
    let (tx, rx) = channel();
    messages
        .send(HostMessage::Request(HostRequest::TerminalList { reply: tx }))
        .map_err(|_| mlua::Error::RuntimeError("[internal] host is gone".to_owned()))?;
    match rx.recv_timeout(IO_WAIT) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(mlua::Error::RuntimeError(error)),
        Err(_) => Err(mlua::Error::RuntimeError(
            "[time] terminal list did not answer".to_owned(),
        )),
    }
}

/// Localized a partir de string ou tabela Lua.
pub(super) fn localized(value: Option<Value>) -> Localized {
    match value {
        Some(Value::String(text)) => {
            let text = text.to_str().map(|value| value.to_string()).unwrap_or_default();
            Localized {
                pt_br: text.clone(),
                en: text,
            }
        }
        Some(Value::Table(table)) => Localized {
            pt_br: table
                .get::<String>("pt-BR")
                .or_else(|_| table.get::<String>("pt_BR"))
                .unwrap_or_default(),
            en: table.get::<String>("en").unwrap_or_default(),
        },
        _ => Localized::default(),
    }
}

/// Converte um valor Lua em JSON, ignorando funções.
pub fn value_to_json(value: Value) -> LuaResult<Json> {
    match value {
        Value::Nil => Ok(Json::Null),
        Value::Boolean(value) => Ok(Json::Bool(value)),
        Value::Integer(value) => Ok(Json::from(value)),
        Value::Number(value) => Ok(Json::from(value)),
        Value::String(value) => Ok(Json::String(
            value.to_str().map(|text| text.to_string()).unwrap_or_default(),
        )),
        Value::Table(table) => {
            if table.raw_len() > 0 {
                let mut items = Vec::new();
                for item in table.sequence_values::<Value>() {
                    items.push(value_to_json(item?)?);
                }
                Ok(Json::Array(items))
            } else {
                let mut map = serde_json::Map::new();
                for pair in table.pairs::<Value, Value>() {
                    let (key, value) = pair?;
                    let key = match key {
                        Value::String(text) => text.to_str().map(|text| text.to_string()).unwrap_or_default(),
                        Value::Integer(number) => number.to_string(),
                        _ => continue,
                    };
                    map.insert(key, value_to_json(value)?);
                }
                Ok(Json::Object(map))
            }
        }
        _ => Ok(Json::Null),
    }
}

fn persist(path: &std::path::Path, map: &std::collections::BTreeMap<String, Json>) {
    if let Ok(bytes) = serde_json::to_vec_pretty(map) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = forja_kernel::store::write_atomic_bytes(path, &bytes);
    }
}

fn validate_setting(state: &HostShared, name: &str, value: &Json) -> LuaResult<()> {
    let Some(schema) = state.schema(name) else {
        return Err(mlua::Error::RuntimeError(format!(
            "[argument] unknown setting `{name}`"
        )));
    };
    let valid = match schema.kind.as_str() {
        "text" => value.is_string(),
        "number" => value
            .as_f64()
            .is_some_and(|number| schema.min.is_none_or(|min| number >= min) && schema.max.is_none_or(|max| number <= max)),
        "toggle" => value.is_boolean(),
        "choice" => value
            .as_str()
            .is_some_and(|text| schema.options.iter().any(|option| option == text)),
        "list" => value
            .as_array()
            .is_some_and(|items| items.iter().all(Json::is_string)),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(mlua::Error::RuntimeError(format!(
            "[argument] value for `{name}` does not match {}",
            schema.kind
        )))
    }
}

/// Chama o `run` de um comando registrado.
pub fn call_command(
    lua: &Lua,
    shared: &Arc<Mutex<HostShared>>,
    deadline: &Arc<Mutex<Instant>>,
    command: &str,
    args: &Json,
    timeout_ms: u64,
) -> Result<Json, String> {
    let run: Function = lua
        .globals()
        .get::<Table>("__forja_command_run")
        .and_then(|table| table.get::<Function>(command))
        .map_err(|_| format!("[argument] unknown extension command `{command}`"))?;
    let (bench, locale) = shared
        .lock()
        .map(|state| (state.bench.clone().unwrap_or(Json::Null), state.locale.clone()))
        .unwrap_or((Json::Null, "en".to_owned()));
    let args = lua.to_value(args).map_err(|error| error.to_string())?;
    let context = lua.create_table().map_err(|error| error.to_string())?;
    context
        .set("bench", lua.to_value(&bench).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    context.set("locale", locale).map_err(|error| error.to_string())?;
    if let Ok(mut deadline) = deadline.lock() {
        *deadline = Instant::now() + Duration::from_millis(timeout_ms);
    }
    let result: Value = run
        .call((args, context))
        .map_err(|error| error.to_string())?;
    value_to_json(result).map_err(|error| error.to_string())
}

/// Chama uma ação de painel.
pub fn call_action(
    lua: &Lua,
    _shared: &Arc<Mutex<HostShared>>,
    deadline: &Arc<Mutex<Instant>>,
    action: &str,
    event: &Json,
    timeout_ms: u64,
) -> Result<Json, String> {
    let function: Function = lua
        .globals()
        .get::<Table>("__forja_actions")
        .and_then(|table| table.get::<Function>(action))
        .map_err(|_| format!("[argument] unknown panel action `{action}`"))?;
    let event = lua.to_value(event).map_err(|error| error.to_string())?;
    if let Ok(mut deadline) = deadline.lock() {
        *deadline = Instant::now() + Duration::from_millis(timeout_ms);
    }
    let result: Value = function.call(event).map_err(|error| error.to_string())?;
    value_to_json(result).map_err(|error| error.to_string())
}

/// Entrega um evento a todas as assinaturas vivas.
pub fn dispatch_event(
    lua: &Lua,
    shared: &Arc<Mutex<HostShared>>,
    deadline: &Arc<Mutex<Instant>>,
    name: &str,
    payload: &Json,
    timeout_ms: u64,
) {
    let Ok(subscriptions) = lua.globals().get::<Table>("__forja_subs") else {
        return;
    };
    let Ok(payload) = lua.to_value(payload) else {
        return;
    };
    for pair in subscriptions.pairs::<String, Table>() {
        let Ok((id, subscription)) = pair else {
            continue;
        };
        let event: String = subscription.get("name").unwrap_or_default();
        if event != name {
            continue;
        }
        let cancelled: bool = subscription.get("cancelled").unwrap_or(false);
        if cancelled {
            continue;
        }
        let Ok(function) = subscription.get::<Function>("fn") else {
            continue;
        };
        if let Ok(mut deadline) = deadline.lock() {
            *deadline = Instant::now() + Duration::from_millis(timeout_ms);
        }
        if let Err(error) = function.call::<Value>(payload.clone()) {
            if let Ok(mut state) = shared.lock() {
                state.log("error", &format!("event {name} ({id}): {error}"));
            }
        }
    }
}

/// Aplica uma configuração vinda da interface.
pub fn set_setting(
    lua: &Lua,
    shared: &Arc<Mutex<HostShared>>,
    deadline: &Arc<Mutex<Instant>>,
    name: &str,
    value: &Json,
    timeout_ms: u64,
) {
    let path = {
        let Ok(mut state) = shared.lock() else {
            return;
        };
        if let Err(error) = validate_setting(&state, name, value) {
            state.log("warn", &error.to_string());
            return;
        }
        state.settings.insert(name.to_owned(), value.clone());
        let path = state
            .data_dir
            .join("extensions")
            .join(&state.id)
            .join("settings.json");
        persist(&path, &state.settings);
        path
    };
    let _ = path;
    dispatch_event(
        lua,
        shared,
        deadline,
        "settings.changed",
        &serde_json::json!({ "key": name }),
        timeout_ms,
    );
}

/// `forja.load` accepts only a `.lua` file below the extension folder.
fn valid_lua_path(path: &str) -> bool {
    if !path.ends_with(".lua") {
        return false;
    }
    let path = std::path::Path::new(path);
    !path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

/// `http://localhost` must be the whole host, not a prefix of another one.
pub(super) fn is_localhost_url(url: &str) -> bool {
    url.strip_prefix("http://localhost").is_some_and(|rest| {
        rest.is_empty() || rest.starts_with('/') || rest.starts_with(':') || rest.starts_with('?') || rest.starts_with('#')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localized_accepts_string_and_table() {
        let lua = Lua::new();
        let text = localized(Some(Value::String(lua.create_string("Oi").unwrap())));
        assert_eq!(text.pt_br, "Oi");
        assert_eq!(text.en, "Oi");
        let table = lua.create_table().unwrap();
        table.set("pt-BR", "Olá").unwrap();
        table.set("en", "Hello").unwrap();
        let text = localized(Some(Value::Table(table)));
        assert_eq!(text.resolve("pt-BR"), "Olá");
        assert_eq!(text.resolve("en"), "Hello");
    }

    #[test]
    fn loaded_lua_paths_stay_inside_the_extension() {
        assert!(valid_lua_path("lib/helpers.lua"));
        assert!(valid_lua_path("./lib/helpers.lua"));
        assert!(!valid_lua_path("lib/helpers.txt"));
        assert!(!valid_lua_path("../outside.lua"));
        assert!(!valid_lua_path("lib/../../outside.lua"));
        assert!(!valid_lua_path("C:/outside.lua"));
        assert!(!valid_lua_path("/outside.lua"));
    }

    #[test]
    fn localhost_accepts_only_the_localhost_host() {
        assert!(is_localhost_url("http://localhost:5173/app"));
        assert!(is_localhost_url("http://localhost/"));
        assert!(is_localhost_url("http://localhost"));
        assert!(!is_localhost_url("http://localhost.evil.example/app"));
        assert!(!is_localhost_url("http://localhost-twin/app"));
        assert!(!is_localhost_url("https://example.com"));
    }
}
