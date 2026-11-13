//! API REST v3 do Jira: consultas JQL, transições e leitura das respostas. Não faz rede.
//!
//! O site e o e-mail não vêm das configurações: são guardados no cofre ao conectar. A chave
//! do projeto (`counter.jira.project`) entra no JQL só depois de conferida.

use forja_contracts::{
    CounterError, RequestComment, RequestItem, RequestKind, RequestScope, RequestState,
};
use forja_counter_kit::url::encode_component;
use forja_counter_kit::HttpRequest;
use serde_json::{json, Value};

pub const PAGE_SIZE: u32 = 50;
pub const MAX_PAGES: usize = 5;
const FIELDS: [&str; 9] = [
    "summary",
    "status",
    "assignee",
    "reporter",
    "labels",
    "updated",
    "description",
    "project",
    "comment",
];

/// `acme.atlassian.net`, sem esquema nem barra final. Aceita o que o usuário colar.
pub fn normalize_site(site: &str) -> Result<String, CounterError> {
    let site = site
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let valid = !site.is_empty()
        && site.contains('.')
        && site
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
    if valid {
        Ok(site.to_ascii_lowercase())
    } else {
        Err(CounterError::Config {
            message: "the Jira site must look like acme.atlassian.net".to_owned(),
        })
    }
}

/// Chave de projeto do Jira (`ENG`): letras maiúsculas, dígitos e `_`, começando por letra.
pub fn check_project(key: &str) -> Result<(), CounterError> {
    let mut chars = key.chars();
    let valid = matches!(chars.next(), Some(c) if c.is_ascii_uppercase())
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if valid {
        Ok(())
    } else {
        Err(CounterError::Config {
            message: "the Jira project key must look like ENG".to_owned(),
        })
    }
}

/// `ENG-42`: a chave de um pedido Jira, conferida antes do envio.
pub fn check_issue_key(key: &str) -> Result<(), CounterError> {
    let valid = key.split_once('-').is_some_and(|(project, number)| {
        check_project(project).is_ok() && !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
    });
    if valid {
        Ok(())
    } else {
        Err(CounterError::Rejected {
            message: "the Jira key must look like ENG-42".to_owned(),
        })
    }
}

/// JQL do escopo. Só recebe chaves já conferidas.
pub fn jql(scope: RequestScope, project: &str) -> Result<String, CounterError> {
    let mut clauses = Vec::new();
    if !project.is_empty() {
        check_project(project)?;
        clauses.push(format!("project = {project}"));
    }
    let open = "statusCategory != Done";
    match scope {
        RequestScope::Open => clauses.push(open.to_owned()),
        RequestScope::Mine => {
            clauses.push(open.to_owned());
            clauses.push("assignee = currentUser()".to_owned());
        }
        RequestScope::All => {}
        RequestScope::ToReview => return Err(CounterError::Unsupported),
    }
    if clauses.is_empty() {
        Ok("ORDER BY updated DESC".to_owned())
    } else {
        Ok(format!("{} ORDER BY updated DESC", clauses.join(" AND ")))
    }
}

pub fn search_request(api: &str, jql: &str, next: Option<&str>) -> HttpRequest {
    let mut body = json!({
        "jql": jql,
        "maxResults": PAGE_SIZE,
        "fields": FIELDS,
    });
    if let Some(token) = next {
        body["nextPageToken"] = Value::String(token.to_owned());
    }
    HttpRequest::post(format!("{api}/search/jql"), body)
}

pub fn issue_request(api: &str, key: &str) -> HttpRequest {
    HttpRequest::get(format!(
        "{api}/issue/{}?fields={}",
        encode_component(key),
        FIELDS.join(",")
    ))
}

pub fn comment_request(api: &str, key: &str, body: &str) -> HttpRequest {
    HttpRequest::post(
        format!("{api}/issue/{}/comment", encode_component(key)),
        json!({ "body": text_to_adf(body) }),
    )
}

pub fn transitions_request(api: &str, key: &str) -> HttpRequest {
    HttpRequest::get(format!("{api}/issue/{}/transitions", encode_component(key)))
}

pub fn apply_transition_request(api: &str, key: &str, transition_id: &str) -> HttpRequest {
    HttpRequest::post(
        format!("{api}/issue/{}/transitions", encode_component(key)),
        json!({ "transition": { "id": transition_id } }),
    )
}

pub fn myself_request(api: &str) -> HttpRequest {
    HttpRequest::get(format!("{api}/myself"))
}

/// Escolhe a transição para `open` (categoria "to do") ou `closed` (categoria "done").
pub fn pick_transition(body: &Value, to: RequestState) -> Result<String, CounterError> {
    let wanted = match to {
        RequestState::Closed => "done",
        RequestState::Open => "new",
        RequestState::Merged => return Err(CounterError::Unsupported),
    };
    let transitions = body
        .get("transitions")
        .and_then(Value::as_array)
        .ok_or_else(|| CounterError::Decode {
            message: "the issue has no transitions list".to_owned(),
        })?;
    transitions
        .iter()
        .find(|transition| transition["to"]["statusCategory"]["key"] == wanted)
        .and_then(|transition| transition["id"].as_str())
        .map(str::to_owned)
        .ok_or(CounterError::Unsupported)
}

/// Mensagem legível do Jira para um erro: `errorMessages` ou o primeiro valor de `errors`.
pub fn error_message(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    value
        .get("errorMessages")
        .and_then(Value::as_array)
        .and_then(|messages| messages.first())
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            value
                .get("errors")
                .and_then(Value::as_object)
                .and_then(|errors| errors.values().find_map(Value::as_str))
                .map(str::to_owned)
        })
}

/// Texto simples para ADF (o formato de documento do Jira).
pub fn text_to_adf(text: &str) -> Value {
    let paragraphs: Vec<Value> = text
        .lines()
        .map(|line| {
            json!({
                "type": "paragraph",
                "content": if line.is_empty() {
                    Vec::new()
                } else {
                    vec![json!({ "type": "text", "text": line })]
                },
            })
        })
        .collect();
    json!({ "type": "doc", "version": 1, "content": paragraphs })
}

/// Extrai o texto de um documento ADF, um parágrafo por linha.
pub fn adf_to_text(node: &Value) -> String {
    let mut out = String::new();
    collect_text(node, &mut out);
    out.trim().to_owned()
}

fn collect_text(node: &Value, out: &mut String) {
    match node {
        Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(Value::as_str) {
                out.push_str(text);
            }
            if let Some(children) = map.get("content").and_then(Value::as_array) {
                for child in children {
                    collect_text(child, out);
                }
                if map.get("type").and_then(Value::as_str) == Some("paragraph") {
                    out.push('\n');
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_text(item, out);
            }
        }
        _ => {}
    }
}

/// Uma página de busca: pedidos, token da próxima página e se é a última.
pub fn parse_search(body: Value, site: &str, login: Option<&str>) -> Result<(Vec<RequestItem>, Option<String>), CounterError> {
    let issues = body
        .get("issues")
        .and_then(Value::as_array)
        .ok_or_else(|| CounterError::Decode {
            message: "the search has no issues list".to_owned(),
        })?;
    let items = issues
        .iter()
        .map(|issue| item_from(issue, site, login))
        .collect();
    let last = body.get("isLast").and_then(Value::as_bool).unwrap_or(true);
    let next = if last {
        None
    } else {
        body.get("nextPageToken").and_then(Value::as_str).map(str::to_owned)
    };
    Ok((items, next))
}

pub fn parse_issue(body: &Value, site: &str, login: Option<&str>) -> RequestItem {
    let mut item = item_from(body, site, login);
    item.body = body.pointer("/fields/description").map(adf_to_text);
    item.comments = body
        .pointer("/fields/comment/comments")
        .and_then(Value::as_array)
        .map(|comments| comments.iter().map(comment_from).collect())
        .unwrap_or_default();
    item
}

pub fn parse_comment(body: &Value) -> RequestComment {
    comment_from(body)
}

pub fn parse_display_name(body: Value) -> Result<String, CounterError> {
    body.get("displayName")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| CounterError::Decode {
            message: "the account has no display name".to_owned(),
        })
}

fn comment_from(comment: &Value) -> RequestComment {
    RequestComment {
        id: comment.get("id").and_then(Value::as_str).map(str::to_owned),
        author: comment
            .pointer("/author/displayName")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        body: comment.get("body").map(adf_to_text).unwrap_or_default(),
        created_at: comment.get("created").and_then(Value::as_str).map(str::to_owned),
    }
}

fn item_from(issue: &Value, site: &str, login: Option<&str>) -> RequestItem {
    let key = issue.get("key").and_then(Value::as_str).unwrap_or_default().to_owned();
    let title = issue
        .pointer("/fields/summary")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let state = match issue.pointer("/fields/status/statusCategory/key").and_then(Value::as_str) {
        Some("done") => RequestState::Closed,
        _ => RequestState::Open,
    };
    let assignee = issue.pointer("/fields/assignee/displayName").and_then(Value::as_str).map(str::to_owned);
    let reporter = issue.pointer("/fields/reporter/displayName").and_then(Value::as_str).map(str::to_owned);
    let mine = login.is_some_and(|login| assignee.as_deref() == Some(login));
    let labels = issue
        .pointer("/fields/labels")
        .and_then(Value::as_array)
        .map(|labels| labels.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    let mut item = RequestItem::new("jira", key.clone(), title);
    item.kind = RequestKind::Issue;
    item.state = state;
    item.url = Some(format!("https://{site}/browse/{key}"));
    item.author = reporter;
    item.assignees = assignee.into_iter().collect();
    item.labels = labels;
    item.project = issue.pointer("/fields/project/key").and_then(Value::as_str).map(str::to_owned);
    item.updated_at = issue.pointer("/fields/updated").and_then(Value::as_str).map(str::to_owned);
    item.mine = mine;
    // Jira não tem revisão: nenhum pedido vem marcado como revisão pedida.
    item
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sites_are_normalized_and_bad_ones_refused() {
        assert_eq!(normalize_site(" https://Acme.atlassian.net/ ").unwrap(), "acme.atlassian.net");
        assert!(normalize_site("acme").is_err());
        assert!(normalize_site("evil.example/path").is_err());
    }

    #[test]
    fn project_and_issue_keys_are_checked() {
        assert!(check_project("ENG").is_ok());
        assert!(check_project("eng").is_err());
        assert!(check_project("ENG\" OR 1=1").is_err(), "no JQL injection");
        assert!(check_issue_key("ENG-42").is_ok());
        assert!(check_issue_key("ENG42").is_err());
    }

    #[test]
    fn jql_follows_the_scope() {
        assert_eq!(
            jql(RequestScope::Mine, "ENG").unwrap(),
            "project = ENG AND statusCategory != Done AND assignee = currentUser() ORDER BY updated DESC"
        );
        assert_eq!(jql(RequestScope::All, "").unwrap(), "ORDER BY updated DESC");
        assert_eq!(jql(RequestScope::ToReview, "ENG"), Err(CounterError::Unsupported));
        assert!(jql(RequestScope::Open, "ENG;drop").is_err());
    }

    #[test]
    fn adf_round_trips_line_breaks() {
        let adf = text_to_adf("primeira\n\nsegunda");
        assert_eq!(adf_to_text(&adf), "primeira\n\nsegunda");
        assert_eq!(adf["type"], "doc");
    }

    #[test]
    fn transitions_are_picked_by_category() {
        let body = json!({"transitions": [
            {"id": "11", "name": "Start", "to": {"statusCategory": {"key": "indeterminate"}}},
            {"id": "31", "name": "Done", "to": {"statusCategory": {"key": "done"}}}
        ]});
        assert_eq!(pick_transition(&body, RequestState::Closed).unwrap(), "31");
        assert_eq!(pick_transition(&body, RequestState::Open), Err(CounterError::Unsupported));
    }
}
