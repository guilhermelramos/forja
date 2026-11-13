//! Transcrição pela OpenAI: envia o WAV ao endpoint de transcrição e lê o campo `text`. Só esse
//! host recebe o áudio e a chave. Os erros voltam como chave de tradução, sem o conteúdo do áudio
//! e sem a chave.

use crate::multipart::Multipart;
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const ENDPOINT: &str = "https://api.openai.com/v1/audio/transcriptions";
/// Modelo de transcrição. Confirmado em fonte secundária; a documentação oficial da OpenAI não
/// estava acessível na hora da escrita (ver o relatório).
pub const MODEL: &str = "gpt-4o-mini-transcribe";
const TOTAL_TIMEOUT: Duration = Duration::from_secs(120);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// Transcreve um WAV respeitando a política JSON retornada por `network.proxy`.
/// `language` é o código ISO-639-1 (`pt`, `en`) ou `None` para detectar.
pub fn transcribe_with_policy(
    key: &str,
    wav: &[u8],
    language: Option<&str>,
    policy: Option<&Value>,
) -> Result<String, &'static str> {
    if policy
        .and_then(|value| value.get("offline"))
        .and_then(Value::as_bool)
        == Some(true)
    {
        return Err("voice-error-network");
    }

    let mut form = Multipart::new(&boundary()).text("model", MODEL);
    if let Some(language) = language {
        form = form.text("language", language);
    }
    let (content_type, body) = form.file("file", "voz.wav", "audio/wav", wav).finish();

    let timeout = policy
        .and_then(|value| value.get("timeout_s"))
        .and_then(Value::as_u64)
        .map(|seconds| Duration::from_secs(seconds.clamp(5, 120)))
        .unwrap_or(TOTAL_TIMEOUT);
    let tls_verify = policy
        .and_then(|value| value.get("tls_verify"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let tls = ureq::tls::TlsConfig::builder()
        .disable_verification(!tls_verify)
        .build();
    let mut config_builder = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .timeout_connect(Some(CONNECT_TIMEOUT.min(timeout)))
        .http_status_as_error(false)
        .tls_config(tls);

    if let Some(proxy_url) = resolved_proxy_url(policy) {
        if let Ok(proxy) = ureq::Proxy::new(proxy_url) {
            config_builder = config_builder.proxy(Some(proxy));
        }
    }

    let agent = ureq::Agent::new_with_config(config_builder.build());
    let response = agent
        .post(ENDPOINT)
        .header("Authorization", &format!("Bearer {key}"))
        .header("Content-Type", &content_type)
        .send(&body[..])
        .map_err(network_error)?;
    let status = response.status().as_u16();
    let text = response
        .into_body()
        .read_to_string()
        .map_err(network_error)?;
    read_response(status, &text)
}

fn resolved_proxy_url(policy: Option<&Value>) -> Option<&str> {
    let policy = policy?;
    if policy.get("mode").and_then(Value::as_str) == Some("none") {
        return None;
    }
    let url = policy.get("url").and_then(Value::as_str)?.trim();
    if url.is_empty() {
        return None;
    }
    let bypassed = policy
        .get("bypass")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().filter_map(Value::as_str).any(|entry| {
                let entry = entry.trim().to_ascii_lowercase();
                entry == "api.openai.com"
                    || entry
                        .strip_prefix("*.")
                        .is_some_and(|suffix| "api.openai.com".ends_with(&format!(".{suffix}")))
            })
        });
    (!bypassed).then_some(url)
}

/// Interpreta a resposta: texto do campo `text` em 2xx, ou a chave do erro.
pub fn read_response(status: u16, text: &str) -> Result<String, &'static str> {
    match status {
        200..=299 => parse_text(text),
        401 | 403 => Err("voice-error-auth"),
        _ => Err("voice-error-http"),
    }
}

/// Código de idioma para o campo `language`. `auto` (ou qualquer valor desconhecido) detecta.
pub fn language_code(setting: &str) -> Option<&'static str> {
    match setting {
        "pt_BR" => Some("pt"),
        "en" => Some("en"),
        _ => None,
    }
}

fn parse_text(text: &str) -> Result<String, &'static str> {
    let value: Value = serde_json::from_str(text).map_err(|_| "voice-error-response")?;
    value
        .get("text")
        .and_then(Value::as_str)
        .map(|text| text.trim().to_owned())
        .ok_or("voice-error-response")
}

fn network_error(error: ureq::Error) -> &'static str {
    match error {
        ureq::Error::Timeout(_) => "voice-error-timeout",
        _ => "voice-error-network",
    }
}

/// Limite único para o corpo: um número que não aparece no áudio com chance prática.
fn boundary() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("forja-{nanos:x}")
}

#[cfg(test)]
mod tests;
