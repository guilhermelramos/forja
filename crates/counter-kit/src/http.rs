//! HTTP em forma pequena: requisição, resposta, mapeamento de status e paginação.
//!
//! O trait [`Transport`] separa quem fala HTTP de quem interpreta a resposta. Os testes
//! usam [`crate::FakeTransport`]; a produção usa o transporte instalado no app.

use forja_contracts::CounterError;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

/// Uma requisição pronta para o transporte. Cabeçalhos de autorização são ocultados no `Debug`.
#[derive(Clone, PartialEq)]
pub struct HttpRequest {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Value>,
}

impl HttpRequest {
    fn new(method: Method, url: impl Into<String>, body: Option<Value>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: Vec::new(),
            body,
        }
    }

    pub fn get(url: impl Into<String>) -> Self {
        Self::new(Method::Get, url, None)
    }

    pub fn post(url: impl Into<String>, body: Value) -> Self {
        Self::new(Method::Post, url, Some(body))
    }

    pub fn put(url: impl Into<String>, body: Value) -> Self {
        Self::new(Method::Put, url, Some(body))
    }

    pub fn patch(url: impl Into<String>, body: Value) -> Self {
        Self::new(Method::Patch, url, Some(body))
    }

    pub fn delete(url: impl Into<String>) -> Self {
        Self::new(Method::Delete, url, None)
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn bearer(self, token: &str) -> Self {
        self.header("Authorization", format!("Bearer {token}"))
    }

    /// Autenticação básica, como o Jira usa (`e-mail:token`).
    pub fn basic(self, user: &str, secret: &str) -> Self {
        let encoded = crate::base64::encode(format!("{user}:{secret}").as_bytes());
        self.header("Authorization", format!("Basic {encoded}"))
    }

    pub fn header_value(&self, name: &str) -> Option<&str> {
        header_lookup(&self.headers, name)
    }
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let headers: Vec<(&str, &str)> = self
            .headers
            .iter()
            .map(|(name, value)| {
                let shown = if is_secret_header(name) {
                    "<redacted>"
                } else {
                    value.as_str()
                };
                (name.as_str(), shown)
            })
            .collect();
        f.debug_struct("HttpRequest")
            .field("method", &self.method.as_str())
            .field("url", &self.url)
            .field("headers", &headers)
            .field("body", &self.body)
            .finish()
    }
}

fn is_secret_header(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "authorization" || name.contains("token") || name.contains("key")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpResponse {
    pub fn new(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    pub fn with_headers(mut self, headers: Vec<(&str, &str)>) -> Self {
        self.headers = headers
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect();
        self
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        header_lookup(&self.headers, name)
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Lê o corpo como JSON. Corpo vazio vira `Value::Null`.
    pub fn json(&self) -> Result<Value, CounterError> {
        if self.body.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&self.body).map_err(|error| CounterError::Decode {
            message: error.to_string(),
        })
    }

    pub fn parse<T: DeserializeOwned>(&self) -> Result<T, CounterError> {
        serde_json::from_value(self.json()?).map_err(|error| CounterError::Decode {
            message: error.to_string(),
        })
    }
}

/// Quem envia requisições HTTP. A implementação de produção fica no app; os testes usam
/// [`crate::FakeTransport`]. Erros de rede saem como `CounterError::Network`.
pub trait Transport: Send + Sync {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, CounterError>;

    /// `true` apenas para o transporte HTTP real em produção; `FakeTransport` nos testes devolve `false`.
    fn is_real(&self) -> bool {
        false
    }
}

/// Transporte usado enquanto o cliente HTTP do app não está instalado. Toda chamada falha
/// com `Network`, sem tocar na rede.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnavailableTransport;

impl Transport for UnavailableTransport {
    fn send(&self, _request: &HttpRequest) -> Result<HttpResponse, CounterError> {
        Err(CounterError::Network {
            message: "HTTP client is not installed in this build".to_owned(),
        })
    }
}

/// Converte o status de uma resposta em erro. `None` para 2xx.
///
/// GitHub responde 403 quando o limite de consultas acaba; esse caso vira `RateLimited`
/// porque o cabeçalho `x-ratelimit-remaining` vem zerado.
pub fn error_for_status(
    status: u16,
    headers: &[(String, String)],
    body: &str,
) -> Option<CounterError> {
    if (200..300).contains(&status) {
        return None;
    }
    let error = match status {
        401 => CounterError::Unauthorized,
        403 if header_lookup(headers, "x-ratelimit-remaining") == Some("0") => {
            CounterError::RateLimited {
                retry_after_s: retry_after_s(headers),
            }
        }
        403 => CounterError::Forbidden,
        404 => CounterError::NotFound,
        429 => CounterError::RateLimited {
            retry_after_s: retry_after_s(headers),
        },
        400 | 409 | 422 => CounterError::Rejected {
            message: first_line(body).unwrap_or_else(|| format!("HTTP {status}")),
        },
        500..=599 => CounterError::Server { status },
        other => CounterError::Rejected {
            message: format!("HTTP {other}"),
        },
    };
    Some(error)
}

/// URL da próxima página pelo cabeçalho `Link`, somente se continuar na origem HTTPS autorizada.
pub fn next_link(headers: &[(String, String)], allowed_origin: &str) -> Option<String> {
    let header = header_lookup(headers, "link")?;
    header.split(',').find_map(|part| {
        let part = part.trim();
        if !(part.contains("rel=\"next\"") || part.contains("rel=next")) {
            return None;
        }
        let start = part.find('<')? + 1;
        let end = start + part[start..].find('>')?;
        let candidate = &part[start..end];
        same_https_origin(candidate, allowed_origin).then(|| candidate.to_owned())
    })
}

fn same_https_origin(candidate: &str, allowed: &str) -> bool {
    fn authority(url: &str) -> Option<String> {
        let (scheme, rest) = url.split_once("://")?;
        if !scheme.eq_ignore_ascii_case("https") {
            return None;
        }
        let authority = rest.split(['/', '?', '#']).next()?;
        if authority.is_empty()
            || authority.contains(['@', '\\'])
            || authority.chars().any(char::is_whitespace)
        {
            return None;
        }
        Some(authority.to_ascii_lowercase())
    }

    authority(candidate)
        .zip(authority(allowed))
        .is_some_and(|(left, right)| left == right)
}

/// Número da próxima página pelo cabeçalho `X-Next-Page` (GitLab). Vazio ou zero: fim.
pub fn next_page_number(headers: &[(String, String)]) -> Option<u32> {
    header_lookup(headers, "x-next-page")?
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|page| *page > 0)
}

/// Segundos de espera pelo cabeçalho `Retry-After`, quando vem em segundos.
pub fn retry_after_s(headers: &[(String, String)]) -> Option<u64> {
    header_lookup(headers, "retry-after")?.trim().parse().ok()
}

fn header_lookup<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn first_line(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    Some(line.chars().take(200).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn every_common_status_maps_to_its_kind() {
        assert_eq!(error_for_status(200, &[], ""), None);
        assert_eq!(
            error_for_status(401, &[], ""),
            Some(CounterError::Unauthorized)
        );
        assert_eq!(
            error_for_status(403, &[], ""),
            Some(CounterError::Forbidden)
        );
        assert_eq!(error_for_status(404, &[], ""), Some(CounterError::NotFound));
        assert_eq!(
            error_for_status(429, &headers(&[("Retry-After", "42")]), ""),
            Some(CounterError::RateLimited {
                retry_after_s: Some(42)
            })
        );
        assert_eq!(
            error_for_status(502, &[], ""),
            Some(CounterError::Server { status: 502 })
        );
    }

    #[test]
    fn github_rate_limit_arrives_as_403_and_is_reported_as_rate_limited() {
        let limited = headers(&[("X-RateLimit-Remaining", "0")]);
        assert_eq!(
            error_for_status(403, &limited, ""),
            Some(CounterError::RateLimited {
                retry_after_s: None
            })
        );
        let plain = headers(&[("X-RateLimit-Remaining", "12")]);
        assert_eq!(
            error_for_status(403, &plain, ""),
            Some(CounterError::Forbidden)
        );
    }

    #[test]
    fn rejections_carry_the_first_line_of_the_body_only() {
        let error = error_for_status(422, &[], "\n  Validation failed\nstack…").unwrap();
        assert_eq!(
            error,
            CounterError::Rejected {
                message: "Validation failed".into()
            }
        );
        assert_eq!(
            error_for_status(418, &[], ""),
            Some(CounterError::Rejected {
                message: "HTTP 418".into()
            })
        );
    }

    #[test]
    fn link_header_yields_the_next_page_only() {
        let link = headers(&[(
            "Link",
            "<https://api.example/x?page=3>; rel=\"last\", <https://api.example/x?page=2>; rel=\"next\"",
        )]);
        assert_eq!(
            next_link(&link, "https://api.example/v1").as_deref(),
            Some("https://api.example/x?page=2")
        );
        assert_eq!(next_link(&link, "https://other.example"), None);
        assert_eq!(
            next_link(
                &headers(&[("Link", "<a>; rel=\"prev\"")]),
                "https://api.example"
            ),
            None
        );
        assert_eq!(next_link(&[], "https://api.example"), None);
    }

    #[test]
    fn pagination_rejects_different_hosts_http_and_authority_confusion() {
        for candidate in [
            "http://api.example/x?page=2",
            "https://api.example.attacker.invalid/x?page=2",
            "https://api.example@attacker.invalid/x?page=2",
        ] {
            let link = headers(&[("Link", &format!("<{candidate}>; rel=next"))]);
            assert_eq!(
                next_link(&link, "https://api.example/v1"),
                None,
                "{candidate}"
            );
        }
    }

    #[test]
    fn x_next_page_ignores_empty_and_zero() {
        assert_eq!(next_page_number(&headers(&[("X-Next-Page", "4")])), Some(4));
        assert_eq!(next_page_number(&headers(&[("X-Next-Page", "")])), None);
        assert_eq!(next_page_number(&headers(&[("X-Next-Page", "0")])), None);
    }

    #[test]
    fn authorization_is_hidden_in_debug_output() {
        let request = HttpRequest::get("https://h/x").bearer("ghp_secret");
        let shown = format!("{request:?}");
        assert!(!shown.contains("ghp_secret"));
        assert!(shown.contains("<redacted>"));
        assert_eq!(
            request.header_value("authorization"),
            Some("Bearer ghp_secret")
        );
    }

    #[test]
    fn json_helpers_report_decode_errors_as_decode() {
        let response = HttpResponse::new(200, "{not json");
        assert!(matches!(response.json(), Err(CounterError::Decode { .. })));
        assert_eq!(HttpResponse::new(204, "").json(), Ok(Value::Null));
    }
}
