//! Árvore declarativa de painel: validação e conversão para JSON.
//!
//! A interface desenha dados; nenhuma função Lua é chamada ao desenhar. Cada
//! `action` vira um id e a função fica no registro `__forja_actions`.

use crate::manifest::Localized;
use mlua::{Lua, Table, Value};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;

const NODE_LIMIT: usize = 500;
const DEPTH_LIMIT: usize = 4;

/// Um painel pronto para o host desenhar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanelSnapshot {
    pub id: String,
    pub extension: String,
    pub title: Localized,
    pub slot: String,
    pub tree: serde_json::Value,
}

/// Converte a tabela recebida por `forja.ui.panel`.
pub fn build(
    lua: &Lua,
    panel: &Table,
    extension: &str,
    locale: &str,
) -> Result<PanelSnapshot, String> {
    let id: String = panel
        .get("id")
        .map_err(|_| "[tree] panel needs an `id`".to_owned())?;
    if id.trim().is_empty() {
        return Err("[tree] panel needs an `id`".to_owned());
    }
    let slot: String = panel
        .get("slot")
        .map_err(|_| "[tree] panel needs a `slot`".to_owned())?;
    if !matches!(slot.as_str(), "right_panel" | "bench_tab" | "sidebar") {
        return Err(format!("[tree] unknown panel slot `{slot}`"));
    }
    let title = super::api::localized(panel.get("title").ok());
    let tree: Value = panel
        .get("tree")
        .map_err(|_| "[tree] panel needs a `tree`".to_owned())?;
    let actions: Table = lua
        .globals()
        .get("__forja_actions")
        .map_err(|_| "[internal] action registry is missing".to_owned())?;
    let mut ids = HashSet::new();
    let mut count = 0;
    let tree = convert(tree, &id, &actions, locale, 0, &mut count, &mut ids, "tree")?;
    if !tree.is_array() {
        return Err("[tree] `tree` must be a list of nodes".to_owned());
    }
    Ok(PanelSnapshot {
        id,
        extension: extension.to_owned(),
        title,
        slot,
        tree,
    })
}

#[allow(clippy::too_many_arguments, clippy::collapsible_match)]
fn convert(
    value: Value,
    panel_id: &str,
    actions: &Table,
    locale: &str,
    depth: usize,
    count: &mut usize,
    ids: &mut HashSet<String>,
    path: &str,
) -> Result<serde_json::Value, String> {
    match value {
        Value::Nil => Ok(serde_json::Value::Null),
        Value::Boolean(value) => Ok(json!(value)),
        Value::Integer(value) => Ok(json!(value)),
        Value::Number(value) => Ok(json!(value)),
        Value::String(value) => {
            let text = value.to_str().map_err(|error| error.to_string())?.to_string();
            Ok(json!(text))
        }
        Value::Table(table) => {
            *count += 1;
            if *count > NODE_LIMIT {
                return Err(format!("[tree] more than {NODE_LIMIT} nodes"));
            }
            if depth > DEPTH_LIMIT {
                return Err(format!("[tree] `{path}` is deeper than {DEPTH_LIMIT}"));
            }
            let node_type: Option<String> = table.get("type").ok();
            let node_id: Option<String> = table.get("id").ok();
            if node_type.is_none() && table.raw_len() == 0 {
                let pt = table.get::<String>("pt-BR").or_else(|_| table.get::<String>("pt_BR"));
                let en = table.get::<String>("en");
                if pt.is_ok() || en.is_ok() {
                    let text = if locale.to_ascii_lowercase().starts_with("pt") {
                        pt.or(en).unwrap_or_default()
                    } else {
                        en.or(pt).unwrap_or_default()
                    };
                    return Ok(serde_json::Value::String(text));
                }
                if table.pairs::<Value, Value>().next().is_none() {
                    return Ok(serde_json::Value::Array(Vec::new()));
                }
            }
            if let Some(node_type) = &node_type {
                if !matches!(
                    node_type.as_str(),
                    "text" | "button" | "list" | "table" | "form" | "field" | "divider" | "group"
                ) {
                    return Err(format!("[tree] `{path}` has unknown node type `{node_type}`"));
                }
                if let Some(id) = &node_id {
                    if matches!(
                        node_type.as_str(),
                        "button" | "list" | "table" | "form" | "field"
                    ) && !ids.insert(id.clone())
                    {
                        return Err(format!("[tree] `{path}` repeats node id `{id}`"));
                    }
                }
            }
            let mut map = serde_json::Map::new();
            for pair in table.pairs::<Value, Value>() {
                let (key, value) = pair.map_err(|error| error.to_string())?;
                let key = match key {
                    Value::String(text) => text.to_str().map_err(|error| error.to_string())?.to_owned(),
                    Value::Integer(number) => number.to_string(),
                    _ => continue,
                };
                let converted = match value {
                    Value::Function(function) => {
                        if key == "action" {
                            let action_id = format!(
                                "{panel_id}|{}|action",
                                node_id.as_deref().unwrap_or(path)
                            );
                            actions
                                .set(action_id.clone(), function)
                                .map_err(|error| error.to_string())?;
                            map.insert(key, json!(action_id));
                        }
                        continue;
                    }
                    other => {
                        let child_path = if key.chars().all(|ch| ch.is_ascii_digit()) {
                            format!("{path}[{key}]")
                        } else {
                            format!("{path}.{key}")
                        };
                        let child_depth =
                            if node_type.as_deref() == Some("group") && key == "children" {
                                depth + 1
                            } else {
                                depth
                            };
                        convert(
                            other,
                            panel_id,
                            actions,
                            locale,
                            child_depth,
                            count,
                            ids,
                            &child_path,
                        )?
                    }
                };
                map.insert(key, converted);
            }
            if table.raw_len() > 0 && map.keys().all(|key| key.parse::<usize>().is_ok()) {
                let mut items: Vec<(usize, serde_json::Value)> = map
                    .into_iter()
                    .filter_map(|(key, value)| key.parse::<usize>().ok().map(|index| (index, value)))
                    .collect();
                items.sort_by_key(|(index, _)| *index);
                return Ok(serde_json::Value::Array(
                    items.into_iter().map(|(_, value)| value).collect(),
                ));
            }
            if let Some(node_type) = &node_type {
                validate(node_type, &map, path)?;
            }
            Ok(serde_json::Value::Object(map))
        }
        _ => Ok(serde_json::Value::Null),
    }
}

#[allow(clippy::collapsible_match)]
fn validate(node_type: &str, map: &serde_json::Map<String, serde_json::Value>, path: &str) -> Result<(), String> {
    let has = |key: &str| map.get(key).is_some_and(|value| !value.is_null());
    let string = |key: &str| map.get(key).and_then(serde_json::Value::as_str).is_some();
    match node_type {
        "text" => {
            if !has("value") {
                return Err(format!("[tree] `{path}` text needs `value`"));
            }
        }
        "button" => {
            if !string("id") || !has("label") || !string("action") {
                return Err(format!("[tree] `{path}` button needs id, label and action"));
            }
        }
        "list" => {
            if !string("id") || !map.get("items").is_some_and(serde_json::Value::is_array) {
                return Err(format!("[tree] `{path}` list needs id and items"));
            }
        }
        "table" => {
            if !string("id")
                || !map.get("columns").is_some_and(serde_json::Value::is_array)
                || !map.get("rows").is_some_and(serde_json::Value::is_array)
            {
                return Err(format!("[tree] `{path}` table needs id, columns and rows"));
            }
        }
        "form" => {
            if !string("id")
                || !map.get("fields").is_some_and(serde_json::Value::is_array)
                || !map.get("submit").is_some_and(serde_json::Value::is_object)
            {
                return Err(format!("[tree] `{path}` form needs id, fields and submit"));
            }
        }
        "field" => {
            if !string("id") || !string("type") || !has("label") {
                return Err(format!("[tree] `{path}` field needs id, type and label"));
            }
        }
        "group" => {
            if !map.get("children").is_some_and(serde_json::Value::is_array) {
                return Err(format!("[tree] `{path}` group needs children"));
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    #[test]
    fn invalid_node_is_refused_with_its_path() {
        let lua = Lua::new();
        let actions = lua.create_table().unwrap();
        lua.globals().set("__forja_actions", actions).unwrap();
        let table = lua
            .load(
                r#"return {
                    { type = "button", label = "x" },
                }"#,
            )
            .eval::<Table>()
            .unwrap();
        let panel = lua.create_table().unwrap();
        panel.set("id", "p").unwrap();
        panel.set("slot", "right_panel").unwrap();
        panel.set("tree", table).unwrap();
        let error = build(&lua, &panel, "ext", "en").unwrap_err();
        assert!(error.contains("tree[1]"), "{error}");
    }

    #[test]
    fn action_function_becomes_an_id() {
        let lua = Lua::new();
        let actions = lua.create_table().unwrap();
        lua.globals().set("__forja_actions", actions.clone()).unwrap();
        let table = lua
            .load(
                r#"return {
                    { type = "button", id = "go", label = "Go", action = function() end },
                }"#,
            )
            .eval::<Table>()
            .unwrap();
        let panel = lua.create_table().unwrap();
        panel.set("id", "p").unwrap();
        panel.set("slot", "right_panel").unwrap();
        panel.set("tree", table).unwrap();
        let snapshot = build(&lua, &panel, "ext", "en").unwrap();
        let action = snapshot.tree[0]["action"].as_str().unwrap();
        assert_eq!(action, "p|go|action");
        assert!(actions.contains_key(action).unwrap());
    }
}
