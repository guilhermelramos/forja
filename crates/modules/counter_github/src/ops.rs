//! Operações do GitHub. Tudo aqui roda em `Ctx::spawn`, fora da thread da interface:
//! leitura do cofre, `git remote`, HTTP e paginação. O resultado volta como `CounterResult`.

use crate::api::{self, ListQuery, MAX_PAGES};
use forja_contracts::{
    CommentArgs, ConnectArgs, CounterError, CounterOp, CounterResult, ItemArgs, ListArgs,
    RequestItem, RequestScope, RequestState, TransitionArgs,
};
use forja_counter_kit::{
    check_host, error_for_status, fixtures_enabled, list_result, next_link, parse_remote,
    read_origin, service_for, HttpRequest, HttpResponse, SecretStore, Transport,
};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

pub const COUNTER: &str = "github";
pub const MODULE: &str = "counter_github";
/// Conta usada no cofre para o login confirmado, ao lado do token do host.
const LOGIN_SUFFIX: &str = "#login";
/// Login usado pelas fixtures de captura de tela.
const FIXTURE_LOGIN: &str = "ana-dev";
const FIXTURE_REPO: &str = "acme/tools";
const FIXTURE_ISSUES: &str = include_str!("../tests/fixtures/issues.json");
const FIXTURE_REVIEW: &str = include_str!("../tests/fixtures/review_search.json");

/// Recursos que um balcão usa. Clonados para a thread de trabalho.
#[derive(Clone)]
pub struct Env {
    pub transport: Arc<dyn Transport>,
    pub secrets: Arc<dyn SecretStore>,
}

/// Configurações lidas da thread da interface antes de sair para o trabalho.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub host: String,
    pub include_drafts: bool,
    pub labels: Vec<String>,
}

pub fn list(env: &Env, config: &Config, args: ListArgs) -> CounterResult {
    let scope = args.scope;
    let outcome = if fixtures_enabled() {
        fixture_items(config, scope)
    } else {
        list_items(env, config, &args)
    };
    match outcome {
        Ok(items) => list_result(COUNTER, args.op_id, scope, items),
        Err(error) => with_op_id(
            CounterResult::failed(COUNTER, CounterOp::List, error),
            args.op_id,
        ),
    }
}

fn list_items(
    env: &Env,
    config: &Config,
    args: &ListArgs,
) -> Result<Vec<RequestItem>, CounterError> {
    let repo = resolve_repo(&args.project_path, &config.host)?;
    let token = token(env, &config.host)?;
    let login = login(env, &config.host);
    let query = ListQuery {
        repo: repo.clone(),
        scope: args.scope,
        login: login.clone(),
        labels: config.labels.clone(),
    };
    let api_base = api::api_base(&config.host);
    let first = api::list_request(&api_base, &query)?;
    let search = args.scope == RequestScope::ToReview;
    let mut items = Vec::new();
    let mut next = Some(first);
    for _ in 0..MAX_PAGES {
        let Some(request) = next.take() else { break };
        let response = execute(env, api::authorized(request, &token))?;
        let page = api::parse_issues(
            response.json()?,
            search,
            &repo,
            login.as_deref(),
            args.scope,
        )?;
        items.extend(page);
        next = next_link(&response.headers, &api_base).map(HttpRequest::get);
    }
    Ok(filter(items, config))
}

/// Tira rascunhos (se a configuração pede) e pedidos sem todos os rótulos exigidos.
fn filter(items: Vec<RequestItem>, config: &Config) -> Vec<RequestItem> {
    items
        .into_iter()
        .filter(|item| config.include_drafts || !item.draft)
        .filter(|item| {
            config
                .labels
                .iter()
                .all(|label| item.labels.contains(label))
        })
        .collect()
}

pub fn get(env: &Env, config: &Config, args: ItemArgs) -> CounterResult {
    let outcome = if fixtures_enabled() {
        fixture_items(config, RequestScope::All).and_then(|items| {
            items
                .into_iter()
                .find(|item| item.key == args.key)
                .ok_or(CounterError::NotFound)
        })
    } else {
        get_item(env, config, &args)
    };
    match outcome {
        Ok(item) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Get);
            result.item = Some(item);
            with_op_id(result, args.op_id)
        }
        Err(error) => with_op_id(
            CounterResult::failed(COUNTER, CounterOp::Get, error),
            args.op_id,
        ),
    }
}

fn get_item(env: &Env, config: &Config, args: &ItemArgs) -> Result<RequestItem, CounterError> {
    let repo = resolve_repo(&args.project_path, &config.host)?;
    let number = api::number_from_key(&args.key)?;
    let token = token(env, &config.host)?;
    let api_base = api::api_base(&config.host);
    let login = login(env, &config.host);
    let issue = execute(
        env,
        api::authorized(api::get_request(&api_base, &repo, number), &token),
    )?;
    let mut item = api::parse_item(issue.json()?, &repo, login.as_deref())?;
    let comments = execute(
        env,
        api::authorized(api::comments_request(&api_base, &repo, number), &token),
    )?;
    item.comments = api::parse_comments(comments.json()?)?;
    Ok(item)
}

pub fn comment(env: &Env, config: &Config, args: CommentArgs) -> CounterResult {
    let outcome = comment_on(env, config, &args);
    match outcome {
        Ok(comment) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Comment);
            result.comment = Some(comment);
            with_op_id(result, args.op_id)
        }
        Err(error) => with_op_id(
            CounterResult::failed(COUNTER, CounterOp::Comment, error),
            args.op_id,
        ),
    }
}

fn comment_on(
    env: &Env,
    config: &Config,
    args: &CommentArgs,
) -> Result<forja_contracts::RequestComment, CounterError> {
    let body = args.body.trim();
    if body.is_empty() {
        return Err(CounterError::Rejected {
            message: "the comment is empty".to_owned(),
        });
    }
    let repo = resolve_repo(&args.project_path, &config.host)?;
    let number = api::number_from_key(&args.key)?;
    let token = token(env, &config.host)?;
    let request = api::comment_request(&api::api_base(&config.host), &repo, number, body);
    let response = execute(env, api::authorized(request, &token))?;
    api::parse_comment(response.json()?)
}

pub fn transition(env: &Env, config: &Config, args: TransitionArgs) -> CounterResult {
    let outcome = transition_item(env, config, &args);
    match outcome {
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

fn transition_item(
    env: &Env,
    config: &Config,
    args: &TransitionArgs,
) -> Result<RequestItem, CounterError> {
    let open = match args.to {
        RequestState::Open => true,
        RequestState::Closed => false,
        RequestState::Merged => return Err(CounterError::Unsupported),
    };
    let repo = resolve_repo(&args.project_path, &config.host)?;
    let number = api::number_from_key(&args.key)?;
    let token = token(env, &config.host)?;
    let api_base = api::api_base(&config.host);
    let request = api::state_request(&api_base, &repo, number, open);
    let response = execute(env, api::authorized(request, &token))?;
    let login = login(env, &config.host);
    api::parse_item(response.json()?, &repo, login.as_deref())
}

/// Confere a conexão com `GET /user` e grava o login confirmado.
pub fn test(env: &Env, config: &Config, op_id: Option<String>) -> CounterResult {
    let outcome = token(env, &config.host).and_then(|token| verify(env, &config.host, &token));
    match outcome {
        Ok(login) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Test);
            result.account = Some(login);
            with_op_id(result, op_id)
        }
        Err(error) => with_op_id(
            CounterResult::failed(COUNTER, CounterOp::Test, error),
            op_id,
        ),
    }
}

/// Valida o token digitado antes de guardá-lo. Só grava se o GitHub aceitar.
pub fn connect(env: &Env, config: &Config, args: ConnectArgs) -> CounterResult {
    let outcome = verify(env, &config.host, args.token.trim()).and_then(|login| {
        env.secrets
            .set(&service_for(MODULE), &config.host, args.token.trim())?;
        env.secrets
            .set(&service_for(MODULE), &login_account(&config.host), &login)?;
        Ok(login)
    });
    match outcome {
        Ok(login) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Connect);
            result.account = Some(login);
            with_op_id(result, args.op_id)
        }
        Err(error) => with_op_id(
            CounterResult::failed(COUNTER, CounterOp::Connect, error),
            args.op_id,
        ),
    }
}

pub fn disconnect(env: &Env, config: &Config, op_id: Option<String>) -> CounterResult {
    let outcome = env
        .secrets
        .delete(&service_for(MODULE), &config.host)
        .and_then(|()| {
            env.secrets
                .delete(&service_for(MODULE), &login_account(&config.host))
        });
    match outcome {
        Ok(()) => with_op_id(CounterResult::ok(COUNTER, CounterOp::Disconnect), op_id),
        Err(error) => with_op_id(
            CounterResult::failed(COUNTER, CounterOp::Disconnect, error),
            op_id,
        ),
    }
}

fn verify(env: &Env, host: &str, token: &str) -> Result<String, CounterError> {
    let request = api::authorized(api::user_request(&api::api_base(host)), token);
    let response = execute(env, request)?;
    api::parse_login(response.json()?)
}

/// Repositório `dono/nome` do remoto `origin`, conferido contra o host configurado.
fn resolve_repo(project_path: &str, host: &str) -> Result<String, CounterError> {
    let url = read_origin(Path::new(project_path))?;
    let remote = parse_remote(&url).ok_or_else(|| CounterError::Config {
        message: "origin is not a repository URL".to_owned(),
    })?;
    check_host(&remote.host, host)?;
    if remote.path.split('/').count() != 2 {
        return Err(CounterError::Config {
            message: "origin must be owner/name on GitHub".to_owned(),
        });
    }
    Ok(remote.path)
}

fn token(env: &Env, host: &str) -> Result<String, CounterError> {
    env.secrets
        .get(&service_for(MODULE), host)?
        .filter(|token| !token.trim().is_empty())
        .ok_or(CounterError::NotConnected)
}

fn login(env: &Env, host: &str) -> Option<String> {
    env.secrets
        .get(&service_for(MODULE), &login_account(host))
        .ok()
        .flatten()
}

fn login_account(host: &str) -> String {
    format!("{host}{LOGIN_SUFFIX}")
}

/// Envia e converte o status em erro. Não lê o corpo quando já há erro.
fn execute(env: &Env, request: HttpRequest) -> Result<HttpResponse, CounterError> {
    let response = env.transport.send(&request)?;
    match error_for_status(response.status, &response.headers, &response.body) {
        Some(error) => Err(error),
        None => Ok(response),
    }
}

fn with_op_id(mut result: CounterResult, op_id: Option<String>) -> CounterResult {
    result.op_id = op_id;
    result
}

/// Lista de captura de tela: as fixtures, filtradas pelo escopo. Sem rede.
pub(crate) fn fixture_items(
    config: &Config,
    scope: RequestScope,
) -> Result<Vec<RequestItem>, CounterError> {
    let (text, search) = if scope == RequestScope::ToReview {
        (FIXTURE_REVIEW, true)
    } else {
        (FIXTURE_ISSUES, false)
    };
    let body: Value = serde_json::from_str(text).map_err(|error| CounterError::Decode {
        message: error.to_string(),
    })?;
    let items = api::parse_issues(body, search, FIXTURE_REPO, Some(FIXTURE_LOGIN), scope)?;
    Ok(filter(items, config)
        .into_iter()
        .filter(|item| match scope {
            RequestScope::Mine => item.mine,
            RequestScope::ToReview => item.review_requested,
            RequestScope::Open => item.is_open(),
            RequestScope::All => true,
        })
        .collect())
}
