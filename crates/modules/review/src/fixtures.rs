//! Deterministic review data for screenshots and offline demos.

use crate::api::{Provider, Target};
use crate::parse::{Check, Comment, PullRequest, Thread};
use crate::state::Snapshot;

pub fn snapshot() -> Snapshot {
    Snapshot {
        target: Target {
            provider: Provider::GitHub,
            host: "github.com".into(),
            repo: "forja/demo".into(),
            api_base: "https://api.github.com".into(),
        },
        branch: "forja/arrumar-login".into(),
        base: "main".into(),
        pr: Some(PullRequest {
            number: 42,
            title: "Arrumar login com expiração".into(),
            body: "## O que mudou\n\nO login agora expira em uma hora.\n\n## Revisão\n\n- conferir o retry\n- validar o token".into(),
            state: "open".into(),
            draft: false,
            url: "https://github.com/forja/demo/pull/42".into(),
            author: "ana".into(),
            source: "forja/arrumar-login".into(),
            target: "main".into(),
            mergeable: "clean".into(),
            merge_state: "blocked".into(),
            reviewers: vec!["bo".into(), "caio".into()],
            labels: vec!["review".into(), "login".into()],
        }),
        checks: vec![
            Check {
                id: 101,
                name: "build".into(),
                state: "success".into(),
                url: "https://github.com/forja/demo/actions/runs/101".into(),
                log: "compiled 34 crates".into(),
                started_at: "2026-11-12T10:00:00Z".into(),
                finished_at: "2026-11-12T10:01:20Z".into(),
                pipeline: false,
            },
            Check {
                id: 102,
                name: "test".into(),
                state: "failure".into(),
                url: "https://github.com/forja/demo/actions/runs/102".into(),
                log: "running 34 tests\ntest login::keeps_retry ... FAILED\nerror: expected 2 retries, got 1".into(),
                started_at: "2026-11-12T10:00:00Z".into(),
                finished_at: "2026-11-12T10:02:10Z".into(),
                pipeline: false,
            },
            Check {
                id: 103,
                name: "lint".into(),
                state: "in_progress".into(),
                url: "https://github.com/forja/demo/actions/runs/103".into(),
                log: String::new(),
                started_at: "2026-11-12T10:00:00Z".into(),
                finished_at: String::new(),
                pipeline: false,
            },
        ],
        threads: vec![Thread {
            id: "thread-login".into(),
            path: "src/login.ts".into(),
            line: 5,
            resolved: false,
            comments: vec![
                Comment {
                    id: "1".into(),
                    author: "bo".into(),
                    body: "O retry precisa de um teste de borda.".into(),
                },
                Comment {
                    id: "2".into(),
                    author: "ana".into(),
                    body: "Vou cobrir no próximo commit.".into(),
                },
            ],
        }],
    }
}
