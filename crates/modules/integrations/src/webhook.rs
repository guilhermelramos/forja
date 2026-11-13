//! Envio dos eventos ao webhook. A rede roda em `Ctx::spawn`, nunca na thread da
//! interface. Só `http` e `https` são aceitos, e o corpo é JSON:
//! `{"event": <nome>, "at": <ms>, "data": <evento>}`.

use serde_json::{json, Value};
use std::time::Duration;

/// Tempo máximo de um envio.
const TIMEOUT: Duration = Duration::from_secs(10);

/// A URL é usável: `http://` ou `https://` seguido de algo, sem espaços.
pub fn valid_url(url: &str) -> bool {
    let url = url.trim();
    let Some(rest) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) else {
        return false;
    };
    !rest.is_empty() && !url.contains(char::is_whitespace)
}

/// O evento foi escolhido na configuração.
pub fn wanted(events: &[String], name: &str) -> bool {
    events.iter().any(|event| event == name)
}

/// Corpo enviado ao webhook.
pub fn envelope(name: &str, data: Value, at_ms: u64) -> Value {
    json!({ "event": name, "at": at_ms, "data": data })
}

/// Envia o corpo por POST. Erro de rede ou status fora de 2xx vira texto.
pub fn send(url: &str, body: &Value) -> Result<(), String> {
    let response = ureq::post(url)
        .config()
        .timeout_global(Some(TIMEOUT))
        .build()
        .send_json(body)
        .map_err(|error| error.to_string())?;
    let status = response.status().as_u16();
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(format!("HTTP {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn only_http_and_https_urls_are_accepted() {
        assert!(valid_url("https://example.test/hook"));
        assert!(valid_url("http://127.0.0.1:8080/x"));
        assert!(!valid_url(""));
        assert!(!valid_url("ftp://example.test"));
        assert!(!valid_url("https://"));
        assert!(!valid_url("https://exa mple.test"));
    }

    #[test]
    fn only_chosen_events_are_sent() {
        let events = vec!["bench.created".to_owned()];
        assert!(wanted(&events, "bench.created"));
        assert!(!wanted(&events, "stage.changed"));
        assert!(!wanted(&[], "bench.created"));
    }

    #[test]
    fn envelope_carries_the_event_name_time_and_data() {
        let body = envelope("stage.changed", json!({"to": "review"}), 42);
        assert_eq!(body["event"], "stage.changed");
        assert_eq!(body["at"], 42);
        assert_eq!(body["data"]["to"], "review");
    }

    /// Reads one HTTP request on loopback, answers `status` and returns the request text.
    fn one_shot_server(status: u16) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let url = format!("http://{}/hook", listener.local_addr().expect("addr"));
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut received: Vec<u8> = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let read = stream.read(&mut buffer).expect("read");
                if read == 0 {
                    break;
                }
                received.extend_from_slice(&buffer[..read]);
                if complete(&received) {
                    break;
                }
            }
            let reply = format!(
                "HTTP/1.1 {status} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let _ = stream.write_all(reply.as_bytes());
            String::from_utf8_lossy(&received).into_owned()
        });
        (url, handle)
    }

    /// True once the headers and the `Content-Length` bytes of the body have arrived.
    fn complete(received: &[u8]) -> bool {
        let text = String::from_utf8_lossy(received);
        let Some(split) = text.find("\r\n\r\n") else {
            return false;
        };
        let length = text[..split]
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        received.len() >= split + 4 + length
    }

    #[test]
    fn posts_the_json_body_and_accepts_2xx() {
        let (url, server) = one_shot_server(204);
        let body = envelope("bench.created", json!({"name": "x"}), 7);
        assert_eq!(send(&url, &body), Ok(()));
        let request = server.join().expect("server");
        assert!(request.starts_with("POST /hook"));
        let (_, payload) = request.split_once("\r\n\r\n").expect("header end");
        let parsed: Value = serde_json::from_str(payload).expect("json body");
        assert_eq!(parsed["event"], "bench.created");
        assert_eq!(parsed["at"], 7);
    }

    #[test]
    fn a_rejected_status_is_an_error() {
        let (url, server) = one_shot_server(500);
        let result = send(&url, &envelope("bench.created", json!({}), 1));
        assert!(result.is_err());
        server.join().expect("server");
    }
}
