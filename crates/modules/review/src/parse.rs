//! Provider JSON into the small common model used by the Review tab.

use crate::api::Provider;
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: String,
    pub draft: bool,
    pub url: String,
    pub author: String,
    pub source: String,
    pub target: String,
    pub mergeable: String,
    pub merge_state: String,
    pub reviewers: Vec<String>,
    pub labels: Vec<String>,
}

impl PullRequest {
    pub fn is_open(&self) -> bool {
        matches!(self.state.as_str(), "open" | "opened")
    }

    pub fn display_state(&self) -> &str {
        if self.draft && self.is_open() {
            "draft"
        } else {
            self.state.as_str()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Check {
    pub id: u64,
    pub name: String,
    pub state: String,
    pub url: String,
    pub log: String,
    pub started_at: String,
    pub finished_at: String,
    /// GitLab pipelines need a second request for their jobs.
    pub pipeline: bool,
}

impl Check {
    pub fn passed(&self) -> bool {
        matches!(self.state.as_str(), "success" | "passed" | "completed")
    }

    pub fn failed(&self) -> bool {
        matches!(
            self.state.as_str(),
            "failure" | "failed" | "cancelled" | "timed_out"
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub body: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Thread {
    pub id: String,
    pub path: String,
    pub line: u64,
    pub resolved: bool,
    pub comments: Vec<Comment>,
}

pub fn pull(value: &Value, provider: Provider) -> Option<PullRequest> {
    match provider {
        Provider::GitHub => github_pull(value),
        Provider::GitLab => gitlab_pull(value),
    }
}

pub fn pulls(value: &Value, provider: Provider) -> Vec<PullRequest> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| pull(item, provider))
                .collect()
        })
        .unwrap_or_default()
}

fn github_pull(value: &Value) -> Option<PullRequest> {
    Some(PullRequest {
        number: value.get("number")?.as_u64()?,
        title: text(value, "title"),
        body: text(value, "body"),
        state: text(value, "state"),
        draft: value.get("draft").and_then(Value::as_bool).unwrap_or(false),
        url: text(value, "html_url"),
        author: pointer_text(value, "/user/login"),
        source: pointer_text(value, "/head/ref"),
        target: pointer_text(value, "/base/ref"),
        mergeable: text(value, "mergeable"),
        merge_state: text(value, "mergeable_state"),
        reviewers: names(value.get("requested_reviewers"), "login"),
        labels: names(value.get("labels"), "name"),
    })
}

fn gitlab_pull(value: &Value) -> Option<PullRequest> {
    let state = text(value, "state");
    Some(PullRequest {
        number: value
            .get("iid")
            .and_then(Value::as_u64)
            .or_else(|| value.get("number").and_then(Value::as_u64))?,
        title: text(value, "title"),
        body: text(value, "description"),
        state,
        draft: value
            .get("draft")
            .and_then(Value::as_bool)
            .or_else(|| value.get("work_in_progress").and_then(Value::as_bool))
            .unwrap_or(false),
        url: text(value, "web_url"),
        author: pointer_text(value, "/author/username"),
        source: text(value, "source_branch"),
        target: text(value, "target_branch"),
        mergeable: text(value, "merge_status"),
        merge_state: text(value, "detailed_merge_status"),
        reviewers: names(value.get("reviewers"), "username"),
        labels: value
            .get("labels")
            .and_then(Value::as_array)
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|label| {
                        label.as_str().map(str::to_owned).or_else(|| {
                            label.get("name").and_then(Value::as_str).map(str::to_owned)
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

pub fn checks(value: &Value, provider: Provider) -> Vec<Check> {
    match provider {
        Provider::GitHub => value
            .get("check_runs")
            .and_then(Value::as_array)
            .map(|runs| runs.iter().map(github_check).collect())
            .unwrap_or_default(),
        Provider::GitLab => value
            .as_array()
            .map(|pipelines| {
                pipelines
                    .iter()
                    .filter_map(|pipeline| {
                        let id = pipeline.get("id")?.as_u64()?;
                        Some(Check {
                            id,
                            name: text(pipeline, "status"),
                            state: text(pipeline, "status"),
                            url: text(pipeline, "web_url"),
                            log: String::new(),
                            started_at: text(pipeline, "created_at"),
                            finished_at: text(pipeline, "updated_at"),
                            pipeline: true,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn github_check(value: &Value) -> Check {
    let status = text(value, "status");
    let conclusion = text(value, "conclusion");
    let state = if status == "completed" && !conclusion.is_empty() {
        conclusion
    } else {
        status
    };
    let output = value.get("output").cloned().unwrap_or(Value::Null);
    let mut log = String::new();
    for key in ["title", "summary", "text"] {
        let part = text(&output, key);
        if !part.is_empty() {
            if !log.is_empty() {
                log.push('\n');
            }
            log.push_str(&part);
        }
    }
    Check {
        id: value.get("id").and_then(Value::as_u64).unwrap_or(0),
        name: text(value, "name"),
        state,
        url: text(value, "details_url"),
        log,
        started_at: text(value, "started_at"),
        finished_at: text(value, "completed_at"),
        pipeline: false,
    }
}

pub fn jobs(value: &Value) -> Vec<Check> {
    value
        .as_array()
        .map(|jobs| {
            jobs.iter()
                .filter_map(|job| {
                    Some(Check {
                        id: job.get("id")?.as_u64()?,
                        name: text(job, "name"),
                        state: text(job, "status"),
                        url: text(job, "web_url"),
                        log: String::new(),
                        started_at: text(job, "started_at"),
                        finished_at: text(job, "finished_at"),
                        pipeline: false,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn threads(value: &Value, provider: Provider) -> Vec<Thread> {
    match provider {
        Provider::GitHub => value
            .pointer("/data/repository/pullRequest/reviewThreads/nodes")
            .and_then(Value::as_array)
            .map(|nodes| {
                nodes
                    .iter()
                    .map(|node| Thread {
                        id: text(node, "id"),
                        path: text(node, "path"),
                        line: node.get("line").and_then(Value::as_u64).unwrap_or(0),
                        resolved: node
                            .get("isResolved")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        comments: node
                            .pointer("/comments/nodes")
                            .and_then(Value::as_array)
                            .map(|comments| {
                                comments
                                    .iter()
                                    .map(|comment| Comment {
                                        id: comment
                                            .get("databaseId")
                                            .and_then(Value::as_u64)
                                            .map(|id| id.to_string())
                                            .unwrap_or_else(|| text(comment, "id")),
                                        author: pointer_text(comment, "/author/login"),
                                        body: text(comment, "body"),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Provider::GitLab => value
            .as_array()
            .map(|discussions| {
                discussions
                    .iter()
                    .map(|discussion| {
                        let notes = discussion
                            .get("notes")
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default();
                        let first = notes.first().cloned().unwrap_or(Value::Null);
                        Thread {
                            id: text(discussion, "id"),
                            path: pointer_text(&first, "/position/new_path"),
                            line: first
                                .pointer("/position/new_line")
                                .and_then(Value::as_u64)
                                .unwrap_or(0),
                            resolved: first
                                .get("resolved")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            comments: notes
                                .iter()
                                .map(|note| Comment {
                                    id: note
                                        .get("id")
                                        .and_then(Value::as_u64)
                                        .map(|id| id.to_string())
                                        .unwrap_or_default(),
                                    author: pointer_text(note, "/author/username"),
                                    body: text(note, "body"),
                                })
                                .collect(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

fn pointer_text(value: &Value, pointer: &str) -> String {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

fn names(value: Option<&Value>, key: &str) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get(key).and_then(Value::as_str).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_pull_and_checks_are_parsed_from_fixtures() {
        let pr = serde_json::json!({
            "number": 42,
            "title": "Add login",
            "body": "Body",
            "state": "open",
            "draft": false,
            "html_url": "https://github.com/acme/app/pull/42",
            "user": { "login": "ana" },
            "head": { "ref": "forja/login" },
            "base": { "ref": "main" },
            "requested_reviewers": [{ "login": "bo" }],
            "labels": [{ "name": "review" }]
        });
        let parsed = pull(&pr, Provider::GitHub).unwrap();
        assert_eq!(parsed.number, 42);
        assert_eq!(parsed.source, "forja/login");
        assert_eq!(parsed.reviewers, vec!["bo"]);
        assert_eq!(parsed.labels, vec!["review"]);

        let checks = serde_json::json!({
            "check_runs": [{
                "id": 7, "name": "build", "status": "completed", "conclusion": "failure",
                "details_url": "https://github.com/acme/app/runs/7",
                "output": { "title": "build failed", "summary": "error: expected 2" }
            }]
        });
        let parsed = checks_for_test(&checks);
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].failed());
        assert!(parsed[0].log.contains("expected 2"));
    }

    fn checks_for_test(value: &Value) -> Vec<Check> {
        checks(value, Provider::GitHub)
    }

    #[test]
    fn gitlab_pull_and_threads_are_parsed() {
        let pr = serde_json::json!({
            "iid": 9, "title": "Draft: Login", "description": "Body", "state": "opened",
            "draft": true, "web_url": "https://gitlab.com/g/a/-/merge_requests/9",
            "author": { "username": "ana" }, "source_branch": "forja/login",
            "target_branch": "main", "reviewers": [{ "username": "bo" }],
            "labels": ["review"]
        });
        let parsed = pull(&pr, Provider::GitLab).unwrap();
        assert_eq!(parsed.number, 9);
        assert!(parsed.draft);
        assert_eq!(parsed.display_state(), "draft");

        let discussions = serde_json::json!([{
            "id": "abc",
            "notes": [{ "id": 1, "body": "Please check", "author": { "username": "bo" },
                        "resolvable": true, "resolved": false,
                        "position": { "new_path": "src/a.rs", "new_line": 12 } }]
        }]);
        let parsed = threads(&discussions, Provider::GitLab);
        assert_eq!(parsed[0].path, "src/a.rs");
        assert_eq!(parsed[0].line, 12);
        assert_eq!(parsed[0].comments[0].author, "bo");
    }
}
