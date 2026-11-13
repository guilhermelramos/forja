//! Review providers: origin detection and REST/GraphQL request builders.
//!
//! GitHub and GitLab share the same tab; only the request shapes differ. Token
//! lookup and transport live in `client.rs`, so these functions stay pure and
//! testable with fixtures.

use forja_counter_kit::{check_host, parse_remote, read_origin, HttpRequest, RemoteRepo};

use serde_json::json;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    GitHub,
    GitLab,
}

impl Provider {
    pub fn id(self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::GitLab => "gitlab",
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Self::GitHub => "review.provider.github",
            Self::GitLab => "review.provider.gitlab",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub provider: Provider,
    pub host: String,
    /// `owner/repo` on GitHub and the full group path on GitLab.
    pub repo: String,
    pub api_base: String,
}

impl Target {
    pub fn project_segment(&self) -> String {
        forja_counter_kit::url::encode_component(&self.repo)
    }
}

/// Resolves the provider from `origin` and the configured counter hosts.
pub fn detect(bench: &Path, github_host: &str, gitlab_host: &str) -> Result<Target, String> {
    let origin = read_origin(bench).map_err(|error| error.to_string())?;
    let remote = parse_remote(&origin).ok_or_else(|| {
        "origin is not a GitHub/GitLab remote with owner and repository".to_owned()
    })?;
    target_from_remote(&remote, github_host, gitlab_host)
}

pub fn target_from_remote(
    remote: &RemoteRepo,
    github_host: &str,
    gitlab_host: &str,
) -> Result<Target, String> {
    let host = remote.host.as_str();
    let github = host.eq_ignore_ascii_case(github_host.trim())
        || (!gitlab_host.trim().is_empty()
            && !host.eq_ignore_ascii_case(gitlab_host.trim())
            && host.contains("github"));
    let provider = if github {
        Provider::GitHub
    } else if host.contains("gitlab") || host.eq_ignore_ascii_case(gitlab_host.trim()) {
        Provider::GitLab
    } else {
        return Err(format!(
            "origin points to {host}, which is not the configured GitHub or GitLab host"
        ));
    };
    if provider == Provider::GitHub {
        check_host(host, github_host).map_err(|error| error.to_string())?;
    } else {
        check_host(host, gitlab_host).map_err(|error| error.to_string())?;
    }
    let repo = if provider == Provider::GitHub {
        let mut parts = remote.path.split('/');
        let owner = parts.next().unwrap_or_default();
        let name = parts.next().unwrap_or_default();
        if owner.is_empty() || name.is_empty() {
            return Err("GitHub origin needs owner/repository".to_owned());
        }
        format!("{owner}/{name}")
    } else {
        remote.path.clone()
    };
    let api_base = match provider {
        Provider::GitHub if host.eq_ignore_ascii_case("github.com") => {
            "https://api.github.com".to_owned()
        }
        Provider::GitHub => format!("https://{host}/api/v3"),
        Provider::GitLab => format!("https://{host}/api/v4"),
    };
    Ok(Target {
        provider,
        host: host.to_owned(),
        repo,
        api_base,
    })
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PullRequestInput {
    pub title: String,
    pub body: String,
    pub source: String,
    pub target: String,
    pub draft: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeOptions {
    pub method: String,
    pub delete_branch: bool,
    pub auto_merge: bool,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            method: "merge".into(),
            delete_branch: false,
            auto_merge: false,
        }
    }
}

pub fn create(t: &Target, input: &PullRequestInput) -> HttpRequest {
    match t.provider {
        Provider::GitHub => HttpRequest::post(
            format!("{}/repos/{}/pulls", t.api_base, t.repo),
            json!({
                "title": input.title,
                "body": input.body,
                "head": input.source,
                "base": input.target,
                "draft": input.draft
            }),
        ),
        Provider::GitLab => {
            let title = if input.draft && !input.title.trim_start().starts_with("Draft:") {
                format!("Draft: {}", input.title)
            } else {
                input.title.clone()
            };
            HttpRequest::post(
                format!(
                    "{}/projects/{}/merge_requests",
                    t.api_base,
                    t.project_segment()
                ),
                json!({
                    "source_branch": input.source,
                    "target_branch": input.target,
                    "title": title,
                    "description": input.body
                }),
            )
        }
    }
}

pub fn find_for_branch(t: &Target, owner: &str, branch: &str) -> HttpRequest {
    match t.provider {
        Provider::GitHub => HttpRequest::get(format!(
            "{}/repos/{}/pulls?state=all&head={}:{}&per_page=10",
            t.api_base,
            t.repo,
            forja_counter_kit::url::encode_component(owner),
            forja_counter_kit::url::encode_component(branch)
        )),
        Provider::GitLab => HttpRequest::get(format!(
            "{}/projects/{}/merge_requests?state=all&source_branch={}&per_page=10",
            t.api_base,
            t.project_segment(),
            forja_counter_kit::url::encode_component(branch)
        )),
    }
}

pub fn detail(t: &Target, number: u64) -> HttpRequest {
    match t.provider {
        Provider::GitHub => {
            HttpRequest::get(format!("{}/repos/{}/pulls/{}", t.api_base, t.repo, number))
        }
        Provider::GitLab => HttpRequest::get(format!(
            "{}/projects/{}/merge_requests/{}",
            t.api_base,
            t.project_segment(),
            number
        )),
    }
}

/// GitHub reads check runs for a ref; GitLab first lists MR pipelines.
pub fn checks(t: &Target, number: u64, git_ref: &str) -> HttpRequest {
    match t.provider {
        Provider::GitHub => HttpRequest::get(format!(
            "{}/repos/{}/commits/{}/check-runs?per_page=100",
            t.api_base,
            t.repo,
            forja_counter_kit::url::encode_component(git_ref)
        )),
        Provider::GitLab => HttpRequest::get(format!(
            "{}/projects/{}/merge_requests/{}/pipelines",
            t.api_base,
            t.project_segment(),
            number
        )),
    }
}

pub fn pipeline_jobs(t: &Target, pipeline: u64) -> HttpRequest {
    HttpRequest::get(format!(
        "{}/projects/{}/pipelines/{}/jobs?per_page=100",
        t.api_base,
        t.project_segment(),
        pipeline
    ))
}

pub fn job_log(t: &Target, job: u64) -> HttpRequest {
    HttpRequest::get(format!(
        "{}/projects/{}/jobs/{}/trace",
        t.api_base,
        t.project_segment(),
        job
    ))
}

pub fn rerun_check(t: &Target, id: u64) -> HttpRequest {
    match t.provider {
        Provider::GitHub => HttpRequest::post(
            format!(
                "{}/repos/{}/check-runs/{}/rerequest",
                t.api_base, t.repo, id
            ),
            json!({}),
        ),
        Provider::GitLab => HttpRequest::post(
            format!(
                "{}/projects/{}/jobs/{}/retry",
                t.api_base,
                t.project_segment(),
                id
            ),
            json!({}),
        ),
    }
}

pub fn request_reviewers(t: &Target, number: u64, reviewers: &[String]) -> HttpRequest {
    match t.provider {
        Provider::GitHub => HttpRequest::post(
            format!(
                "{}/repos/{}/pulls/{}/requested_reviewers",
                t.api_base, t.repo, number
            ),
            json!({ "reviewers": reviewers }),
        ),
        Provider::GitLab => HttpRequest::put(
            format!(
                "{}/projects/{}/merge_requests/{}",
                t.api_base,
                t.project_segment(),
                number
            ),
            json!({ "reviewer_ids": reviewers }),
        ),
    }
}

pub fn add_labels(t: &Target, number: u64, labels: &[String]) -> HttpRequest {
    match t.provider {
        Provider::GitHub => HttpRequest::post(
            format!("{}/repos/{}/issues/{}/labels", t.api_base, t.repo, number),
            json!({ "labels": labels }),
        ),
        Provider::GitLab => HttpRequest::put(
            format!(
                "{}/projects/{}/merge_requests/{}",
                t.api_base,
                t.project_segment(),
                number
            ),
            json!({ "add_labels": labels.join(",") }),
        ),
    }
}

pub fn merge(t: &Target, number: u64, options: &MergeOptions) -> HttpRequest {
    match t.provider {
        Provider::GitHub => HttpRequest::put(
            format!("{}/repos/{}/pulls/{}/merge", t.api_base, t.repo, number),
            json!({
                "merge_method": options.method,
                "delete_branch": options.delete_branch
            }),
        ),
        Provider::GitLab => HttpRequest::put(
            format!(
                "{}/projects/{}/merge_requests/{}/merge",
                t.api_base,
                t.project_segment(),
                number
            ),
            json!({
                "merge_when_pipeline_succeeds": options.auto_merge,
                "should_remove_source_branch": options.delete_branch,
                "squash": options.method == "squash"
            }),
        ),
    }
}

pub fn threads(t: &Target, number: u64) -> HttpRequest {
    match t.provider {
        Provider::GitHub => {
            let query = "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){pullRequest(number:$number){reviewThreads(first:50){nodes{id isResolved path line comments(first:20){nodes{id databaseId body author{login}}}}}}}}";
            let (owner, name) = split_repo(&t.repo);
            graphql(
                t,
                query,
                json!({ "owner": owner, "name": name, "number": number }),
                false,
            )
        }
        Provider::GitLab => HttpRequest::get(format!(
            "{}/projects/{}/merge_requests/{}/discussions?per_page=100",
            t.api_base,
            t.project_segment(),
            number
        )),
    }
}

pub fn reply_thread(t: &Target, number: u64, thread: &str, body: &str) -> HttpRequest {
    match t.provider {
        Provider::GitHub => {
            let query = "mutation($thread:ID!,$body:String!){addPullRequestReviewThreadReply(input:{pullRequestReviewThreadId:$thread,body:$body}){comment{id}}}";
            graphql(t, query, json!({ "thread": thread, "body": body }), false)
        }
        Provider::GitLab => HttpRequest::post(
            format!(
                "{}/projects/{}/merge_requests/{}/discussions/{}/notes",
                t.api_base,
                t.project_segment(),
                number,
                thread
            ),
            json!({ "body": body }),
        ),
    }
}

pub fn resolve_thread(t: &Target, number: u64, thread: &str, resolved: bool) -> HttpRequest {
    match t.provider {
        Provider::GitHub => {
            let query = if resolved {
                "mutation($thread:ID!){resolveReviewThread(input:{threadId:$thread}){thread{isResolved}}}"
            } else {
                "mutation($thread:ID!){unresolveReviewThread(input:{threadId:$thread}){thread{isResolved}}}"
            };
            graphql(t, query, json!({ "thread": thread }), false)
        }
        Provider::GitLab => HttpRequest::put(
            format!(
                "{}/projects/{}/merge_requests/{}/discussions/{}?resolved={}",
                t.api_base,
                t.project_segment(),
                number,
                thread,
                resolved
            ),
            json!({}),
        ),
    }
}

fn graphql(t: &Target, query: &str, variables: serde_json::Value, add_repo: bool) -> HttpRequest {
    let mut variables = variables;
    if add_repo {
        if let (Some(object), (owner, name)) = (variables.as_object_mut(), split_repo(&t.repo)) {
            object.insert("owner".into(), json!(owner));
            object.insert("name".into(), json!(name));
        }
    }
    HttpRequest::post(
        format!("{}/graphql", t.api_base),
        json!({ "query": query, "variables": variables }),
    )
}

fn split_repo(repo: &str) -> (&str, &str) {
    repo.split_once('/').unwrap_or((repo, ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(host: &str, path: &str) -> RemoteRepo {
        RemoteRepo {
            host: host.into(),
            path: path.into(),
        }
    }

    #[test]
    fn github_and_gitlab_targets_use_the_right_api_base() {
        let github = target_from_remote(
            &remote("github.com", "acme/app"),
            "github.com",
            "gitlab.com",
        )
        .unwrap();
        assert_eq!(github.provider, Provider::GitHub);
        assert_eq!(github.api_base, "https://api.github.com");
        assert_eq!(github.repo, "acme/app");

        let gitlab = target_from_remote(
            &remote("gitlab.com", "grupo/sub/app"),
            "github.com",
            "gitlab.com",
        )
        .unwrap();
        assert_eq!(gitlab.provider, Provider::GitLab);
        assert_eq!(gitlab.api_base, "https://gitlab.com/api/v4");
        assert_eq!(gitlab.project_segment(), "grupo%2Fsub%2Fapp");
    }

    #[test]
    fn create_requests_carry_title_body_branch_and_draft() {
        let input = PullRequestInput {
            title: "Add login".into(),
            body: "Body".into(),
            source: "forja/login".into(),
            target: "main".into(),
            draft: true,
        };
        let github = target_from_remote(
            &remote("github.com", "acme/app"),
            "github.com",
            "gitlab.com",
        )
        .unwrap();
        let request = create(&github, &input);
        assert_eq!(request.method, forja_counter_kit::Method::Post);
        assert!(request.url.ends_with("/repos/acme/app/pulls"));
        assert_eq!(request.body.as_ref().unwrap()["draft"], true);
        assert_eq!(request.body.as_ref().unwrap()["head"], "forja/login");

        let gitlab = target_from_remote(
            &remote("gitlab.com", "grupo/app"),
            "github.com",
            "gitlab.com",
        )
        .unwrap();
        let request = create(&gitlab, &input);
        assert!(request
            .url
            .ends_with("/projects/grupo%2Fapp/merge_requests"));
        assert_eq!(request.body.as_ref().unwrap()["title"], "Draft: Add login");
    }

    #[test]
    fn checks_request_targets_the_branch_and_merge_method_is_forwarded() {
        let target = target_from_remote(
            &remote("github.com", "acme/app"),
            "github.com",
            "gitlab.com",
        )
        .unwrap();
        let request = checks(&target, 42, "forja/login");
        assert!(request.url.contains("/commits/forja%2Flogin/check-runs"));
        let merge = merge(
            &target,
            42,
            &MergeOptions {
                method: "squash".into(),
                delete_branch: true,
                auto_merge: false,
            },
        );
        assert_eq!(merge.body.as_ref().unwrap()["merge_method"], "squash");
        assert_eq!(merge.body.as_ref().unwrap()["delete_branch"], true);
    }
}
