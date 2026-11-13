//! Operações do Jira. Rodam em `Ctx::spawn`, fora da thread da interface.
//! Autenticação básica com e-mail e token; o site é lido do cofre, não das configurações.

use crate::api::{self, MAX_PAGES};
use forja_contracts::{
    CommentArgs, ConnectArgs, CounterError, CounterOp, CounterResult, ItemArgs, ListArgs,
    RequestItem, RequestScope, TransitionArgs,
};
use forja_counter_kit::{
    error_for_status, fixtures_enabled, list_result, service_for, HttpRequest, HttpResponse,
    SecretStore, Transport,
};
use serde_json::Value;
use std::sync::Arc;

pub const COUNTER: &str = "jira";
pub const MODULE: &str = "counter_jira";
/// Conta do cofre com o site conectado. Os demais itens ficam sob o próprio site.
const SITE_ACCOUNT: &str = "site";
const FIXTURE_SITE: &str = "acme.atlassian.net";
const FIXTURE_LOGIN: &str = "Ana Lima";
const FIXTURE_SEARCH: &str = include_str!("../tests/fixtures/search.json");

#[derive(Clone)]
pub struct Env {
    pub transport: Arc<dyn Transport>,
    pub secrets: Arc<dyn SecretStore>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Chave do projeto (`ENG`). Vazio mostra todos os projetos visíveis.
    pub project: String,
}

/// Credenciais lidas do cofre, sem `Debug` que mostre o token.
#[derive(Clone)]
pub struct Creds {
    pub site: String,
    pub email: String,
    pub token: String,
    pub login: Option<String>,
}

impl std::fmt::Debug for Creds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Creds")
            .field("site", &self.site)
            .field("email", &self.email)
            .field("token", &"<redacted>")
            .finish()
    }
}

pub fn list(env: &Env, config: &Config, args: ListArgs) -> CounterResult {
    let scope = args.scope;
    let outcome = if fixtures_enabled() {
        fixture_items(scope)
    } else {
        list_items(env, config, scope)
    };
    match outcome {
        Ok(items) => list_result(COUNTER, args.op_id, scope, items),
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::List, error), args.op_id),
    }
}

fn list_items(env: &Env, config: &Config, scope: RequestScope) -> Result<Vec<RequestItem>, CounterError> {
    let creds = creds(env)?;
    let jql = api::jql(scope, &config.project)?;
    let api_base = api_base(&creds.site);
    let mut items = Vec::new();
    let mut next: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let request = authorized(api::search_request(&api_base, &jql, next.as_deref()), &creds);
        let response = execute(env, request)?;
        let (page, token) = api::parse_search(response.json()?, &creds.site, creds.login.as_deref())?;
        items.extend(page);
        match token {
            Some(token) => next = Some(token),
            None => break,
        }
    }
    Ok(items)
}

pub fn get(env: &Env, _config: &Config, args: ItemArgs) -> CounterResult {
    match get_item(env, &args) {
        Ok(item) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Get);
            result.item = Some(item);
            with_op_id(result, args.op_id)
        }
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Get, error), args.op_id),
    }
}

fn get_item(env: &Env, args: &ItemArgs) -> Result<RequestItem, CounterError> {
    api::check_issue_key(&args.key)?;
    let creds = creds(env)?;
    let request = authorized(api::issue_request(&api_base(&creds.site), &args.key), &creds);
    let response = execute(env, request)?;
    Ok(api::parse_issue(&response.json()?, &creds.site, creds.login.as_deref()))
}

pub fn comment(env: &Env, _config: &Config, args: CommentArgs) -> CounterResult {
    match comment_on(env, &args) {
        Ok(comment) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Comment);
            result.comment = Some(comment);
            with_op_id(result, args.op_id)
        }
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Comment, error), args.op_id),
    }
}

fn comment_on(env: &Env, args: &CommentArgs) -> Result<forja_contracts::RequestComment, CounterError> {
    let body = args.body.trim();
    if body.is_empty() {
        return Err(CounterError::Rejected {
            message: "the comment is empty".to_owned(),
        });
    }
    api::check_issue_key(&args.key)?;
    let creds = creds(env)?;
    let request = authorized(api::comment_request(&api_base(&creds.site), &args.key, body), &creds);
    let response = execute(env, request)?;
    Ok(api::parse_comment(&response.json()?))
}

pub fn transition(env: &Env, _config: &Config, args: TransitionArgs) -> CounterResult {
    match transition_item(env, &args) {
        Ok(item) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Transition);
            result.item = Some(item);
            with_op_id(result, args.op_id)
        }
        Err(error) => with_op_id(
            CounterResult::failed(COUNTER, CounterOp::Transition, error),
            args.op_id,
        ),
    }
}

fn transition_item(env: &Env, args: &TransitionArgs) -> Result<RequestItem, CounterError> {
    api::check_issue_key(&args.key)?;
    let creds = creds(env)?;
    let api_base = api_base(&creds.site);
    let transitions = execute(
        env,
        authorized(api::transitions_request(&api_base, &args.key), &creds),
    )?
    .json()?;
    let transition_id = api::pick_transition(&transitions, args.to)?;
    execute(
        env,
        authorized(
            api::apply_transition_request(&api_base, &args.key, &transition_id),
            &creds,
        ),
    )?;
    let response = execute(env, authorized(api::issue_request(&api_base, &args.key), &creds))?;
    Ok(api::parse_issue(&response.json()?, &creds.site, creds.login.as_deref()))
}

pub fn test(env: &Env, _config: &Config, op_id: Option<String>) -> CounterResult {
    match creds(env).and_then(|creds| verify(env, &creds.site, &creds.email, &creds.token)) {
        Ok(account) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Test);
            result.account = Some(account);
            with_op_id(result, op_id)
        }
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Test, error), op_id),
    }
}

/// Confere site, e-mail e token com `/myself`; só então grava tudo no cofre.
pub fn connect(env: &Env, _config: &Config, args: ConnectArgs) -> CounterResult {
    let outcome: Result<String, CounterError> = (|| {
        let site = api::normalize_site(args.site.as_deref().unwrap_or_default())?;
        let email = args.email.as_deref().unwrap_or_default().trim().to_owned();
        if email.is_empty() {
            return Err(CounterError::Config {
                message: "the Jira account e-mail is missing".to_owned(),
            });
        }
        let token = args.token.trim().to_owned();
        let account = verify(env, &site, &email, &token)?;
        let service = service_for(MODULE);
        env.secrets.set(&service, SITE_ACCOUNT, &site)?;
        env.secrets.set(&service, &site, &token)?;
        env.secrets.set(&service, &email_account(&site), &email)?;
        env.secrets.set(&service, &login_account(&site), &account)?;
        Ok(account)
    })();
    match outcome {
        Ok(account) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Connect);
            result.account = Some(account);
            with_op_id(result, args.op_id)
        }
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Connect, error), args.op_id),
    }
}

pub fn disconnect(env: &Env, _config: &Config, op_id: Option<String>) -> CounterResult {
    let service = service_for(MODULE);
    let outcome: Result<(), CounterError> = (|| {
        let Some(site) = env.secrets.get(&service, SITE_ACCOUNT)? else {
            return Ok(());
        };
        env.secrets.delete(&service, &site)?;
        env.secrets.delete(&service, &email_account(&site))?;
        env.secrets.delete(&service, &login_account(&site))?;
        env.secrets.delete(&service, SITE_ACCOUNT)
    })();
    match outcome {
        Ok(()) => with_op_id(CounterResult::ok(COUNTER, CounterOp::Disconnect), op_id),
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Disconnect, error), op_id),
    }
}

fn verify(env: &Env, site: &str, email: &str, token: &str) -> Result<String, CounterError> {
    let creds = Creds {
        site: site.to_owned(),
        email: email.to_owned(),
        token: token.to_owned(),
        login: None,
    };
    let response = execute(env, authorized(api::myself_request(&api_base(site)), &creds))?;
    api::parse_display_name(response.json()?)
}

fn creds(env: &Env) -> Result<Creds, CounterError> {
    let service = service_for(MODULE);
    let site = env
        .secrets
        .get(&service, SITE_ACCOUNT)?
        .ok_or(CounterError::NotConnected)?;
    let token = env
        .secrets
        .get(&service, &site)?
        .filter(|token| !token.trim().is_empty())
        .ok_or(CounterError::NotConnected)?;
    let email = env
        .secrets
        .get(&service, &email_account(&site))?
        .ok_or(CounterError::NotConnected)?;
    let login = env.secrets.get(&service, &login_account(&site)).ok().flatten();
    Ok(Creds {
        site,
        email,
        token,
        login,
    })
}

fn api_base(site: &str) -> String {
    format!("https://{site}/rest/api/3")
}

fn email_account(site: &str) -> String {
    format!("{site}#email")
}

fn login_account(site: &str) -> String {
    format!("{site}#login")
}

/// Autenticação básica (`e-mail:token`) e JSON, como a API v3 pede.
fn authorized(request: HttpRequest, creds: &Creds) -> HttpRequest {
    request
        .basic(&creds.email, &creds.token)
        .header("Accept", "application/json")
}

fn execute(env: &Env, request: HttpRequest) -> Result<HttpResponse, CounterError> {
    let response = env.transport.send(&request)?;
    match error_for_status(response.status, &response.headers, &response.body) {
        Some(CounterError::Rejected { message }) => Err(CounterError::Rejected {
            message: api::error_message(&response.body).unwrap_or(message),
        }),
        Some(error) => Err(error),
        None => Ok(response),
    }
}

fn with_op_id(mut result: CounterResult, op_id: Option<String>) -> CounterResult {
    result.op_id = op_id;
    result
}

/// Lista de captura de tela: fixtures, sem rede.
pub(crate) fn fixture_items(scope: RequestScope) -> Result<Vec<RequestItem>, CounterError> {
    if scope == RequestScope::ToReview {
        return Err(CounterError::Unsupported);
    }
    let body: Value = serde_json::from_str(FIXTURE_SEARCH).map_err(|error| CounterError::Decode {
        message: error.to_string(),
    })?;
    let (items, _) = api::parse_search(body, FIXTURE_SITE, Some(FIXTURE_LOGIN))?;
    Ok(items
        .into_iter()
        .filter(|item| match scope {
            RequestScope::Mine => item.mine,
            RequestScope::Open => item.is_open(),
            _ => true,
        })
        .collect())
}
