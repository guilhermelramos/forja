//! Classificações de falhas do ajudante e do provedor, chamadas pelo
//! classificador principal de [`crate::human_error`].

use crate::human_error::{has_backtick_internal_id, strip_wrappers};

/// Classifica falhas ligadas ao lançamento do ajudante e ao provedor.
pub(crate) fn classify_launch_key(full_lower: &str, inner_lower: &str) -> Option<&'static str> {
    if is_provider_failure_inner(full_lower, inner_lower) {
        return Some("kernel.error.provider-unavailable");
    }
    if is_helper_executable_missing_inner(full_lower, inner_lower) {
        return Some("kernel.error.helper-executable-missing");
    }
    None
}

/// Verdadeiro quando o texto é uma falha de comando interno sem causa mais
/// específica (por exemplo, um wrapper `command \`...\` failed` genérico).
pub fn is_internal_command_failure(raw: &str) -> bool {
    let trimmed = raw.trim();
    let normalized = strip_wrappers(trimmed);
    is_internal_command_failure_inner(
        &trimmed.to_ascii_lowercase(),
        &normalized.to_ascii_lowercase(),
    )
}

pub(crate) fn is_internal_command_failure_inner(full_lower: &str, inner_lower: &str) -> bool {
    let wrapped = (full_lower.starts_with("command `")
        && (full_lower.contains("` failed:") || full_lower.contains("` falhou:")))
        || (full_lower.contains("command `") && inner_lower.contains("` failed:"));
    let technical = inner_lower.contains("os error")
        || inner_lower.contains("error:")
        || inner_lower.contains("failed")
        || inner_lower.contains("falhou")
        || has_backtick_internal_id(inner_lower);
    (wrapped && technical) || has_backtick_internal_id(inner_lower)
}

/// Prefixos de ids internos: só contam quando seguidos de alfanuméricos.
const INTERNAL_ID_PREFIXES: [&str; 2] = ["ses_", "ctx_"];

/// Módulos com comandos internos conhecidos, usados no formato `modulo.acao`.
const KNOWN_COMMAND_MODULES: &[&str] = &[
    "accounts", "archive", "bench", "benches", "board", "browser", "chat", "content", "counter",
    "diff", "editor", "explorer", "extension", "file", "find", "git", "hello", "helper", "hook",
    "import", "integrations", "kernel", "mcp", "muster", "network", "notify", "onboarding",
    "ports", "privacy", "project", "projects", "remote", "request", "review", "routine", "search",
    "shell", "skill", "terminal", "test", "trust", "update", "usage", "viewers", "voice",
];

/// Verdadeiro só para o formato exato de id interno: prefixo + alfanuméricos.
pub(crate) fn is_exact_internal_id(text: &str) -> bool {
    INTERNAL_ID_PREFIXES.iter().any(|prefix| {
        text.strip_prefix(prefix).is_some_and(|rest| {
            !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
    })
}

fn is_command_segment(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Nome de comando interno no formato `modulo.acao`, com módulo conhecido.
pub(crate) fn is_known_internal_command(text: &str) -> bool {
    if text.len() < 3 || text.len() > 64 || !text.is_ascii() || text.contains(char::is_whitespace) {
        return false;
    }
    let mut segments = text.split('.');
    let module = segments.next().unwrap_or("");
    let action: Vec<&str> = segments.collect();
    !action.is_empty()
        && is_command_segment(module)
        && action.iter().all(|part| is_command_segment(part))
        && KNOWN_COMMAND_MODULES
            .iter()
            .any(|known| known.eq_ignore_ascii_case(module))
}

/// Reduz ids internos (`ses_…`, `ctx_…`) e nomes de comando conhecidos
/// (`modulo.acao`) na frase principal. Texto legítimo entre crases fica intacto.
pub fn redact_internal_ids(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        let mut hit: Option<(usize, usize)> = None;

        for prefix in INTERNAL_ID_PREFIXES {
            if let Some(offset) = rest.find(prefix) {
                let after = offset + prefix.len();
                let end = after
                    + rest[after..]
                        .bytes()
                        .take_while(|byte| byte.is_ascii_alphanumeric())
                        .count();
                if end > after && hit.is_none_or(|(earliest, _)| offset < earliest) {
                    hit = Some((offset, end));
                }
            }
        }

        if let Some(open) = rest.find('`') {
            if hit.is_none_or(|(earliest, _)| open < earliest) {
                if let Some(close) = rest[open + 1..].find('`') {
                    let inner = &rest[open + 1..open + 1 + close];
                    if is_known_internal_command(inner) {
                        hit = Some((open, open + close + 2));
                    }
                }
            }
        }

        let Some((offset, end)) = hit else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..offset]);
        out.push_str(if rest[offset..].starts_with('`') {
            "[interno]"
        } else {
            "[id]"
        });
        index += end;
    }
    out
}

/// Verdadeiro quando o texto descreve uma falha de saldo, credencial ou limite
/// do provedor do ajudante (inclusive quando só o id interno da sessão aparece).
pub fn is_provider_failure(raw: &str) -> bool {
    let trimmed = raw.trim();
    let normalized = strip_wrappers(trimmed);
    is_provider_failure_inner(&trimmed.to_ascii_lowercase(), &normalized.to_ascii_lowercase())
        || trimmed.to_ascii_lowercase().contains("ses_")
}

fn is_provider_failure_inner(_full_lower: &str, inner_lower: &str) -> bool {
    [
        "insufficient balance",
        "insufficient_quota",
        "insufficient quota",
        "quota exceeded",
        "rate limit",
        "pay-as-you-go",
        "billing",
        "invalid api key",
        "bad credentials",
        "sem saldo",
        "saldo insuficiente",
    ]
    .iter()
    .any(|pattern| inner_lower.contains(pattern))
}

/// Verdadeiro quando o texto descreve um executável de ajudante ausente.
pub fn is_helper_executable_missing(raw: &str) -> bool {
    let trimmed = raw.trim();
    let normalized = strip_wrappers(trimmed);
    is_helper_executable_missing_inner(
        &trimmed.to_ascii_lowercase(),
        &normalized.to_ascii_lowercase(),
    )
}

fn is_helper_executable_missing_inner(full_lower: &str, inner_lower: &str) -> bool {
    inner_lower.contains("executable-not-found:")
        || inner_lower.contains("not installed on this machine")
        || (full_lower.contains("helper.launch")
            && (inner_lower.contains("os error 2")
                || inner_lower.contains("cannot find the file")
                || inner_lower.contains("não pode encontrar o arquivo")
                || inner_lower.contains("no such file")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::human_error::humanize_error;
    use crate::i18n::{BundleSource, I18n, Locale};

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
    fn missing_helper_executable_gets_human_phrase_without_raw_os_error() {
        let pt = test_i18n(Locale::PtBr);
        let en = test_i18n(Locale::En);

        let missing = humanize_error(&pt, "executable-not-found:OpenCode");
        assert_eq!(
            missing.summary,
            "Não achei o programa do ajudante. Confira a instalação em Ajudantes."
        );
        assert!(!missing.summary.contains("os error"));
        assert!(!missing.summary.contains("executable-not-found:"));
        assert_eq!(missing.detail.as_deref(), Some("executable-not-found:OpenCode"));

        let wrapped = humanize_error(
            &en,
            "command `helper.launch` failed: The system cannot find the file specified. (os error 2)",
        );
        assert!(wrapped.summary.contains("Could not find the helper's program"), "{}", wrapped.summary);
        assert!(!wrapped.summary.to_ascii_lowercase().contains("os error"));
        assert!(!wrapped.summary.contains('`'));
        assert!(wrapped.detail.is_some());
        assert!(is_helper_executable_missing("executable-not-found:X"));
    }

    #[test]
    fn provider_failure_gets_localized_guidance_without_internal_session_id() {
        let pt = test_i18n(Locale::PtBr);
        let en = test_i18n(Locale::En);

        let balance = humanize_error(
            &pt,
            "Insufficient balance. This request is pay-as-you-go, billed to ses_e1ef4970cffeQh6MI0sy42sh5e",
        );
        assert!(balance.summary.contains("saldo"), "{}", balance.summary);
        assert!(balance.summary.contains("credencial"), "{}", balance.summary);
        assert!(!balance.summary.contains("ses_"));
        assert!(!balance.summary.contains("Insufficient balance"));

        let limit = humanize_error(&en, "rate limit exceeded for the current model");
        assert!(limit.summary.contains("plan limits"), "{}", limit.summary);
        assert!(!limit.summary.to_ascii_lowercase().contains("rate limit"));
        // Só texto específico do provedor conta; "credential" solto não.
        assert!(is_provider_failure("invalid api key"));
        assert!(is_provider_failure("bad credentials"));
        assert!(!is_provider_failure("credential is invalid"));
    }

    #[test]
    fn internal_command_classifier_does_not_steal_specific_causes() {
        let en = test_i18n(Locale::En);

        let auth = humanize_error(&en, "command `git.fetch` failed: fatal: authentication failed");
        assert_eq!(
            auth.summary,
            "Authentication was refused while accessing the remote repository."
        );
        assert!(auth.detail.as_deref().unwrap().contains("git.fetch"));

        let perm = humanize_error(&en, "command `x` failed: Access is denied. (os error 5)");
        assert_eq!(
            perm.summary,
            "Permission was denied or the file is in use by another process."
        );
        assert!(perm.detail.as_deref().unwrap().contains("os error 5"));
    }

    #[test]
    fn internal_command_failure_hides_command_name_and_ids() {
        let pt = test_i18n(Locale::PtBr);
        let en = test_i18n(Locale::En);

        let raw = "command `helper.launch` failed: ctx_ab12 internal step failed";
        let human = humanize_error(&pt, raw);
        assert_eq!(
            human.summary,
            "Não foi possível concluir uma ação interna do aplicativo. Tente de novo; se continuar, confira o log."
        );
        assert!(!human.summary.contains("helper.launch"));
        assert!(!human.summary.contains("ctx_"));
        // Os detalhes são recolhidos e mostram o texto original para diagnóstico.
        let detail = human.detail.as_deref().unwrap();
        assert!(detail.contains("helper.launch"), "{detail}");
        assert!(detail.contains("ctx_ab12"), "{detail}");

        let clean = humanize_error(&en, "command `content.run` falhou: error: unknown step");
        assert!(clean.summary.contains("Could not complete an internal application action"));
        assert!(!clean.summary.to_ascii_lowercase().contains("failed"));
        assert!(is_internal_command_failure(raw));
    }

    #[test]
    fn summary_redaction_keeps_user_backtick_text() {
        let pt = test_i18n(Locale::PtBr);
        let raw = "Não achei `meu_arquivo` nem `config.toml` para ctx_abc123";
        let human = humanize_error(&pt, raw);
        assert!(human.summary.contains("`meu_arquivo`"), "{}", human.summary);
        assert!(human.summary.contains("`config.toml`"), "{}", human.summary);
        assert!(!human.summary.contains("ctx_abc123"), "{}", human.summary);
        assert!(human.summary.contains("[id]"), "{}", human.summary);

        let redacted = redact_internal_ids("rodei `helper.launch` e `meu_arquivo` com ctx_ab12");
        assert_eq!(redacted, "rodei [interno] e `meu_arquivo` com [id]");
    }
}
