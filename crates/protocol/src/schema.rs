//! Method schemas exported by `meta.methods`.
//!
//! The registry is the single source for the CLI and for the future MCP
//! bridge: name, i18n description key, parameter schema, result schema and
//! safety annotations (`effect`, `network`, `idempotent`, `exposed_mcp`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One RPC parameter as the CLI and MCP clients see it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParamSchema {
    /// Code name in English, used as the JSON key.
    pub name: String,
    /// Human description shown by `forja <module> <method> --help`.
    pub description: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub kind: ParamKind,
    /// Extra CLI flags accepted for the same parameter (for example a
    /// Portuguese alias used in the product examples).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

impl ParamSchema {
    pub fn required(name: impl Into<String>, kind: ParamKind) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            required: true,
            kind,
            aliases: Vec::new(),
        }
    }

    pub fn optional(name: impl Into<String>, kind: ParamKind) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            required: false,
            kind,
            aliases: Vec::new(),
        }
    }

    pub fn describe(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }
}

/// JSON shape of a parameter. The handler still validates the concrete value.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    #[default]
    String,
    Integer,
    Number,
    Boolean,
    Json,
}

/// What a method does to the world. Also drives `exposed_mcp` and the MCP
/// annotations (`readOnlyHint`, `destructiveHint`).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MethodEffect {
    #[default]
    Read,
    Write,
    Destructive,
}

impl MethodEffect {
    /// Read methods are exposed to MCP by default; write and destructive
    /// methods need an explicit opt-in.
    pub fn default_exposed(self) -> bool {
        matches!(self, Self::Read)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Destructive => "destructive",
        }
    }
}

/// Optional result schema, validated by the future MCP bridge.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ResultSchema {
    /// i18n key with a human description of the result.
    pub description_key: String,
    /// Description resolved by the app for CLI/MCP help. Empty until then.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// JSON Schema object. `null` means "declared but not described".
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub schema: Value,
}

impl ResultSchema {
    pub fn new(description_key: impl Into<String>) -> Self {
        Self {
            description_key: description_key.into(),
            description: String::new(),
            schema: Value::Null,
        }
    }

    pub fn schema(mut self, schema: Value) -> Self {
        self.schema = schema;
        self
    }
}

/// One module-owned RPC method.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Method {
    /// Dotted id in English, for example `hello.greet`.
    pub name: String,
    /// i18n key of the human description.
    pub description_key: String,
    /// Description resolved by the app for CLI/MCP help. Empty until then.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default)]
    pub params: Vec<ParamSchema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ResultSchema>,
    /// `read`, `write` or `destructive`.
    pub effect: MethodEffect,
    /// Reaches the network or another machine (`openWorldHint`).
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub idempotent: bool,
    /// Exposed by the MCP bridge. Defaults to true only for read methods.
    #[serde(default)]
    pub exposed_mcp: bool,
}

impl Method {
    pub fn new(name: impl Into<String>, description_key: impl Into<String>) -> Self {
        let effect = MethodEffect::Read;
        Self {
            name: name.into(),
            description_key: description_key.into(),
            description: String::new(),
            params: Vec::new(),
            result: None,
            effect,
            network: false,
            idempotent: false,
            exposed_mcp: effect.default_exposed(),
        }
    }

    pub fn param(mut self, param: ParamSchema) -> Self {
        self.params.push(param);
        self
    }

    pub fn result(mut self, result: ResultSchema) -> Self {
        self.result = Some(result);
        self
    }

    pub fn effect(mut self, effect: MethodEffect) -> Self {
        self.effect = effect;
        self.exposed_mcp = effect.default_exposed();
        self
    }

    pub fn network(mut self, network: bool) -> Self {
        self.network = network;
        self
    }

    pub fn idempotent(mut self, idempotent: bool) -> Self {
        self.idempotent = idempotent;
        self
    }

    /// Overrides the effect-derived MCP exposure.
    pub fn expose_mcp(mut self, exposed: bool) -> Self {
        self.exposed_mcp = exposed;
        self
    }

    /// The first dot separates the CLI group from the action.
    pub fn group_and_action(&self) -> (&str, &str) {
        match self.name.split_once('.') {
            Some((group, action)) => (group, action),
            None => (self.name.as_str(), ""),
        }
    }

    /// Checks required parameters and basic JSON kinds. Both the shell and the
    /// CLI apply this before calling a module handler.
    pub fn check_params(&self, params: &serde_json::Map<String, Value>) -> Result<(), String> {
        for param in &self.params {
            match params.get(&param.name) {
                None | Some(Value::Null) if param.required => {
                    return Err(format!("missing required parameter `{}`", param.name));
                }
                None | Some(Value::Null) => {}
                Some(value) => {
                    if !kind_matches(param.kind, value) {
                        return Err(format!(
                            "parameter `{}` must be {}",
                            param.name,
                            kind_name(param.kind)
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

/// Compatibility alias used by earlier kernel code. Prefer [`Method`].
pub type MethodSchema = Method;

fn kind_matches(kind: ParamKind, value: &Value) -> bool {
    match kind {
        ParamKind::String => value.is_string(),
        ParamKind::Integer => value.is_i64() || value.is_u64(),
        ParamKind::Number => value.is_number(),
        ParamKind::Boolean => value.is_boolean(),
        ParamKind::Json => true,
    }
}

fn kind_name(kind: ParamKind) -> &'static str {
    match kind {
        ParamKind::String => "a string",
        ParamKind::Integer => "an integer",
        ParamKind::Number => "a number",
        ParamKind::Boolean => "a boolean",
        ParamKind::Json => "JSON",
    }
}

/// Deterministic catalog returned by `meta.methods`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MethodCatalog {
    #[serde(default)]
    pub methods: Vec<Method>,
}

impl MethodCatalog {
    pub fn new(mut methods: Vec<Method>) -> Self {
        methods.sort_by(|a, b| a.name.cmp(&b.name));
        Self { methods }
    }

    pub fn find(&self, name: &str) -> Option<&Method> {
        self.methods.iter().find(|method| method.name == name)
    }

    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn params(pairs: &[(&str, Value)]) -> serde_json::Map<String, Value> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect()
    }

    #[test]
    fn required_params_and_kinds_are_checked() {
        let schema = Method::new("hello.greet", "hello.method.greet.description").param(
            ParamSchema::required("name", ParamKind::String).alias("nome"),
        );
        assert!(schema.check_params(&params(&[("name", json!("Ana"))])).is_ok());
        assert!(schema.check_params(&params(&[])).is_err());
        assert!(schema.check_params(&params(&[("name", json!(3))])).is_err());
        assert!(schema.check_params(&params(&[("name", Value::Null)])).is_err());
    }

    #[test]
    fn catalog_is_sorted_and_finds_methods() {
        let catalog = MethodCatalog::new(vec![
            Method::new("zeta.run", "zeta.run.description"),
            Method::new("alpha.run", "alpha.run.description"),
        ]);
        assert_eq!(catalog.methods[0].name, "alpha.run");
        assert!(catalog.find("zeta.run").is_some());
        assert!(catalog.find("missing").is_none());
    }

    #[test]
    fn group_and_action_splits_at_the_first_dot() {
        let method = Method::new("hello.greet", "hello.method.greet.description");
        assert_eq!(method.group_and_action(), ("hello", "greet"));
        let single = Method::new("methods", "meta.description");
        assert_eq!(single.group_and_action(), ("methods", ""));
    }

    #[test]
    fn effect_controls_default_mcp_exposure() {
        let read = Method::new("bench.list", "bench.list.description");
        assert!(read.exposed_mcp);
        let write = read.clone().effect(MethodEffect::Write);
        assert!(!write.exposed_mcp);
        let destructive = read.clone().effect(MethodEffect::Destructive);
        assert!(!destructive.exposed_mcp);
        let explicit = write.clone().expose_mcp(true);
        assert!(explicit.exposed_mcp);
    }

    #[test]
    fn method_serializes_every_safety_field() {
        let method = Method::new("bench.create", "bench.method.create.description")
            .effect(MethodEffect::Write)
            .network(false)
            .idempotent(false)
            .param(ParamSchema::required("name", ParamKind::String))
            .result(ResultSchema::new("bench.method.create.result").schema(json!({"type": "object"})));
        let value = serde_json::to_value(&method).unwrap();
        assert_eq!(value["effect"], "write");
        assert_eq!(value["network"], false);
        assert_eq!(value["idempotent"], false);
        assert_eq!(value["exposed_mcp"], false);
        assert_eq!(value["result"]["description_key"], "bench.method.create.result");
    }
}
