//! Missing command values asked in the Anvil, one field at a time.
//!
//! A command declares the values it needs with [`forja_kernel::CommandField`].
//! A choice is a filterable list built from another command's result; text uses
//! the search field itself. The shell knows nothing about benches or projects:
//! the module names the source command and the fields to read.

use crate::app::ForjaApp;
use forja_kernel::{Args, CommandField, CommandFieldKind};
use serde_json::Value;

#[derive(Clone, Debug)]
pub(crate) struct AskOption {
    pub(crate) label: String,
    pub(crate) detail: String,
    pub(crate) value: String,
}

/// A command waiting for values. Fields are asked in order.
#[derive(Clone, Debug)]
pub(crate) struct Ask {
    command: String,
    fields: Vec<CommandField>,
    index: usize,
    args: serde_json::Map<String, Value>,
    pub(crate) options: Vec<AskOption>,
}

impl Ask {
    fn new(command: &str, fields: Vec<CommandField>) -> Self {
        Self {
            command: command.to_owned(),
            fields,
            index: 0,
            args: serde_json::Map::new(),
            options: Vec::new(),
        }
    }

    pub(crate) fn field(&self) -> Option<&CommandField> {
        self.fields.get(self.index)
    }

    pub(crate) fn is_text(&self) -> bool {
        matches!(
            self.field().map(|field| &field.kind),
            Some(CommandFieldKind::Text)
        )
    }

    fn source(
        &self,
    ) -> Option<(
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        Option<&'static str>,
    )> {
        match &self.field()?.kind {
            CommandFieldKind::Choice {
                command,
                list,
                label,
                value,
                detail,
            } => Some((command, list, label, value, *detail)),
            CommandFieldKind::Text => None,
        }
    }

    /// Stores the value of the current field. Returns true when every field is
    /// filled and the command can run.
    fn push_value(&mut self, value: &str) -> bool {
        if let Some(field) = self.field() {
            self.args
                .insert(field.arg.to_owned(), Value::String(value.to_owned()));
            self.index += 1;
        }
        self.index >= self.fields.len()
    }

    fn finish(self) -> (String, serde_json::Map<String, Value>) {
        (self.command, self.args)
    }
}

impl ForjaApp {
    /// Starts asking for the values a command declared. Returns false when the
    /// command can run without arguments.
    pub(crate) fn start_ask(&mut self, command: &str) -> bool {
        let Some(spec) = self.runtime.registry().command(command) else {
            return false;
        };
        let fields = if spec.fields.is_empty() && command == "terminal.focus" {
            vec![terminal_focus_field()]
        } else {
            spec.fields.clone()
        };
        if fields.is_empty() {
            return false;
        }
        self.anvil_open = true;
        self.anvil_ask = Some(Ask::new(command, fields));
        self.anvil_rail_select = false;
        self.anvil_muster_title = false;
        self.anvil_query.clear();
        self.anvil_query_seen.clear();
        self.anvil_selected = 0;
        self.anvil_selection_explicit = false;
        self.anvil_selection_id = None;
        self.anvil_selection_missing = false;
        self.focus_anvil = true;
        self.load_ask_options();
        true
    }

    pub(crate) fn load_ask_options(&mut self) {
        let Some((source, list, label, value, detail)) =
            self.anvil_ask.as_ref().and_then(Ask::source)
        else {
            if let Some(ask) = self.anvil_ask.as_mut() {
                ask.options.clear();
            }
            return;
        };
        let options = match self.runtime.run_command(source, Args::new()) {
            Ok(result) => ask_options(&result, list, label, value, detail),
            Err(_) => Vec::new(),
        };
        if let Some(ask) = self.anvil_ask.as_mut() {
            ask.options = options;
        }
    }

    pub(crate) fn apply_ask_choice(&mut self, index: usize) {
        let Some(value) = self
            .anvil_ask
            .as_ref()
            .and_then(|ask| ask.options.get(index))
            .map(|option| option.value.clone())
        else {
            return;
        };
        self.apply_ask_value(value);
    }

    pub(crate) fn apply_ask_value(&mut self, value: String) {
        let done = {
            let Some(ask) = self.anvil_ask.as_mut() else {
                return;
            };
            ask.push_value(&value)
        };
        if !done {
            self.anvil_query.clear();
            self.anvil_query_seen.clear();
            self.anvil_selected = 0;
            self.anvil_selection_explicit = false;
            self.anvil_selection_id = None;
            self.anvil_selection_missing = false;
            self.focus_anvil = true;
            self.load_ask_options();
            return;
        }
        let Some(ask) = self.anvil_ask.take() else {
            return;
        };
        let (command, args) = ask.finish();
        self.anvil_open = false;
        self.anvil_query.clear();
        self.anvil_query_seen.clear();
        self.anvil_selected = 0;
        self.run_command_args(&command, Args::from_map(args));
    }
}

// The terminal command also serves RPC callers that already supply an id.
// The Anvil must collect one when invoked without arguments.
fn terminal_focus_field() -> CommandField {
    CommandField {
        arg: "terminal_id",
        label_key: "terminal.command.focus",
        kind: CommandFieldKind::Choice {
            command: "terminal.list",
            list: "terminals",
            label: "title",
            value: "terminal_id",
            detail: Some("bench_path"),
        },
    }
}

/// Reads the options of a question from the result of its source command.
fn ask_options(
    result: &Value,
    list: &str,
    label: &str,
    value: &str,
    detail: Option<&str>,
) -> Vec<AskOption> {
    let array = result
        .get(list)
        .and_then(Value::as_array)
        .or_else(|| result.as_array());
    let Some(array) = array else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|item| {
            if let Some(text) = item.as_str() {
                return Some(AskOption {
                    label: text.to_owned(),
                    detail: String::new(),
                    value: text.to_owned(),
                });
            }
            let value = item.get(value).and_then(Value::as_str)?;
            let label = item.get(label).and_then(Value::as_str).unwrap_or(value);
            let detail = detail
                .and_then(|field| item.get(field))
                .and_then(Value::as_str)
                .unwrap_or_default();
            Some(AskOption {
                label: label.to_owned(),
                detail: detail.to_owned(),
                value: value.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_focus_asks_for_an_existing_terminal_id() {
        let field = terminal_focus_field();
        let mut ask = Ask::new("terminal.focus", vec![field]);
        let result = serde_json::json!({"terminals": [
            {"terminal_id": "7", "title": "Shell", "bench_path": "project"}
        ]});
        ask.options = ask_options(&result, "terminals", "title", "terminal_id", Some("bench_path"));
        assert_eq!(ask.options[0].value, "7");
        assert!(ask.push_value("7"));
        let (command, args) = ask.finish();
        assert_eq!(command, "terminal.focus");
        assert_eq!(args["terminal_id"], "7");
    }

    #[test]
    fn ask_collects_every_field_in_order_before_running() {
        let fields = vec![
            CommandField {
                arg: "project_id",
                label_key: "bench.ask.project",
                kind: CommandFieldKind::Choice {
                    command: "project.list",
                    list: "projects",
                    label: "name",
                    value: "id",
                    detail: None,
                },
            },
            CommandField {
                arg: "name",
                label_key: "bench.ask.name",
                kind: CommandFieldKind::Text,
            },
        ];
        let mut ask = Ask::new("bench.create", fields);
        assert!(!ask.push_value("p1"), "the name is still missing");
        assert_eq!(
            ask.args.get("project_id"),
            Some(&Value::String("p1".into()))
        );
        assert!(ask.push_value("minha bancada"), "all fields are filled");
        let (command, args) = ask.finish();
        assert_eq!(command, "bench.create");
        assert_eq!(
            args.get("name"),
            Some(&Value::String("minha bancada".into()))
        );
    }

    #[test]
    fn ask_options_read_the_label_value_and_context_fields() {
        let result = serde_json::json!({
            "benches": [
                { "bench_id": "b1", "name": "Uma", "branch": "main" },
                { "bench_id": "b2", "name": "Duas" }
            ]
        });
        let options = ask_options(&result, "benches", "name", "bench_id", Some("branch"));
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].label, "Uma");
        assert_eq!(options[0].value, "b1");
        assert_eq!(options[0].detail, "main");
        assert_eq!(options[1].detail, "");
    }
}
