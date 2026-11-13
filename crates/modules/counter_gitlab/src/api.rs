//! API REST do GitLab (v4): montagem das requisições e leitura das respostas. Não faz rede.
//!
//! Issues têm a chave `#iid`; merge requests têm `!iid`. Para "Para revisar" usamos merge
//! requests com `reviewer_username`; para "Meus", `scope=assigned_to_me` nos dois tipos.

use forja_contracts::{CounterError, RequestComment, RequestItem, RequestKind, RequestScope, RequestState};
use forja_counter_kit::url::encode_component;
use forja_counter_kit::HttpRequest;
use serde::Deserialize;
use serde_json::{json, Value};

pub const PAGE_SIZE: u32 = 50;
/// Páginas por lista (issues e merge requests têm listas separadas).
pub const MAX_PAGES: usize = 5;

pub fn api_base(host: &str) -> String {
    format!("https://{host}/api/v4")
}

/// Caminho do projeto codificado para a URL: `grupo%2Fprojeto`.
pub fn project_segment(path: &str) -> String {
    encode_component(path)
}

/// Chave de um pedido: `#12` para issue, `!45` para merge request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Issue,
    Change,
}

pub fn parse_key(key: &str) -> Result<(Kind, u64), CounterError> {
    let (kind, digits) = match key.chars().next() {
        Some('#') => (Kind::Issue, &key[1..]),
        Some('!') => (Kind::Change, &key[1..]),
        _ => {
            return Err(CounterError::Rejected {
                message: "the GitLab key must look like #12 or !45".to_owned(),
            })
        }
    };
    digits
        .parse::<u64>()
        .map(|number| (kind, number))
        .map_err(|_| CounterError::Rejected {
            message: "the GitLab key must end in a number".to_owned(),
        })
}

/// Lista pedida por um escopo. Issues não têm revisão: `ToReview` só lista merge requests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListQuery {
    pub project: String,
    pub scope: RequestScope,
    pub login: Option<String>,
    pub kind: Kind,
}

pub fn list_request(api: &str, query: &ListQuery, page: u32) -> Result<HttpRequest, CounterError> {
    let base = format!("{api}/projects/{}", project_segment(&query.project));
    let (path, filter) = match (query.kind, query.scope) {
        (Kind::Issue, RequestScope::ToReview) => {
            return Err(CounterError::Unsupported);
        }
        (Kind::Issue, RequestScope::Mine) => ("issues", "state=opened&scope=assigned_to_me".to_owned()),
        (Kind::Issue, RequestScope::Open) => ("issues", "state=opened".to_owned()),
        (Kind::Issue, RequestScope::All) => ("issues", "state=all".to_owned()),
        (Kind::Change, RequestScope::Mine) => {
            ("merge_requests", "state=opened&scope=assigned_to_me".to_owned())
        }
        (Kind::Change, RequestScope::Open) => ("merge_requests", "state=opened".to_owned()),
        (Kind::Change, RequestScope::All) => ("merge_requests", "state=all".to_owned()),
        (Kind::Change, RequestScope::ToReview) => {
            let login = query.login.as_deref().ok_or_else(|| CounterError::Config {
                message: "the GitLab account login is unknown; test the connection again".to_owned(),
            })?;
            (
                "merge_requests",
                format!("state=opened&reviewer_username={}", encode_component(login)),
            )
        }
    };
    Ok(HttpRequest::get(format!(
        "{base}/{path}?{filter}&per_page={PAGE_SIZE}&page={page}&order_by=updated_at&sort=desc"
    )))
}

/// Um pedido: `issues/{iid}` ou `merge_requests/{iid}`.
pub fn item_path(project: &str, kind: Kind, number: u64) -> String {
    let collection = match kind {
        Kind::Issue => "issues",
        Kind::Change => "merge_requests",
    };
    format!("/projects/{}/{collection}/{number}", project_segment(project))
}

pub fn get_request(api: &str, project: &str, kind: Kind, number: u64) -> HttpRequest {
    HttpRequest::get(format!("{api}{}", item_path(project, kind, number)))
}

pub fn notes_request(api: &str, project: &str, kind: Kind, number: u64) -> HttpRequest {
    HttpRequest::get(format!(
        "{api}{}/notes?sort=asc&per_page=100",
        item_path(project, kind, number)
    ))
}

pub fn note_request(api: &str, project: &str, kind: Kind, number: u64, body: &str) -> HttpRequest {
    HttpRequest::post(
        format!("{api}{}/notes", item_path(project, kind, number)),
        json!({ "body": body }),
    )
}

/// Fecha ou reabre: `state_event` vale `close` ou `reopen`.
pub fn state_request(api: &str, project: &str, kind: Kind, number: u64, open: bool) -> HttpRequest {
    let event = if open { "reopen" } else { "close" };
    HttpRequest::put(
        format!("{api}{}", item_path(project, kind, number)),
        json!({ "state_event": event }),
    )
}

pub fn user_request(api: &str) -> HttpRequest {
    HttpRequest::get(format!("{api}/user"))
}

#[derive(Debug, Deserialize)]
struct GlUser {
    username: String,
}

#[derive(Debug, Deserialize)]
struct GlItem {
    iid: u64,
    title: String,
    state: String,
    #[serde(default)]
    web_url: Option<String>,
    #[serde(default)]
    author: Option<GlUser>,
    #[serde(default)]
    assignees: Vec<GlUser>,
    #[serde(default)]
    reviewers: Vec<GlUser>,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    work_in_progress: bool,
}

#[derive(Debug, Deserialize)]
struct GlNote {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    system: bool,
    #[serde(default)]
    author: Option<GlUser>,
    body: String,
    #[serde(default)]
    created_at: Option<String>,
}

/// Converte uma lista de issues ou merge requests em pedidos comuns.
pub fn parse_items(
    body: Value,
    kind: Kind,
    project: &str,
    login: Option<&str>,
    scope: RequestScope,
) -> Result<Vec<RequestItem>, CounterError> {
    let items: Vec<GlItem> = serde_json::from_value(body).map_err(decode)?;
    Ok(items
        .into_iter()
        .map(|item| item_from(item, kind, project, login, scope == RequestScope::ToReview))
        .collect())
}

pub fn parse_item(body: Value, kind: Kind, project: &str, login: Option<&str>) -> Result<RequestItem, CounterError> {
    let item: GlItem = serde_json::from_value(body).map_err(decode)?;
    Ok(item_from(item, kind, project, login, false))
}

/// Notas de sistema (mudança de rótulo, de etapa…) não são comentários: ficam de fora.
pub fn parse_notes(body: Value) -> Result<Vec<RequestComment>, CounterError> {
    let notes: Vec<GlNote> = serde_json::from_value(body).map_err(decode)?;
    Ok(notes
        .into_iter()
        .filter(|note| !note.system)
        .map(note_from)
        .collect())
}

pub fn parse_note(body: Value) -> Result<RequestComment, CounterError> {
    let note: GlNote = serde_json::from_value(body).map_err(decode)?;
    Ok(note_from(note))
}

pub fn parse_login(body: Value) -> Result<String, CounterError> {
    let user: GlUser = serde_json::from_value(body).map_err(decode)?;
    Ok(user.username)
}

fn decode(error: serde_json::Error) -> CounterError {
    CounterError::Decode {
        message: error.to_string(),
    }
}

fn note_from(note: GlNote) -> RequestComment {
    RequestComment {
        id: note.id.map(|id| id.to_string()),
        author: note.author.map(|user| user.username).unwrap_or_default(),
        body: note.body,
        created_at: note.created_at,
    }
}

fn item_from(item: GlItem, kind: Kind, project: &str, login: Option<&str>, review: bool) -> RequestItem {
    let (request_kind, key, state) = match kind {
        Kind::Issue => (
            RequestKind::Issue,
            format!("#{}", item.iid),
            if item.state == "closed" {
                RequestState::Closed
            } else {
                RequestState::Open
            },
        ),
        Kind::Change => (
            RequestKind::Change,
            format!("!{}", item.iid),
            match item.state.as_str() {
                "merged" => RequestState::Merged,
                "opened" => RequestState::Open,
                _ => RequestState::Closed,
            },
        ),
    };
    let author = item.author.map(|user| user.username);
    let assignees: Vec<String> = item.assignees.into_iter().map(|user| user.username).collect();
    let reviewers: Vec<String> = item.reviewers.into_iter().map(|user| user.username).collect();
    let mine = login.is_some_and(|login| {
        author.as_deref() == Some(login) || assignees.iter().any(|name| name == login)
    });
    let review_requested = review
        || login.is_some_and(|login| reviewers.iter().any(|name| name == login));
    RequestItem {
        counter: "gitlab".to_owned(),
        key,
        title: item.title,
        kind: request_kind,
        state,
        draft: item.draft || item.work_in_progress,
        url: item.web_url,
        author,
        assignees,
        labels: item.labels,
        project: Some(project.to_owned()),
        updated_at: item.updated_at,
        mine,
        review_requested,
        body: item.description,
        comments: Vec::new(),
        checks: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_paths_are_encoded_as_one_segment() {
        assert_eq!(project_segment("grupo/sub/app"), "grupo%2Fsub%2Fapp");
        let query = ListQuery {
            project: "grupo/app".into(),
            scope: RequestScope::Open,
            login: None,
            kind: Kind::Issue,
        };
        let request = list_request("https://gitlab.com/api/v4", &query, 2).unwrap();
        assert!(request.url.contains("/projects/grupo%2Fapp/issues?state=opened"));
        assert!(request.url.contains("page=2"));
    }

    #[test]
    fn keys_decide_the_collection() {
        assert_eq!(parse_key("#12"), Ok((Kind::Issue, 12)));
        assert_eq!(parse_key("!45"), Ok((Kind::Change, 45)));
        assert!(parse_key("12").is_err());
        assert!(parse_key("#x").is_err());
        assert!(item_path("a/b", Kind::Change, 3).contains("/merge_requests/3"));
    }

    #[test]
    fn issues_have_no_review_scope() {
        let query = ListQuery {
            project: "a/b".into(),
            scope: RequestScope::ToReview,
            login: Some("ana".into()),
            kind: Kind::Issue,
        };
        assert_eq!(list_request("https://x/api/v4", &query, 1), Err(CounterError::Unsupported));
    }

    #[test]
    fn merge_requests_map_states_and_reviews() {
        let body = serde_json::json!([
            {"iid": 7, "title": "a", "state": "merged", "author": {"username": "bia"}},
            {"iid": 8, "title": "b", "state": "opened", "work_in_progress": true,
             "reviewers": [{"username": "ana"}]}
        ]);
        let items = parse_items(body, Kind::Change, "a/b", Some("ana"), RequestScope::Open).unwrap();
        assert_eq!(items[0].key, "!7");
        assert_eq!(items[0].state, RequestState::Merged);
        assert!(items[1].draft);
        assert!(items[1].review_requested);
    }

    #[test]
    fn system_notes_are_not_comments() {
        let notes = parse_notes(serde_json::json!([
            {"id": 1, "system": true, "body": "changed the label", "author": {"username": "bot"}},
            {"id": 2, "system": false, "body": "obrigado", "author": {"username": "bia"}}
        ]))
        .unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].author, "bia");
    }
}
