//! API GraphQL do Linear: consultas, mutações e leitura das respostas. Não faz rede.
//!
//! O Linear não tem revisão nem repositório: "Para revisar" responde `unsupported`, e a
//! fonte é a equipe configurada em `counter.linear.team` (vazio = todas as visíveis).

use forja_contracts::{
    CounterError, RequestComment, RequestItem, RequestKind, RequestScope, RequestState,
};
use forja_counter_kit::{HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::{json, Map, Value};

pub const ENDPOINT: &str = "https://api.linear.app/graphql";
pub const PAGE_SIZE: u32 = 50;
pub const MAX_PAGES: usize = 5;

pub const LIST_QUERY: &str = "query Issues($first: Int!, $after: String, $filter: IssueFilter) { \
    issues(first: $first, after: $after, filter: $filter, orderBy: updatedAt) { \
    nodes { id identifier title url updatedAt description state { name type } \
    assignee { name } creator { name } labels { nodes { name } } team { key } } \
    pageInfo { hasNextPage endCursor } } }";

pub const VIEWER_QUERY: &str = "query Viewer { viewer { id name email } }";

/// Confere o identificador `ENG-42` antes de qualquer envio.
pub fn check_identifier(key: &str) -> Result<(), CounterError> {
    let valid = key.split_once('-').is_some_and(|(team, number)| {
        !team.is_empty()
            && team.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            && !number.is_empty()
            && number.chars().all(|c| c.is_ascii_digit())
    });
    if valid {
        Ok(())
    } else {
        Err(CounterError::Rejected {
            message: "the Linear key must look like ENG-42".to_owned(),
        })
    }
}

/// Requisição GraphQL: um só endpoint, token puro no cabeçalho `Authorization`.
pub fn graphql(token: &str, query: &str, variables: Value) -> HttpRequest {
    HttpRequest::post(ENDPOINT, json!({ "query": query, "variables": variables }))
        .header("Authorization", token)
        .header("Content-Type", "application/json")
}

/// Filtro de uma lista. `team` vazio mostra todas as equipes visíveis.
pub fn list_variables(scope: RequestScope, team: &str, after: Option<&str>) -> Result<Value, CounterError> {
    let mut filter = Map::new();
    if !team.is_empty() {
        filter.insert("team".into(), json!({ "key": { "eq": team } }));
    }
    let open_states = json!({ "state": { "type": { "nin": ["completed", "canceled"] } } });
    match scope {
        RequestScope::All => {}
        RequestScope::Open => merge(&mut filter, open_states),
        RequestScope::Mine => {
            merge(&mut filter, open_states);
            filter.insert("assignee".into(), json!({ "isMe": { "eq": true } }));
        }
        RequestScope::ToReview => return Err(CounterError::Unsupported),
    }
    Ok(json!({
        "first": PAGE_SIZE,
        "after": after,
        "filter": Value::Object(filter),
    }))
}

fn merge(target: &mut Map<String, Value>, value: Value) {
    if let Value::Object(fields) = value {
        target.extend(fields);
    }
}

pub fn get_variables(key: &str) -> Value {
    json!({ "id": key })
}

pub const GET_QUERY: &str = "query Issue($id: String!) { issue(id: $id) { id identifier title url updatedAt \
    description state { name type } assignee { name } creator { name } labels { nodes { name } } \
    team { key } comments { nodes { id body createdAt user { name } } } } }";

pub const STATES_QUERY: &str = "query Issue($id: String!) { issue(id: $id) { id team { states { nodes { id name type } } } } }";

pub const COMMENT_MUTATION: &str = "mutation CommentCreate($input: CommentCreateInput!) { \
    commentCreate(input: $input) { success comment { id body createdAt user { name } } } }";

pub const UPDATE_MUTATION: &str = "mutation IssueUpdate($id: String!, $input: IssueUpdateInput!) { \
    issueUpdate(id: $id, input: $input) { success issue { id identifier title url updatedAt \
    description state { name type } assignee { name } creator { name } labels { nodes { name } } team { key } } } }";

/// Escolhe o estado do Linear para `open` (não concluído) ou `closed` (concluído).
pub fn pick_state(states: &Value, to: RequestState) -> Result<String, CounterError> {
    let wanted = match to {
        RequestState::Closed => "completed",
        RequestState::Open => "unstarted",
        RequestState::Merged => return Err(CounterError::Unsupported),
    };
    let nodes = states
        .pointer("/data/issue/team/states/nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| CounterError::Decode {
            message: "the team has no workflow states".to_owned(),
        })?;
    nodes
        .iter()
        .find(|state| state["type"] == wanted)
        .or_else(|| {
            // Abrir também aceita estados de backlog ou em andamento, se não houver "não iniciado".
            if to == RequestState::Open {
                nodes.iter().find(|state| state["type"] == "backlog" || state["type"] == "started")
            } else {
                None
            }
        })
        .and_then(|state| state["id"].as_str())
        .map(str::to_owned)
        .ok_or(CounterError::Unsupported)
}

/// Erros do GraphQL vêm com HTTP 200 e uma lista `errors`. Traduz o código para o tipo de erro.
pub fn graphql_error(response: &HttpResponse) -> Result<Value, CounterError> {
    let body = response.json()?;
    let Some(errors) = body.get("errors").and_then(Value::as_array) else {
        return Ok(body);
    };
    let first = errors.first();
    let code = first
        .and_then(|error| error.pointer("/extensions/code"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let message = first
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("request failed")
        .to_owned();
    Err(match code {
        "AUTHENTICATION_ERROR" => CounterError::Unauthorized,
        "FORBIDDEN" => CounterError::Forbidden,
        "NOT_FOUND" | "ENTITY_NOT_FOUND" => CounterError::NotFound,
        "RATELIMITED" => CounterError::RateLimited { retry_after_s: None },
        _ => CounterError::Rejected { message },
    })
}

#[derive(Debug, Deserialize)]
struct LinNode {
    identifier: String,
    title: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default, rename = "updatedAt")]
    updated_at: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    state: Option<LinState>,
    #[serde(default)]
    assignee: Option<LinName>,
    #[serde(default)]
    creator: Option<LinName>,
    #[serde(default)]
    labels: Option<LinLabels>,
}

#[derive(Debug, Deserialize)]
struct LinState {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize)]
struct LinName {
    name: String,
}

#[derive(Debug, Deserialize)]
struct LinLabels {
    nodes: Vec<LinName>,
}

#[derive(Debug, Deserialize)]
struct LinComment {
    id: String,
    body: String,
    #[serde(default, rename = "createdAt")]
    created_at: Option<String>,
    #[serde(default)]
    user: Option<LinName>,
}

/// Lista de issues com a página seguinte. Retorna também o cursor, se houver.
pub fn parse_page(data: &Value, login: Option<&str>) -> Result<(Vec<RequestItem>, Option<String>), CounterError> {
    let connection = data.pointer("/data/issues").ok_or_else(|| CounterError::Decode {
        message: "the response has no issues list".to_owned(),
    })?;
    let nodes: Vec<LinNode> = serde_json::from_value(connection["nodes"].clone()).map_err(decode)?;
    let items = nodes
        .into_iter()
        .map(|node| item_from(node, login))
        .collect();
    let next = connection
        .pointer("/pageInfo/hasNextPage")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        .then(|| connection.pointer("/pageInfo/endCursor").and_then(Value::as_str).map(str::to_owned))
        .flatten();
    Ok((items, next))
}

/// Um issue com comentários (`data.issue`).
pub fn parse_issue(data: &Value, login: Option<&str>) -> Result<RequestItem, CounterError> {
    let issue = data.pointer("/data/issue").ok_or(CounterError::NotFound)?;
    let node: LinNode = serde_json::from_value(issue.clone()).map_err(decode)?;
    let comments: Vec<LinComment> =
        serde_json::from_value(issue["comments"]["nodes"].clone()).unwrap_or_default();
    let mut item = item_from(node, login);
    item.comments = comments.into_iter().map(comment_from).collect();
    Ok(item)
}

pub fn parse_comment(data: &Value) -> Result<RequestComment, CounterError> {
    let comment = data
        .pointer("/data/commentCreate/comment")
        .ok_or_else(|| CounterError::Rejected {
            message: "the comment was not created".to_owned(),
        })?;
    let comment: LinComment = serde_json::from_value(comment.clone()).map_err(decode)?;
    Ok(comment_from(comment))
}

pub fn parse_updated(data: &Value, login: Option<&str>) -> Result<RequestItem, CounterError> {
    let issue = data
        .pointer("/data/issueUpdate/issue")
        .ok_or_else(|| CounterError::Rejected {
            message: "the issue was not updated".to_owned(),
        })?;
    let node: LinNode = serde_json::from_value(issue.clone()).map_err(decode)?;
    Ok(item_from(node, login))
}

pub fn parse_viewer(data: &Value) -> Result<String, CounterError> {
    let viewer = data.pointer("/data/viewer").ok_or_else(|| CounterError::Decode {
        message: "the response has no viewer".to_owned(),
    })?;
    viewer["name"]
        .as_str()
        .or_else(|| viewer["email"].as_str())
        .map(str::to_owned)
        .ok_or_else(|| CounterError::Decode {
            message: "the viewer has no name".to_owned(),
        })
}

/// Id interno (UUID) de um issue, lido da consulta de estados.
pub fn issue_id(data: &Value) -> Result<String, CounterError> {
    data.pointer("/data/issue/id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(CounterError::NotFound)
}

fn decode(error: serde_json::Error) -> CounterError {
    CounterError::Decode {
        message: error.to_string(),
    }
}

fn comment_from(comment: LinComment) -> RequestComment {
    RequestComment {
        id: Some(comment.id),
        author: comment.user.map(|user| user.name).unwrap_or_default(),
        body: comment.body,
        created_at: comment.created_at,
    }
}

fn item_from(node: LinNode, login: Option<&str>) -> RequestItem {
    let state = match node.state.as_ref().map(|state| state.kind.as_str()) {
        Some("completed") => RequestState::Closed,
        Some("canceled") => RequestState::Closed,
        _ => RequestState::Open,
    };
    let assignee = node.assignee.map(|user| user.name);
    let author = node.creator.map(|user| user.name);
    let mine = login.is_some_and(|login| assignee.as_deref() == Some(login));
    let labels = node
        .labels
        .map(|labels| labels.nodes.into_iter().map(|label| label.name).collect())
        .unwrap_or_default();
    let mut item = RequestItem::new("linear", node.identifier, node.title);
    item.kind = RequestKind::Issue;
    item.state = state;
    item.url = node.url;
    item.author = author;
    item.assignees = assignee.into_iter().collect();
    item.labels = labels;
    item.updated_at = node.updated_at;
    item.mine = mine;
    item.body = node.description;
    item
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_checked_before_any_request() {
        assert!(check_identifier("ENG-42").is_ok());
        assert!(check_identifier("eng-42").is_err());
        assert!(check_identifier("ENG-").is_err());
        assert!(check_identifier("42").is_err());
    }

    #[test]
    fn review_is_unsupported_and_mine_filters_by_viewer() {
        assert_eq!(list_variables(RequestScope::ToReview, "", None), Err(CounterError::Unsupported));
        let mine = list_variables(RequestScope::Mine, "ENG", None).unwrap();
        assert_eq!(mine["filter"]["team"]["key"]["eq"], "ENG");
        assert_eq!(mine["filter"]["assignee"]["isMe"]["eq"], true);
        assert_eq!(mine["filter"]["state"]["type"]["nin"][0], "completed");
        let all = list_variables(RequestScope::All, "", Some("c1")).unwrap();
        assert_eq!(all["after"], "c1");
        assert!(all["filter"].as_object().unwrap().is_empty());
    }

    #[test]
    fn graphql_errors_map_to_kinds_even_with_http_200() {
        let auth = HttpResponse::new(
            200,
            r#"{"errors":[{"message":"bad key","extensions":{"code":"AUTHENTICATION_ERROR"}}]}"#,
        );
        assert_eq!(graphql_error(&auth), Err(CounterError::Unauthorized));
        let other = HttpResponse::new(200, r#"{"errors":[{"message":"no team"}]}"#);
        assert_eq!(
            graphql_error(&other),
            Err(CounterError::Rejected {
                message: "no team".into()
            })
        );
        assert!(graphql_error(&HttpResponse::new(200, r#"{"data":{}}"#)).is_ok());
    }

    #[test]
    fn states_are_picked_by_type() {
        let states = serde_json::json!({"data": {"issue": {"team": {"states": {"nodes": [
            {"id": "s1", "name": "Backlog", "type": "backlog"},
            {"id": "s2", "name": "Done", "type": "completed"}
        ]}}}}});
        assert_eq!(pick_state(&states, RequestState::Closed).unwrap(), "s2");
        assert_eq!(pick_state(&states, RequestState::Open).unwrap(), "s1");
        assert_eq!(pick_state(&states, RequestState::Merged), Err(CounterError::Unsupported));
    }

    #[test]
    fn pages_expose_the_cursor_only_when_there_is_more() {
        let data = serde_json::json!({"data": {"issues": {
            "nodes": [{"id": "u1", "identifier": "ENG-1", "title": "a",
                       "state": {"type": "started"}, "assignee": {"name": "Ana"}}],
            "pageInfo": {"hasNextPage": true, "endCursor": "c2"}}}});
        let (items, next) = parse_page(&data, Some("Ana")).unwrap();
        assert_eq!(items[0].key, "ENG-1");
        assert!(items[0].mine);
        assert_eq!(next.as_deref(), Some("c2"));
    }
}
