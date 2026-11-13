//! Testes do corpo multipart: comparação byte a byte com o fixture, e o `Content-Type`.

use super::*;

/// O fixture é escrito com quebras LF; o corpo HTTP usa CRLF.
const FIXTURE: &str = include_str!("../../tests/fixtures/transcription_body.txt");

#[test]
fn the_transcription_body_matches_the_fixture_byte_for_byte() {
    let (content_type, body) = Multipart::new("forja-test")
        .text("model", "gpt-4o-mini-transcribe")
        .text("language", "pt")
        .file("file", "voz.wav", "audio/wav", b"RIFFtest")
        .finish();
    // Normalize first: a Windows checkout with core.autocrlf may already hold CRLF.
    let expected = FIXTURE.replace("\r\n", "\n").replace('\n', "\r\n");
    assert_eq!(String::from_utf8(body).expect("utf-8 fixture"), expected);
    assert_eq!(content_type, "multipart/form-data; boundary=forja-test");
}

#[test]
fn a_body_with_no_fields_is_only_the_closing_boundary() {
    let (_, body) = Multipart::new("x").finish();
    assert_eq!(body, b"--x--\r\n".to_vec());
}

#[test]
fn binary_file_bytes_are_copied_untouched() {
    let payload = [0u8, 255, 13, 10, 45, 45];
    let (_, body) = Multipart::new("b").file("f", "a.bin", "application/octet-stream", &payload).finish();
    let start = body
        .windows(payload.len())
        .position(|window| window == payload)
        .expect("payload is in the body");
    assert_eq!(&body[start..start + payload.len()], &payload);
}
