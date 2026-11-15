//! Converts low-level OS, I/O, process and Git errors into clear localized
//! messages, keeping the raw technical text only as an optional detail.

use crate::error::KernelError;
use crate::i18n::I18n;
use crate::runtime::Ctx;
use std::io;

/// User-facing error message paired with optional raw technical context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HumanError {
    pub summary: String,
    pub detail: Option<String>,
}

impl HumanError {
    /// Creates a message with no technical detail attached.
    pub fn simple(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            detail: None,
        }
    }

    /// Creates a localized summary paired with raw technical detail.
    pub fn with_detail(summary: impl Into<String>, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        let trimmed = detail.trim();
        Self {
            summary: summary.into(),
            detail: (!trimmed.is_empty()).then(|| trimmed.to_owned()),
        }
    }
}

/// Converts a raw error string using the runtime's active locale.
pub fn humanize_error(i18n: &I18n, raw: &str) -> HumanError {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return HumanError::simple(i18n.t("kernel.error.generic"));
    }
    let normalized = strip_wrappers(trimmed);
    let lower = normalized.to_ascii_lowercase();
    let full_lower = trimmed.to_ascii_lowercase();

    if let Some(key) = classify_key(&full_lower, &lower) {
        return HumanError::with_detail(i18n.t(key), trimmed);
    }

    if looks_technical(normalized, &lower) {
        return HumanError::with_detail(i18n.t("kernel.error.generic"), trimmed);
    }

    if normalized != trimmed {
        return HumanError::with_detail(normalized, trimmed);
    }

    HumanError::simple(trimmed)
}

/// Convenience wrapper for module views and command handlers.
pub fn humanize_ctx(cx: &Ctx<'_>, raw: &str) -> HumanError {
    humanize_error(cx.i18n(), raw)
}

/// Converts a [`std::io::Error`] into a localized [`HumanError`].
pub fn humanize_io_error(i18n: &I18n, error: &io::Error) -> HumanError {
    let raw = error.to_string();
    let key = match error.kind() {
        io::ErrorKind::NotFound => "kernel.error.not-found",
        io::ErrorKind::PermissionDenied => "kernel.error.permission",
        io::ErrorKind::AlreadyExists => "kernel.error.already-exists",
        _ => return humanize_error(i18n, &raw),
    };
    HumanError::with_detail(i18n.t(key), raw)
}

/// Converts a [`KernelError`] without exposing internal command wrappers.
pub fn humanize_kernel_error(i18n: &I18n, error: &KernelError) -> HumanError {
    match error {
        KernelError::Io(io_err) => humanize_io_error(i18n, io_err),
        KernelError::Command { message, .. } => humanize_error(i18n, message),
        KernelError::UnknownCommand(_) => HumanError::with_detail(
            i18n.t("kernel.error.generic"),
            error.to_string(),
        ),
        other => humanize_error(i18n, &other.to_string()),
    }
}

fn strip_wrappers(mut text: &str) -> &str {
    loop {
        let before = text;
        if let Some(rest) = text.strip_prefix("command `") {
            if let Some((_, after)) = rest.split_once("` failed:") {
                text = after.trim();
            } else if let Some((_, after)) = rest.split_once("` falhou:") {
                text = after.trim();
            }
        }
        for prefix in [
            "An IO error occurred:",
            "i/o error:",
            "could not run git init:",
            "could not run git clone:",
            "could not run git:",
            "git is not available:",
        ] {
            if let Some(rest) = text.strip_prefix(prefix) {
                text = rest.trim();
            }
        }
        if text == before {
            break;
        }
    }
    text.trim_matches('\'')
}

fn classify_key(full_lower: &str, inner_lower: &str) -> Option<&'static str> {
    if is_invalid_directory(inner_lower) || is_invalid_directory(full_lower) {
        return Some("kernel.error.invalid-directory");
    }
    if is_program_not_found(full_lower, inner_lower) {
        return Some("kernel.error.program-not-found");
    }
    if is_permission_error(inner_lower) {
        return Some("kernel.error.permission");
    }
    if is_path_name_error(inner_lower) {
        return Some("kernel.error.invalid-name");
    }
    if is_already_exists(inner_lower) {
        return Some("kernel.error.already-exists");
    }
    if is_disk_full(inner_lower) {
        return Some("kernel.error.disk-full");
    }
    if is_not_found(inner_lower) {
        return Some("kernel.error.not-found");
    }
    if let Some(git_key) = classify_git_key(full_lower, inner_lower) {
        return Some(git_key);
    }
    if let Some(helper_key) = classify_helper_key(inner_lower) {
        return Some(helper_key);
    }
    if is_network_error(inner_lower) {
        return Some("kernel.error.network");
    }
    None
}

fn classify_helper_key(inner_lower: &str) -> Option<&'static str> {
    if inner_lower.contains("no installed helper profile matches")
        || inner_lower.contains("no installed helpers found")
        || inner_lower.contains("nenhum ajudante instalado foi encontrado")
    {
        return Some("kernel.error.helper-not-installed");
    }
    if inner_lower.contains("helper profile") && inner_lower.contains("is disabled") {
        return Some("kernel.error.helper-disabled");
    }
    if inner_lower.contains("helper profile") && inner_lower.contains("not trusted") {
        return Some("kernel.error.helper-untrusted");
    }
    if inner_lower.contains("terminal module is not available")
        || inner_lower.contains("no terminal is available")
        || inner_lower.contains("returned no terminal_id")
    {
        return Some("kernel.error.terminal-unavailable");
    }
    None
}

fn is_invalid_directory(lower: &str) -> bool {
    lower.contains("os error 267")
        || lower.contains("directory name is invalid")
        || lower.contains("nome do diretório é inválido")
        || lower.contains("nome do diretorio e invalido")
}

fn is_program_not_found(full_lower: &str, inner_lower: &str) -> bool {
    let mentions_exec = full_lower.contains("could not run ")
        || full_lower.contains("git is not available")
        || full_lower.contains("failed to execute")
        || full_lower.contains("git failed to start");
    let missing = inner_lower.contains("os error 2")
        || inner_lower.contains("cannot find the file")
        || inner_lower.contains("não pode encontrar o arquivo")
        || inner_lower.contains("program not found")
        || inner_lower.contains("command not found")
        || inner_lower.contains("not found in path");
    (mentions_exec && missing) || inner_lower.contains("program not found")
}

fn is_permission_error(lower: &str) -> bool {
    lower.contains("os error 5")
        || lower.contains("os error 32")
        || lower.contains("access is denied")
        || lower.contains("acesso negado")
        || lower.contains("permission denied")
        || lower.contains("used by another process")
        || lower.contains("sharing violation")
}

fn is_path_name_error(lower: &str) -> bool {
    lower.contains("os error 206")
        || lower.contains("os error 123")
        || lower.contains("filename or extension is too long")
        || lower.contains("file name too long")
        || lower.contains("cannot lock ref")
        || lower.contains("not a valid branch name")
        || lower.contains("syntax is incorrect")
}

fn is_already_exists(lower: &str) -> bool {
    lower.contains("os error 183")
        || lower.contains("already exists")
        || lower.contains("já existe")
}

fn is_disk_full(lower: &str) -> bool {
    lower.contains("os error 112")
        || lower.contains("no space left")
        || lower.contains("disk is full")
}

fn is_not_found(lower: &str) -> bool {
    lower.contains("os error 2")
        || lower.contains("os error 3")
        || lower.contains("cannot find the file")
        || lower.contains("cannot find the path")
        || lower.contains("no such file or directory")
        || lower.contains("não pode encontrar o arquivo")
        || lower.contains("não foi possível encontrar o caminho")
}

fn classify_git_key(full_lower: &str, inner_lower: &str) -> Option<&'static str> {
    if inner_lower.contains("not a git repository") {
        return Some("kernel.error.git-not-repo");
    }
    if inner_lower.contains("invalid reference:")
        || (inner_lower.contains("base branch") && inner_lower.contains("does not exist"))
        || inner_lower.contains("unknown revision")
        || inner_lower.contains("not a valid object name")
    {
        return Some("kernel.error.git-invalid-ref");
    }
    if inner_lower.contains("already checked out")
        || inner_lower.contains("is already used by worktree")
        || inner_lower.contains("used by worktree")
    {
        return Some("kernel.error.git-worktree-in-use");
    }
    if inner_lower.contains("nothing to commit")
        || inner_lower.contains("no changes added to commit")
    {
        return Some("kernel.error.git-nothing-to-commit");
    }
    if inner_lower.contains("conflict") || inner_lower.contains("unmerged files") {
        return Some("kernel.error.git-conflict");
    }
    if inner_lower.contains("rejected")
        || inner_lower.contains("failed to push")
        || inner_lower.contains("non-fast-forward")
    {
        return Some("kernel.error.git-push-rejected");
    }
    if inner_lower.contains("no upstream branch")
        || inner_lower.contains("has no upstream")
        || inner_lower.contains("no tracking information")
    {
        return Some("kernel.error.git-no-upstream");
    }
    if inner_lower.contains("authentication failed")
        || inner_lower.contains("could not read username")
        || inner_lower.contains("terminal prompts disabled")
    {
        return Some("kernel.error.git-auth");
    }
    if inner_lower.contains("contains modified or untracked files")
        || inner_lower.contains("uncommitted changes")
    {
        return Some("kernel.error.git-dirty");
    }
    if full_lower.starts_with("git ")
        || full_lower.contains("could not run git")
        || full_lower.contains("git is not available")
        || full_lower.contains("git command failed")
        || inner_lower.contains("preparing worktree")
        || inner_lower.contains("fatal:")
    {
        return Some("kernel.error.git-generic");
    }
    None
}

fn is_network_error(lower: &str) -> bool {
    lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("connection refused")
        || lower.contains("failed to connect")
        || lower.contains("dns")
}

fn looks_technical(raw: &str, lower: &str) -> bool {
    lower.contains("os error")
        || has_ascii_word(lower, "failed")
        || lower.contains("error:")
        || lower.contains("fatal:")
        || lower.contains("could not run ")
        || lower.contains("an io error")
        || lower.contains("exited with")
        || lower.contains("invalid setting")
        || raw.contains("{\"")
        || raw.contains(r"\\?\")
        || has_backtick_internal_id(raw)
}

fn has_ascii_word(haystack: &str, word: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut start = 0;
    while let Some(offset) = haystack[start..].find(word) {
        let idx = start + offset;
        let end = idx + word.len();
        let left_ok = idx == 0 || !bytes[idx - 1].is_ascii_alphanumeric();
        let right_ok = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
        if left_ok && right_ok {
            return true;
        }
        start = idx + 1;
    }
    false
}

fn has_backtick_internal_id(text: &str) -> bool {
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else {
            break;
        };
        let inner = &after[..close];
        if inner.len() >= 3
            && inner.len() <= 64
            && !inner.contains(char::is_whitespace)
            && (inner.contains('.') || inner.contains('_'))
            && inner
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':'))
        {
            return true;
        }
        rest = &after[close + 1..];
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{BundleSource, Locale};

    fn test_i18n(locale: Locale) -> I18n {
        let bundle = BundleSource {
            module: "kernel".to_owned(),
            en: include_str!("../i18n/en.ftl").to_owned(),
            pt_br: include_str!("../i18n/pt-BR.ftl").to_owned(),
        };
        let (i18n, warnings) = I18n::build(&[bundle], locale);
        assert!(warnings.is_empty(), "{warnings:?}");
        i18n
    }

    #[test]
    fn converts_os_and_io_errors_in_pt_br_and_en() {
        let pt = test_i18n(Locale::PtBr);
        let en = test_i18n(Locale::En);

        let dir_err = humanize_error(
            &pt,
            "could not run git: The directory name is invalid. (os error 267)",
        );
        assert_eq!(dir_err.summary, "O caminho da pasta é inválido ou não existe mais.");
        assert!(dir_err.detail.as_deref().unwrap().contains("os error 267"));

        let perm_err = humanize_error(&en, "An IO error occurred: 'Access is denied. (os error 5)'");
        assert_eq!(
            perm_err.summary,
            "Permission was denied or the file is in use by another process."
        );
        assert!(!perm_err.summary.to_ascii_lowercase().contains("os error"));
        assert!(perm_err.detail.as_deref().unwrap().contains("os error 5"));

        let io_not_found = humanize_io_error(
            &pt,
            &io::Error::new(io::ErrorKind::NotFound, "missing.txt (os error 2)"),
        );
        assert_eq!(io_not_found.summary, "O arquivo ou a pasta não foi encontrado.");
        assert!(io_not_found.detail.is_some());
    }

    #[test]
    fn converts_process_not_found_and_git_errors_without_raw_terms() {
        let pt = test_i18n(Locale::PtBr);
        let en = test_i18n(Locale::En);

        let missing_git = humanize_error(
            &pt,
            "could not run git: program not found (os error 2)",
        );
        assert!(missing_git.summary.contains("programa necessário"), "{}", missing_git.summary);

        let invalid_ref = humanize_error(
            &en,
            "git worktree add -b forja/long -- C:\\tmp master failed: fatal: invalid reference: master",
        );
        assert_eq!(
            invalid_ref.summary,
            "The selected base branch or reference does not exist in this repository."
        );
        assert!(!invalid_ref.summary.to_ascii_lowercase().contains("failed"));

        let preparing = humanize_error(
            &en,
            "git worktree add -b forja/very-long -- path main failed: Preparing worktree (new branch 'forja/very-long')",
        );
        assert_eq!(preparing.summary, "Could not complete the Git operation.");
        assert!(!preparing.summary.to_ascii_lowercase().contains("failed"));

        let cmd_err = humanize_kernel_error(
            &pt,
            &KernelError::Command {
                id: "review.refresh".to_owned(),
                message: "git command failed".to_owned(),
            },
        );
        assert_eq!(cmd_err.summary, "Não foi possível concluir a operação do Git.");
        assert!(!cmd_err.summary.contains("review.refresh"));
    }

    #[test]
    fn preserves_clean_localized_messages_without_extra_detail() {
        let pt = test_i18n(Locale::PtBr);
        let clean = humanize_error(&pt, "Informe um nome para a bancada.");
        assert_eq!(clean.summary, "Informe um nome para a bancada.");
        assert_eq!(clean.detail, None);
    }

    #[test]
    fn converts_helper_launch_and_wrapped_content_errors_without_internal_ids() {
        let pt = test_i18n(Locale::PtBr);
        let en = test_i18n(Locale::En);

        let wrapped_pt = humanize_error(
            &pt,
            "command `content.run` falhou: command `helper.launch` failed: no installed helper profile matches helper.default.profile",
        );
        assert!(
            wrapped_pt.summary.contains("Nenhum ajudante instalado"),
            "{}",
            wrapped_pt.summary
        );
        assert!(!wrapped_pt.summary.contains('`'));
        assert!(!wrapped_pt.summary.contains("helper.default.profile"));
        assert!(wrapped_pt.detail.is_some());

        let wrapped_en = humanize_error(
            &en,
            "command `helper.launch` failed: No installed helpers found on PATH.",
        );
        assert!(
            wrapped_en.summary.contains("No installed helper was found on PATH"),
            "{}",
            wrapped_en.summary
        );
        assert!(!wrapped_en.summary.contains("helper.launch"));
        assert!(!wrapped_en.summary.to_ascii_lowercase().contains("failed"));
        assert!(wrapped_en.detail.is_some());

        let clean_wrapped = humanize_error(
            &pt,
            "command `helper.launch` failed: O ajudante Claude Code não está instalado neste computador.",
        );
        assert_eq!(
            clean_wrapped.summary,
            "O ajudante Claude Code não está instalado neste computador."
        );
        assert!(clean_wrapped.detail.is_some());

        let worktree_delete = humanize_error(
            &pt,
            "git branch -D main failed: error: cannot delete branch 'main' used by worktree at 'C:/repo'",
        );
        assert_eq!(
            worktree_delete.summary,
            "Esta branch já está em uso em outra bancada."
        );
        assert!(worktree_delete.detail.is_some());
    }
}
