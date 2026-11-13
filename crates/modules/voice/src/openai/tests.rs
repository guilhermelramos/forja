//! Testes da leitura da resposta e do código de idioma. Nenhum teste chama a rede.

use super::*;

#[test]
fn a_successful_response_gives_the_trimmed_text() {
    assert_eq!(read_response(200, r#"{"text": "  olá mundo \n"}"#), Ok("olá mundo".to_owned()));
}

#[test]
fn a_success_without_a_text_field_is_a_response_error() {
    assert_eq!(read_response(200, r#"{"other": 1}"#), Err("voice-error-response"));
    assert_eq!(read_response(200, "não é json"), Err("voice-error-response"));
}

#[test]
fn auth_failures_have_their_own_key() {
    assert_eq!(read_response(401, "{}"), Err("voice-error-auth"));
    assert_eq!(read_response(403, "{}"), Err("voice-error-auth"));
}

#[test]
fn other_statuses_are_http_errors() {
    assert_eq!(read_response(429, "{}"), Err("voice-error-http"));
    assert_eq!(read_response(500, "erro"), Err("voice-error-http"));
}

#[test]
fn the_language_setting_maps_to_iso_codes() {
    assert_eq!(language_code("pt_BR"), Some("pt"));
    assert_eq!(language_code("en"), Some("en"));
    assert_eq!(language_code("auto"), None, "auto lets the service detect the language");
    assert_eq!(language_code("qualquer"), None);
}

#[test]
fn the_boundary_is_unique_per_call() {
    let first = boundary();
    std::thread::sleep(Duration::from_millis(2));
    assert_ne!(first, boundary());
    assert!(first.starts_with("forja-"));
}
