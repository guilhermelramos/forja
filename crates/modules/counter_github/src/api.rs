//! API REST do GitHub: montagem das requisições e leitura das respostas. Não faz rede.
//!
//! Issues e pull requests usam o mesmo caminho (`/issues`); o campo `pull_request` diz
//! qual é qual. "Para revisar" usa a busca, porque só ela filtra por revisor pedido.

use forja_contracts::{CounterError, RequestComment, RequestItem, RequestKind, RequestScope, RequestState};
use forja_counter_kit::url::encode_component;
use forja_counter_kit::HttpRequest;
use serde::Deserialize;
use serde_json::{json, Value};

pub const ACCEPT: &str = "application/vnd.github+json";
pub const API_VERSION: &str = "2022-11-28";
pub const USER_AGENT: &str = "Forja";
pub const PAGE_SIZE: u32 = 50;
/// Teto de páginas por lista: 250 itens. Acima disso a tela mostra o que chegou.
pub const MAX_PAGES: usize = 5;

/// `https://api.github.com` para github.com; `https://<host>/api/v3` para servidores próprios.
pub fn api_base(host: &str) -> String {
    if host.eq_ignore_ascii_case("github.com") {
        "https://api.github.com".to_owned()
    } else {
        format!("https://{host}/api/v3")
    }
}

/// Acrescenta token e os cabeçalhos da API a uma requisição.
pub fn authorized(request: HttpRequest, token: &str) -> HttpRequest {
    request
        .bearer(token)
        .header("Accept", ACCEPT)
        .header("X-GitHub-Api-Version", API_VERSION)
        .header("User-Agent", USER_AGENT)
}

/// Número de um pedido a partir da chave `#123`.
pub fn number_from_key(key: &str) -> Result<u64, CounterError> {
    key.strip_prefix('#')
        .and_then(|digits| digits.parse::<u64>().ok())
        .ok_or_else(|| CounterError::Rejected {
            message: "the GitHub request key must look like #123".to_owned(),
        })
}

/// Consulta de uma lista, já resolvida pelo módulo (escopo, login e rótulos).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListQuery {
    pub repo: String,
    pub scope: RequestScope,
    /// Login da conta conectada. Obrigatório para `Mine` e `ToReview`.
    pub login: Option<String>,
    pub labels: Vec<String>,
}

pub fn list_request(api: &str, query: &ListQuery) -> Result<HttpRequest, CounterError> {
    let labels = if query.labels.is_empty() {
        String::new()
    } else {
        format!("&labels={}", encode_component(&query.labels.join(",")))
    };
    let repo = &query.repo;
    let url = match query.scope {
        RequestScope::Open => format!(
            "{api}/repos/{repo}/issues?state=open&per_page={PAGE_SIZE}&sort=updated&direction=desc{labels}"
        ),
        RequestScope::All => format!(
            "{api}/repos/{repo}/issues?state=all&per_page={PAGE_SIZE}&sort=updated&direction=desc{labels}"
        ),
        RequestScope::Mine => {
            let login = required_login(query)?;
            format!(
                "{api}/repos/{repo}/issues?state=open&assignee={}&per_page={PAGE_SIZE}&sort=updated&direction=desc{labels}",
                encode_component(login)
            )
        }
        RequestScope::ToReview => {
            let login = required_login(query)?;
            let q = format!("repo:{repo} is:pr is:open review-requested:{login}");
            format!(
                "{api}/search/issues?q={}&per_page={PAGE_SIZE}&sort=updated&order=desc",
                encode_component(&q)
            )
        }
    };
    Ok(HttpRequest::get(url))
}

fn required_login(query: &ListQuery) -> Result<&str, CounterError> {
    query.login.as_deref().ok_or_else(|| CounterError::Config {
        message: "the GitHub account login is unknown; test the connection again".to_owned(),
    })
}

pub fn get_request(api: &str, repo: &str, number: u64) -> HttpRequest {
    HttpRequest::get(format!("{api}/repos/{repo}/issues/{number}"))
}

pub fn comments_request(api: &str, repo: &str, number: u64) -> HttpRequest {
    HttpRequest::get(format!(
        "{api}/repos/{repo}/issues/{number}/comments?per_page=100"
    ))
}

pub fn comment_request(api: &str, repo: &str, number: u64, body: &str) -> HttpRequest {
    HttpRequest::post(
        format!("{api}/repos/{repo}/issues/{number}/comments"),
        json!({ "body": body }),
    )
}

pub fn state_request(api: &str, repo: &str, number: u64, open: bool) -> HttpRequest {
    let state = if open { "open" } else { "closed" };
    HttpRequest::patch(
        format!("{api}/repos/{repo}/issues/{number}"),
        json!({ "state": state }),
    )
}

pub fn user_request(api: &str) -> HttpRequest {
    HttpRequest::get(format!("{api}/user"))
}

#[derive(Debug, Deserialize)]
struct GhUser {
    login: String,
}

#[derive(Debug, Deserialize)]
struct GhPullRef {
    #[serde(default)]
    merged_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhIssue {
    number: u64,
    title: String,
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    user: Option<GhUser>,
    #[serde(default)]
    assignees: Vec<GhUser>,
    #[serde(default)]
    labels: Vec<Value>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    pull_request: Option<GhPullRef>,
}

#[derive(Debug, Deserialize)]
struct GhSearch {
    items: Vec<GhIssue>,
}

#[derive(Debug, Deserialize)]
struct GhComment {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    user: Option<GhUser>,
    body: String,
    #[serde(default)]
    created_at: Option<String>,
}

/// Converte a lista de issues, ou o resultado da busca (`{items: [...]}`), em pedidos comuns.
pub fn parse_issues(
    body: Value,
    search: bool,
    repo: &str,
    login: Option<&str>,
    scope: RequestScope,
) -> Result<Vec<RequestItem>, CounterError> {
    let issues: Vec<GhIssue> = if search {
        serde_json::from_value::<GhSearch>(body)
            .map(|search| search.items)
            .map_err(decode)?
    } else {
        serde_json::from_value(body).map_err(decode)?
    };
    Ok(issues
        .into_iter()
        .map(|issue| item_from(issue, repo, login, scope == RequestScope::ToReview))
        .collect())
}

/// Um pedido só, como `GET /issues/{n}` devolve.
pub fn parse_item(body: Value, repo: &str, login: Option<&str>) -> Result<RequestItem, CounterError> {
    let issue: GhIssue = serde_json::from_value(body).map_err(decode)?;
    Ok(item_from(issue, repo, login, false))
}

pub fn parse_comments(body: Value) -> Result<Vec<RequestComment>, CounterError> {
    let comments: Vec<GhComment> = serde_json::from_value(body).map_err(decode)?;
    Ok(comments.into_iter().map(comment_from).collect())
}

pub fn parse_comment(body: Value) -> Result<RequestComment, CounterError> {
    let comment: GhComment = serde_json::from_value(body).map_err(decode)?;
    Ok(comment_from(comment))
}

pub fn parse_login(body: Value) -> Result<String, CounterError> {
    let user: GhUser = serde_json::from_value(body).map_err(decode)?;
    Ok(user.login)
}

fn decode(error: serde_json::Error) -> CounterError {
    CounterError::Decode {
        message: error.to_string(),
    }
}

fn comment_from(comment: GhComment) -> RequestComment {
    RequestComment {
        id: comment.id.map(|id| id.to_string()),
        author: comment.user.map(|user| user.login).unwrap_or_default(),
        body: comment.body,
        created_at: comment.created_at,
    }
}

fn item_from(issue: GhIssue, repo: &str, login: Option<&str>, review_requested: bool) -> RequestItem {
    let kind = if issue.pull_request.is_some() {
        RequestKind::Change
    } else {
        RequestKind::Issue
    };
    let merged = issue
        .pull_request
        .as_ref()
        .is_some_and(|pull| pull.merged_at.is_some());
    let state = match issue.state.as_str() {
        "closed" if merged => RequestState::Merged,
        "closed" => RequestState::Closed,
        _ => RequestState::Open,
    };
    let author = issue.user.map(|user| user.login);
    let assignees: Vec<String> = issue.assignees.into_iter().map(|user| user.login).collect();
    let mine = login.is_some_and(|login| {
        author.as_deref() == Some(login) || assignees.iter().any(|name| name == login)
    });
    let labels = issue.labels.iter().filter_map(label_name).collect();
    RequestItem {
        counter: "github".to_owned(),
        key: format!("#{}", issue.number),
        title: issue.title,
        kind,
        state,
        draft: issue.draft,
        url: issue.html_url,
        author,
        assignees,
        labels,
        project: Some(repo.to_owned()),
        updated_at: issue.updated_at,
        mine,
        review_requested,
        body: issue.body,
        comments: Vec::new(),
        checks: None,
    }
}

/// A API manda rótulos como objetos `{name}`; versões antigas mandavam texto puro.
fn label_name(label: &Value) -> Option<String> {
    match label {
        Value::String(name) => Some(name.clone()),
        Value::Object(map) => map.get("name").and_then(Value::as_str).map(str::to_owned),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enterprise_hosts_use_the_api_v3_prefix() {
        assert_eq!(api_base("github.com"), "https://api.github.com");
        assert_eq!(api_base("ghe.example.com"), "https://ghe.example.com/api/v3");
    }

    #[test]
    fn keys_must_be_hash_numbers() {
        assert_eq!(number_from_key("#42"), Ok(42));
        assert!(number_from_key("42").is_err());
        assert!(number_from_key("#4x").is_err());
    }

    #[test]
    fn labels_are_joined_and_encoded() {
        let query = ListQuery {
            repo: "acme/tools".into(),
            scope: RequestScope::Open,
            login: None,
            labels: vec!["good first".into(), "bug".into()],
        };
        let request = list_request("https://api.github.com", &query).unwrap();
        assert!(request.url.contains("state=open"));
        assert!(request.url.contains("labels=good%20first%2Cbug"));
    }

    #[test]
    fn mine_and_review_need_the_login() {
        let query = ListQuery {
            repo: "acme/tools".into(),
            scope: RequestScope::Mine,
            login: None,
            labels: Vec::new(),
        };
        assert!(matches!(
            list_request("https://api.github.com", &query),
            Err(CounterError::Config { .. })
        ));
        let review = ListQuery {
            scope: RequestScope::ToReview,
            login: Some("ana-dev".into()),
            ..query
        };
        let request = list_request("https://api.github.com", &review).unwrap();
        assert!(request.url.contains("/search/issues?q="));
        assert!(request.url.contains("review-requested%3Aana-dev"));
    }

    #[test]
    fn merged_pull_requests_are_not_just_closed() {
        let body = json!([
            {"number": 1, "title": "a", "state": "closed", "user": {"login": "x"},
             "pull_request": {"merged_at": "2026-01-02T00:00:00Z"}},
            {"number": 2, "title": "b", "state": "closed", "user": {"login": "x"}},
            {"number": 3, "title": "c", "state": "open", "draft": true,
             "pull_request": {"merged_at": null}, "labels": [{"name": "bug"}, "docs"]}
        ]);
        let items = parse_issues(body, false, "acme/tools", None, RequestScope::Open).unwrap();
        assert_eq!(items[0].state, RequestState::Merged);
        assert_eq!(items[0].kind, RequestKind::Change);
        assert_eq!(items[1].state, RequestState::Closed);
        assert_eq!(items[1].kind, RequestKind::Issue);
        assert!(items[2].draft);
        assert_eq!(items[2].labels, vec!["bug".to_owned(), "docs".to_owned()]);
    }

    #[test]
    fn mine_is_set_for_author_or_assignee_and_search_marks_reviews() {
        let body = json!({"total_count": 1, "items": [
            {"number": 9, "title": "rev", "state": "open", "user": {"login": "bia"},
             "assignees": [{"login": "ana-dev"}], "pull_request": {"merged_at": null}}
        ]});
        let items =
            parse_issues(body, true, "acme/tools", Some("ana-dev"), RequestScope::ToReview).unwrap();
        assert!(items[0].mine);
        assert!(items[0].review_requested);
    }

    #[test]
    fn comments_and_login_parse() {
        let comments = parse_comments(json!([
            {"id": 5, "user": {"login": "bia"}, "body": "ok", "created_at": "2026-01-03T10:00:00Z"}
        ]))
        .unwrap();
        assert_eq!(comments[0].author, "bia");
        assert_eq!(comments[0].id.as_deref(), Some("5"));
        assert_eq!(parse_login(json!({"login": "ana-dev"})).unwrap(), "ana-dev");
        assert!(matches!(parse_login(json!({})), Err(CounterError::Decode { .. })));
    }
}
