//! Encanamento comum aos balcões da Forja: transporte HTTP, mapeamento de status,
//! paginação, leitura do remoto `origin` e cofre de segredos.
//!
//! Esta é uma biblioteca de apoio, não um módulo: os `counter_*` dependem dela, e nunca
//! uns dos outros. Um balcão novo usa estas peças e só implementa a API do próprio serviço.
//!
//! Regras que valem para todo balcão:
//! - o token vai só para o host configurado do balcão (`check_host`);
//! - segredo não aparece em `Debug`, log nem mensagem de erro;
//! - rede e processo nunca rodam na thread da interface: quem chama já está em `Ctx::spawn`.

pub mod base64;
pub mod client;
pub mod fake;
pub mod http;
pub mod remote;
pub mod secrets;
pub mod url;
pub mod vault;

pub use client::UreqTransport;
pub use fake::FakeTransport;
pub use http::{
    error_for_status, next_link, next_page_number, retry_after_s, HttpRequest, HttpResponse,
    Method, Transport, UnavailableTransport,
};
pub use remote::{check_host, is_plain_segment, parse_remote, read_origin, RemoteRepo};
pub use secrets::{service_for, MemorySecretStore, SecretStore, UnavailableStore};
pub use vault::SystemStore;

/// Ligado por `FORJA_FIXTURES=1` (captura de tela e demonstração). Com ele, os balcões
/// leem as fixtures embutidas em vez de chamar a rede.
pub fn fixtures_enabled() -> bool {
    std::env::var("FORJA_FIXTURES")
        .map(|value| matches!(value.trim(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

/// Monta `CounterResult::ok` com o item de uma lista, para as fixtures.
pub fn list_result(
    counter: &str,
    op_id: Option<String>,
    scope: forja_contracts::RequestScope,
    items: Vec<forja_contracts::RequestItem>,
) -> forja_contracts::CounterResult {
    let mut result = forja_contracts::CounterResult::ok(counter, forja_contracts::CounterOp::List);
    result.op_id = op_id;
    result.scope = Some(scope);
    result.items = items;
    result
}

/// Transporte padrão dos balcões: HTTP real pelo `ureq`. Trocar aqui troca para todos.
pub fn default_transport() -> std::sync::Arc<dyn Transport> {
    std::sync::Arc::new(UreqTransport::new())
}

/// Se `base` for o transporte real (`is_real()`), aplica a política JSON retornada por `network.proxy`.
/// Transportes falsos em teste (`FakeTransport`) são preservados como estão.
pub fn transport_for_policy(
    base: &std::sync::Arc<dyn Transport>,
    policy_json: Option<&serde_json::Value>,
) -> std::sync::Arc<dyn Transport> {
    if !base.is_real() {
        return std::sync::Arc::clone(base);
    }
    let Some(policy) = policy_json.and_then(|value| {
        serde_json::from_value::<forja_contracts::NetworkPolicy>(value.clone()).ok()
    }) else {
        return std::sync::Arc::clone(base);
    };
    std::sync::Arc::new(UreqTransport::from_policy(&policy))
}

/// Cofre padrão dos balcões: Gerenciador de Credenciais do Windows. Nunca grava em arquivo.
pub fn default_secrets() -> std::sync::Arc<dyn SecretStore> {
    std::sync::Arc::new(SystemStore)
}

/// Sufixo da chave i18n que explica um erro de balcão. Cada módulo que mostra erros tem as
/// próprias traduções, com o prefixo do módulo (`counter_github-error-unauthorized`), porque
/// as chaves são globais e um módulo tem de funcionar sem os outros.
pub fn error_suffix(error: &forja_contracts::CounterError) -> &'static str {
    use forja_contracts::CounterError as E;
    match error {
        E::NotConnected => "not-connected",
        E::Unauthorized => "unauthorized",
        E::Forbidden => "forbidden",
        E::NotFound => "not-found",
        E::RateLimited { .. } => "rate-limited",
        E::Rejected { .. } => "rejected",
        E::Server { .. } => "server",
        E::Network { .. } => "network",
        E::Decode { .. } => "decode",
        E::Config { .. } => "config",
        E::Vault { .. } => "vault",
        E::Unsupported => "unsupported",
    }
}

/// Todos os sufixos de `error_suffix`, para cada módulo declarar as próprias chaves.
pub const ERROR_SUFFIXES: [&str; 12] = [
    "not-connected",
    "unauthorized",
    "forbidden",
    "not-found",
    "rate-limited",
    "rejected",
    "server",
    "network",
    "decode",
    "config",
    "vault",
    "unsupported",
];

/// Chave completa do erro para um módulo: `counter_github-error-unauthorized`.
pub fn error_key(module: &str, error: &forja_contracts::CounterError) -> String {
    format!("{module}-error-{}", error_suffix(error))
}
