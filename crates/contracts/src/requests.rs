//! Pedidos e balcões: o pedido em forma comum e as mensagens entre `request`, `counter`
//! e os balcões (`counter_<nome>`). Ids de comando e formas de JSON estão em
//! `design/contracts.md`, seção "Pedidos e balcões".
//!
//! Todo balcão roda HTTP fora da thread da interface. Por isso os comandos `<balcão>.*`
//! respondem na hora com `{ "started": true }` e o resultado chega pelo evento
//! `counter.result` ([`CounterResult`]), correlacionado por `op_id` quando quem chama o envia.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Tipo de pedido. `Change` é pull request ou merge request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestKind {
    #[default]
    Issue,
    Change,
}

/// Estado comum a todos os balcões. Pedido local usa só `open` e `closed`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestState {
    #[default]
    Open,
    Closed,
    Merged,
}

/// Escopo pedido a um balcão. A tela traduz os seus filtros para estes quatro.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestScope {
    /// Atribuído a mim ou aberto por mim.
    Mine,
    /// Pede a minha revisão. Balcão sem revisão responde `unsupported`.
    ToReview,
    #[default]
    Open,
    All,
}

/// Comentário de um pedido, já sem markup do balcão.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestComment {
    #[serde(default)]
    pub id: Option<String>,
    pub author: String,
    pub body: String,
    /// RFC 3339, em UTC.
    #[serde(default)]
    pub created_at: Option<String>,
}

/// Um pedido vindo de um balcão ou criado localmente.
///
/// `key` é o identificador que o próprio balcão usa (`#123`, `!45`, `ENG-42`, `PROJ-7`,
/// `LOCAL-1`). `counter` é o id do balcão (`github`, `gitlab`, `linear`, `jira`, `local`).
/// O par `(counter, key)` identifica o pedido.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestItem {
    pub counter: String,
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub kind: RequestKind,
    #[serde(default)]
    pub state: RequestState,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub assignees: Vec<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    /// Repositório, equipe ou projeto do balcão, como o balcão o chama.
    #[serde(default)]
    pub project: Option<String>,
    /// RFC 3339, em UTC.
    #[serde(default)]
    pub updated_at: Option<String>,
    /// Atribuído à conta conectada ou aberto por ela.
    #[serde(default)]
    pub mine: bool,
    /// A conta conectada foi pedida como revisora.
    #[serde(default)]
    pub review_requested: bool,
    /// Texto do pedido. Vem só em `get`; listas trazem `None`.
    #[serde(default)]
    pub body: Option<String>,
    /// Comentários. Vêm só em `get`.
    #[serde(default)]
    pub comments: Vec<RequestComment>,
    /// Estado dos checks do pedido, quando o balcão informa. Aditivo: balcões que não
    /// informam deixam `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<CheckState>,
}

/// Estado agregado dos checks de um pedido (CI ou pipeline).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    #[default]
    Pending,
    Passing,
    Failing,
}

impl RequestItem {
    /// Pedido mínimo: o resto é preenchido pelo balcão.
    pub fn new(counter: impl Into<String>, key: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            counter: counter.into(),
            key: key.into(),
            title: title.into(),
            kind: RequestKind::default(),
            state: RequestState::default(),
            draft: false,
            url: None,
            author: None,
            assignees: Vec::new(),
            labels: Vec::new(),
            project: None,
            updated_at: None,
            mine: false,
            review_requested: false,
            body: None,
            comments: Vec::new(),
            checks: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.state == RequestState::Open
    }

    /// Referência que o comando `bench.create` recebe em `from_request`.
    pub fn request_ref(&self) -> RequestRef {
        RequestRef {
            counter: self.counter.clone(),
            key: self.key.clone(),
            title: self.title.clone(),
            url: self.url.clone(),
        }
    }
}

/// Pedido resumido, usado como `from_request` em `bench.create`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestRef {
    pub counter: String,
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub url: Option<String>,
}

/// Motivo de falha de um balcão. A tela traduz cada variante por i18n; nenhuma mensagem
/// do balcão vai para a interface sem passar por aqui.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CounterError {
    /// Não há token salvo para este balcão.
    NotConnected,
    /// 401: o token foi recusado.
    Unauthorized,
    /// 403: o token não tem permissão para esta ação.
    Forbidden,
    /// 404: o item não existe ou a conta não o enxerga.
    NotFound,
    /// 429: limite de consultas do balcão.
    RateLimited { retry_after_s: Option<u64> },
    /// 400, 409 ou 422: o balcão recusou o pedido. `message` vem da primeira linha da resposta.
    Rejected { message: String },
    /// 5xx.
    Server { status: u16 },
    /// Sem resposta: DNS, conexão, TLS ou tempo esgotado.
    Network { message: String },
    /// Resposta que não é o formato esperado.
    Decode { message: String },
    /// Falta configuração, por exemplo host, equipe ou projeto. `message` diz o que falta.
    Config { message: String },
    /// O cofre de segredos do sistema não respondeu.
    Vault { message: String },
    /// O balcão não tem esse escopo ou essa ação.
    Unsupported,
}

impl fmt::Display for CounterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConnected => f.write_str("not connected"),
            Self::Unauthorized => f.write_str("unauthorized (401)"),
            Self::Forbidden => f.write_str("forbidden (403)"),
            Self::NotFound => f.write_str("not found (404)"),
            Self::RateLimited { .. } => f.write_str("rate limited (429)"),
            Self::Rejected { message } => write!(f, "rejected: {message}"),
            Self::Server { status } => write!(f, "server error ({status})"),
            Self::Network { message } => write!(f, "network: {message}"),
            Self::Decode { message } => write!(f, "decode: {message}"),
            Self::Config { message } => write!(f, "config: {message}"),
            Self::Vault { message } => write!(f, "vault: {message}"),
            Self::Unsupported => f.write_str("unsupported"),
        }
    }
}

/// Operação que produziu um [`CounterResult`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterOp {
    List,
    Get,
    Comment,
    Transition,
    Connect,
    Test,
    Disconnect,
}

/// Resultado de qualquer operação de balcão. Publicado como evento `counter.result`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterResult {
    /// Id do balcão: `github`, `gitlab`, `linear` ou `jira`.
    pub counter: String,
    pub op: CounterOp,
    /// Eco do `op_id` enviado por quem chamou.
    #[serde(default)]
    pub op_id: Option<String>,
    #[serde(default)]
    pub scope: Option<RequestScope>,
    /// Preenchido em `list`.
    #[serde(default)]
    pub items: Vec<RequestItem>,
    /// Preenchido em `get` e `transition`.
    #[serde(default)]
    pub item: Option<RequestItem>,
    /// Preenchido em `comment`.
    #[serde(default)]
    pub comment: Option<RequestComment>,
    /// Conta confirmada em `connect` e `test`, por exemplo o login.
    #[serde(default)]
    pub account: Option<String>,
    #[serde(default)]
    pub error: Option<CounterError>,
}

impl CounterResult {
    pub fn ok(counter: impl Into<String>, op: CounterOp) -> Self {
        Self {
            counter: counter.into(),
            op,
            op_id: None,
            scope: None,
            items: Vec::new(),
            item: None,
            comment: None,
            account: None,
            error: None,
        }
    }

    pub fn failed(counter: impl Into<String>, op: CounterOp, error: CounterError) -> Self {
        Self {
            error: Some(error),
            ..Self::ok(counter, op)
        }
    }
}

/// Argumentos de `counter_<nome>.list`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListArgs {
    /// Pasta do projeto. O balcão lê `origin` dela para saber o repositório.
    pub project_path: String,
    #[serde(default)]
    pub scope: RequestScope,
    #[serde(default)]
    pub op_id: Option<String>,
    /// Texto livre. O balcão o aplica quando a API aceita busca.
    #[serde(default)]
    pub query: Option<String>,
}

/// Argumentos de `counter_<nome>.get`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemArgs {
    pub project_path: String,
    pub key: String,
    #[serde(default)]
    pub op_id: Option<String>,
}

/// Argumentos de `counter_<nome>.comment`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentArgs {
    pub project_path: String,
    pub key: String,
    pub body: String,
    #[serde(default)]
    pub op_id: Option<String>,
}

impl fmt::Debug for CommentArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommentArgs")
            .field("project_path", &self.project_path)
            .field("key", &self.key)
            .field("body", &"<redacted>")
            .field("op_id", &self.op_id)
            .finish()
    }
}

/// Argumentos de `counter_<nome>.transition`. Só `open` e `closed`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionArgs {
    pub project_path: String,
    pub key: String,
    pub to: RequestState,
    #[serde(default)]
    pub op_id: Option<String>,
}

/// Argumentos de `counter_<nome>.connect`. O token nunca aparece no `Debug`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectArgs {
    pub token: String,
    /// Site do balcão, quando ele não vem de uma configuração (Jira).
    #[serde(default)]
    pub site: Option<String>,
    /// E-mail da conta, quando o balcão usa par e-mail + token (Jira).
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub op_id: Option<String>,
}

impl fmt::Debug for ConnectArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectArgs")
            .field("token", &"<redacted>")
            .field("site", &self.site)
            .field("email", &self.email)
            .field("op_id", &self.op_id)
            .finish()
    }
}

/// Argumentos de `counter_<nome>.test` e `counter_<nome>.disconnect`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpArgs {
    #[serde(default)]
    pub op_id: Option<String>,
}

/// Um balcão instalado, anunciado por `counter.sources`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterSource {
    /// Id curto: `github`.
    pub counter: String,
    /// Módulo dono dos comandos: `counter_github`.
    pub module: String,
    /// Chave i18n do nome exibido.
    pub label_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_and_states_serialize_snake_case() {
        assert_eq!(serde_json::to_string(&RequestScope::ToReview).unwrap(), "\"to_review\"");
        assert_eq!(serde_json::to_string(&RequestState::Merged).unwrap(), "\"merged\"");
    }

    #[test]
    fn counter_error_is_tagged_with_kind() {
        let json = serde_json::to_value(CounterError::RateLimited { retry_after_s: Some(30) }).unwrap();
        assert_eq!(json["kind"], "rate_limited");
        assert_eq!(json["retry_after_s"], 30);
    }

    #[test]
    fn result_round_trips_with_defaults() {
        let mut item = RequestItem::new("github", "#12", "Fix login");
        item.labels.push("bug".into());
        let mut result = CounterResult::ok("github", CounterOp::List);
        result.op_id = Some("op-1".into());
        result.items.push(item.clone());
        let json = serde_json::to_string(&result).unwrap();
        let back: CounterResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back, result);
        let minimal: RequestItem =
            serde_json::from_str(r#"{"counter":"jira","key":"ENG-1","title":"t"}"#).unwrap();
        assert_eq!(minimal.state, RequestState::Open);
        assert_eq!(minimal.kind, RequestKind::Issue);
    }

    #[test]
    fn secrets_never_show_in_debug() {
        let connect = ConnectArgs {
            token: "ghp_secret".into(),
            site: None,
            email: None,
            op_id: None,
        };
        assert!(!format!("{connect:?}").contains("ghp_secret"));
        let comment = CommentArgs {
            project_path: "C:/p".into(),
            key: "#1".into(),
            body: "texto privado".into(),
            op_id: None,
        };
        assert!(!format!("{comment:?}").contains("texto privado"));
    }

    #[test]
    fn request_ref_copies_the_identity_fields() {
        let mut item = RequestItem::new("linear", "ENG-42", "Cache");
        item.url = Some("https://linear.app/x/issue/ENG-42".into());
        let reference = item.request_ref();
        assert_eq!(reference.key, "ENG-42");
        assert_eq!(reference.url, item.url);
    }
}
