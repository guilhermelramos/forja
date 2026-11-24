//! Testes do wrapper humanizado do diff.

use super::humanize_error;
use forja_kernel::{Runtime, RuntimeOptions};
use std::path::Path;

const RAW_GIT_ERROR: &str = "git rev-parse --verify --end-of-options C:\\ghost\\999^{commit} failed: fatal: Needed a single revision";
const HUMAN_PHRASE: &str = "A branch base ou referência informada não existe neste repositório.";

fn test_runtime(root: &Path) -> Runtime {
    Runtime::new(
        vec![],
        RuntimeOptions {
            data_dir: Some(root.join("data")),
            user_dir: Some(root.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("runtime")
}

#[test]
fn git_rev_parse_error_becomes_human_phrase() {
    let root = std::env::temp_dir().join(format!("forja-diff-human-{}", std::process::id()));
    let mut runtime = test_runtime(&root);
    let cx = runtime.ctx("diff");
    let human = humanize_error(&cx, RAW_GIT_ERROR);
    assert_eq!(human.summary, HUMAN_PHRASE);
    assert!(!human.summary.contains("rev-parse"));
    assert!(!human.summary.to_ascii_lowercase().contains("failed"));
    assert!(human.detail.is_some());
    let _ = std::fs::remove_dir_all(&root);
}
