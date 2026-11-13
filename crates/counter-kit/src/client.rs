//! Cliente HTTP de produção dos balcões, sobre `ureq` 3.
//!
//! Status não é erro aqui: 401, 404, 429 etc. voltam como `HttpResponse` e quem decide é
//! [`crate::error_for_status`]. Só falha de rede, tempo esgotado e corpo ilegível viram
//! `CounterError`. Roda sempre fora da thread da interface (dentro de `Ctx::spawn`).

use crate::http::{HttpRequest, HttpResponse, Method, Transport};
use forja_contracts::{CounterError, NetworkPolicy};
use std::time::Duration;

/// Tempo total de uma requisição. Balcão lento não pode travar a tarefa para sempre.
pub const TOTAL_TIMEOUT: Duration = Duration::from_secs(30);
/// Tempo para abrir a conexão.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct UreqTransport {
    proxy_agent: ureq::Agent,
    direct_agent: ureq::Agent,
    policy: NetworkPolicy,
}

impl UreqTransport {
    pub fn new() -> Self {
        Self::from_policy(&NetworkPolicy::default())
    }

    /// Constrói o transporte HTTP seguindo a política publicada por `network.proxy`.
    pub fn from_policy(policy: &NetworkPolicy) -> Self {
        let timeout = Duration::from_secs(policy.timeout_s.clamp(5, 120));
        let connect_timeout = CONNECT_TIMEOUT.min(timeout);
        let direct_agent = build_agent(timeout, connect_timeout, policy.tls_verify, None);
        let proxy_url = if policy.mode == "none" || policy.url.trim().is_empty() {
            None
        } else {
            Some(policy.url.trim())
        };
        let proxy_agent = build_agent(timeout, connect_timeout, policy.tls_verify, proxy_url);
        Self {
            proxy_agent,
            direct_agent,
            policy: policy.clone(),
        }
    }
}

fn build_agent(
    timeout: Duration,
    connect_timeout: Duration,
    tls_verify: bool,
    proxy_url: Option<&str>,
) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .disable_verification(!tls_verify)
        .build();
    let mut builder = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .timeout_connect(Some(connect_timeout))
        .max_redirects(0)
        .http_status_as_error(false)
        .tls_config(tls);
    if let Some(url) = proxy_url {
        if let Ok(proxy) = ureq::Proxy::new(url) {
            builder = builder.proxy(Some(proxy));
        }
    }
    ureq::Agent::new_with_config(builder.build())
}

fn extract_host(url: &str) -> &str {
    let without_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let authority = without_scheme.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority
        .rsplit_once('@')
        .map(|(_, hp)| hp)
        .unwrap_or(authority);
    if let Some(bracketed) = host_port.strip_prefix('[') {
        bracketed.split(']').next().unwrap_or("")
    } else {
        host_port.split(':').next().unwrap_or("")
    }
}

fn is_bypassed(url: &str, bypass: &[String]) -> bool {
    let host = extract_host(url).trim().to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    bypass.iter().any(|entry| {
        let pattern = entry.trim().to_ascii_lowercase();
        if pattern.is_empty() {
            return false;
        }
        if let Some(suffix) = pattern.strip_prefix("*.") {
            host == suffix || host.ends_with(&format!(".{suffix}"))
        } else if let Some(suffix) = pattern.strip_prefix('.') {
            host == suffix.trim_start_matches('.') || host.ends_with(&pattern)
        } else {
            host == pattern
        }
    })
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for UreqTransport {
    fn is_real(&self) -> bool {
        true
    }

    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, CounterError> {
        if self.policy.offline {
            return Err(CounterError::Network {
                message: "offline mode enabled by network.mode.offline".to_owned(),
            });
        }
        let agent = if is_bypassed(&request.url, &self.policy.bypass) {
            &self.direct_agent
        } else {
            &self.proxy_agent
        };
        let outcome = match request.method {
            Method::Get => with_headers(agent.get(&request.url), request).call(),
            Method::Delete => with_headers(agent.delete(&request.url), request).call(),
            Method::Post | Method::Put | Method::Patch => {
                let builder = match request.method {
                    Method::Post => agent.post(&request.url),
                    Method::Put => agent.put(&request.url),
                    _ => agent.patch(&request.url),
                };
                let builder = with_headers(builder, request);
                match &request.body {
                    Some(body) => builder.send_json(body),
                    None => builder.send_empty(),
                }
            }
        };
        let response = outcome.map_err(network_error)?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                Some((name.as_str().to_owned(), value.to_str().ok()?.to_owned()))
            })
            .collect();
        let body = response
            .into_body()
            .read_to_string()
            .map_err(network_error)?;
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}

fn with_headers<B>(
    mut builder: ureq::RequestBuilder<B>,
    request: &HttpRequest,
) -> ureq::RequestBuilder<B> {
    for (name, value) in &request.headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    builder
}

fn network_error(error: ureq::Error) -> CounterError {
    match error {
        ureq::Error::Timeout(timeout) => CounterError::Network {
            message: format!("timed out ({timeout:?})"),
        },
        other => CounterError::Network {
            message: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::error_for_status;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Servidor local de uma resposta: devolve o texto recebido para conferência.
    fn serve_once(
        status_line: &'static str,
        headers: &'static str,
        body: &'static str,
    ) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback is available");
        let address = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("one connection");
            let mut received = Vec::new();
            let mut buffer = [0u8; 4096];
            // Lê até o fim dos cabeçalhos e do corpo declarado.
            loop {
                let read = stream.read(&mut buffer).unwrap_or(0);
                if read == 0 {
                    break;
                }
                received.extend_from_slice(&buffer[..read]);
                let text = String::from_utf8_lossy(&received);
                if let Some(end) = text.find("\r\n\r\n") {
                    let declared = text
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length: ")
                                .or_else(|| line.strip_prefix("Content-Length: "))
                        })
                        .and_then(|value| value.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if received.len() >= end + 4 + declared {
                        break;
                    }
                }
            }
            let response = format!(
                "{status_line}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&received).into_owned()
        });
        (address, handle)
    }

    #[test]
    fn a_real_round_trip_sends_headers_and_reads_status_headers_and_body() {
        let (address, server) = serve_once(
            "HTTP/1.1 200 OK",
            "Content-Type: application/json\r\nX-Next-Page: 2\r\n",
            r#"{"login":"ana-dev"}"#,
        );
        let request = HttpRequest::get(format!("{address}/user")).bearer("ghp_local_test");
        let response = UreqTransport::new()
            .send(&request)
            .expect("loopback responds");
        assert_eq!(response.status, 200);
        assert_eq!(response.header("x-next-page"), Some("2"));
        assert_eq!(response.json().unwrap()["login"], "ana-dev");
        let received = server.join().unwrap();
        assert!(received.starts_with("GET /user HTTP/1.1"));
        assert!(received
            .to_lowercase()
            .contains("authorization: bearer ghp_local_test"));
    }

    #[test]
    fn a_json_body_is_sent_and_a_refusal_is_a_status_not_an_error() {
        let (address, server) = serve_once("HTTP/1.1 401 Unauthorized", "", "Bad credentials");
        let request = HttpRequest::post(
            format!("{address}/comments"),
            serde_json::json!({"body": "oi"}),
        );
        let response = UreqTransport::new()
            .send(&request)
            .expect("status is data, not transport failure");
        assert_eq!(response.status, 401);
        assert_eq!(
            error_for_status(response.status, &response.headers, &response.body),
            Some(CounterError::Unauthorized)
        );
        let received = server.join().unwrap();
        assert!(received.starts_with("POST /comments HTTP/1.1"));
        let (_, sent_body) = received.split_once("\r\n\r\n").expect("headers end");
        let sent: serde_json::Value = serde_json::from_str(sent_body).expect("JSON body");
        assert_eq!(sent["body"], "oi");
    }

    #[test]
    fn a_redirect_is_returned_without_sending_credentials_to_its_target() {
        let (address, server) = serve_once(
            "HTTP/1.1 302 Found",
            "Location: http://127.0.0.1:1/credential-capture\r\n",
            "",
        );
        let request = HttpRequest::get(format!("{address}/user")).bearer("ghp_redirect_test");
        let response = UreqTransport::new()
            .send(&request)
            .expect("redirect is a response");
        assert_eq!(response.status, 302);
        let received = server.join().unwrap();
        assert!(received
            .to_lowercase()
            .contains("authorization: bearer ghp_redirect_test"));
    }

    #[test]
    fn an_unreachable_host_is_a_network_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let request = HttpRequest::get(format!("http://127.0.0.1:{port}/x"));
        assert!(matches!(
            UreqTransport::new().send(&request),
            Err(CounterError::Network { .. })
        ));
    }

    #[test]
    fn offline_policy_blocks_before_opening_socket() {
        let (address, server) = serve_once("HTTP/1.1 200 OK", "", "ok");
        let policy = NetworkPolicy {
            offline: true,
            ..NetworkPolicy::default()
        };
        let transport = UreqTransport::from_policy(&policy);
        let result = transport.send(&HttpRequest::get(format!("{address}/ping")));
        assert!(matches!(result, Err(CounterError::Network { .. })));
        drop(server);
    }

    #[test]
    fn manual_proxy_routes_non_bypassed_requests_and_respects_bypass() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback is available");
        let proxy_addr = format!("http://{}", listener.local_addr().unwrap());
        let proxy_server = std::thread::spawn(move || {
            fn read_headers(stream: &mut std::net::TcpStream) -> String {
                let mut received = Vec::new();
                let mut buffer = [0u8; 4096];
                loop {
                    let read = stream.read(&mut buffer).unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    received.extend_from_slice(&buffer[..read]);
                    if String::from_utf8_lossy(&received).contains("\r\n\r\n") {
                        break;
                    }
                }
                String::from_utf8_lossy(&received).into_owned()
            }

            let (mut stream, _) = listener.accept().expect("one proxy connection");
            let first = read_headers(&mut stream);
            let combined = if first.starts_with("CONNECT ") {
                stream
                    .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                    .unwrap();
                stream.flush().unwrap();
                let inner = read_headers(&mut stream);
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nvia-proxy")
                    .unwrap();
                format!("{first}{inner}")
            } else {
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nvia-proxy")
                    .unwrap();
                first
            };
            stream.flush().unwrap();
            let _ = stream.shutdown(std::net::Shutdown::Write);
            let mut drain = [0u8; 64];
            let _ = stream.read(&mut drain);
            combined
        });
        let (direct_addr, direct_server) = serve_once("HTTP/1.1 200 OK", "", "direct-ok");

        let mut policy = NetworkPolicy {
            mode: "manual".to_owned(),
            url: proxy_addr,
            bypass: vec!["127.0.0.1".to_owned(), "*.internal".to_owned()],
            tls_verify: false,
            timeout_s: 15,
            ..NetworkPolicy::default()
        };
        let transport = UreqTransport::from_policy(&policy);
        assert!(transport.is_real());
        assert!(is_bypassed("https://127.0.0.1:8080/api", &policy.bypass));
        assert!(is_bypassed("https://git.internal/api", &policy.bypass));
        assert!(!is_bypassed("https://api.github.com/user", &policy.bypass));

        let response = transport
            .send(&HttpRequest::get("http://example.test/status"))
            .expect("manual proxy handles HTTP request");
        assert_eq!(response.status, 200);
        assert_eq!(response.body, "via-proxy");
        let received = proxy_server.join().unwrap();
        assert!(received.contains("example.test"));

        let direct_response = transport
            .send(&HttpRequest::get(format!("{direct_addr}/health")))
            .expect("bypassed host skips proxy");
        assert_eq!(direct_response.body, "direct-ok");
        assert!(direct_server.join().unwrap().contains("GET /health"));

        policy.mode = "none".to_owned();
        let _direct = UreqTransport::from_policy(&policy);
    }
}
