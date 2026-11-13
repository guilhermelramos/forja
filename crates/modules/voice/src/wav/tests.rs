//! Testes do WAV e do medidor: cabeçalho de 44 bytes, tamanhos e os limites das conversões.

use super::*;

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

#[test]
fn the_wav_header_describes_mono_16_bit_pcm_at_the_given_rate() {
    let samples: Vec<i16> = vec![0, 1000, -1000, 32767];
    let bytes = encode(&samples, 16_000).expect("encodes");
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(&bytes[12..16], b"fmt ");
    assert_eq!(u32_at(&bytes, 16), 16, "PCM fmt chunk is 16 bytes");
    assert_eq!(u16_at(&bytes, 20), 1, "PCM format");
    assert_eq!(u16_at(&bytes, 22), 1, "mono");
    assert_eq!(u32_at(&bytes, 24), 16_000, "sample rate");
    assert_eq!(u32_at(&bytes, 28), 32_000, "byte rate = rate * 2");
    assert_eq!(u16_at(&bytes, 34), 16, "bits per sample");
    assert_eq!(&bytes[36..40], b"data");
    assert_eq!(u32_at(&bytes, 40), 8, "four samples of two bytes");
    assert_eq!(u32_at(&bytes, 4), 36 + 8, "RIFF size");
    assert_eq!(bytes.len(), 44 + 8);
}

#[test]
fn the_samples_are_stored_little_endian_after_the_header() {
    let bytes = encode(&[1, -2], 8_000).expect("encodes");
    assert_eq!(&bytes[44..48], &[1, 0, 0xFE, 0xFF]);
}

#[test]
fn an_empty_recording_is_still_a_valid_header() {
    let bytes = encode(&[], 16_000).expect("encodes");
    assert_eq!(bytes.len(), 44);
    assert_eq!(u32_at(&bytes, 40), 0);
}

#[test]
fn the_level_is_the_rms_clamped_to_one() {
    assert_eq!(level(&[]), 0.0);
    assert_eq!(level(&[0.0; 8]), 0.0);
    assert!((level(&[0.5, -0.5, 0.5, -0.5]) - 0.5).abs() < 1e-6);
    assert_eq!(level(&[3.0, -3.0]), 1.0);
}

#[test]
fn samples_are_clamped_before_scaling() {
    assert_eq!(to_i16(0.0), 0);
    assert_eq!(to_i16(1.0), i16::MAX);
    assert_eq!(to_i16(-1.0), -i16::MAX);
    assert_eq!(to_i16(2.0), i16::MAX);
    assert_eq!(to_i16(-7.0), -i16::MAX);
}

#[test]
fn stereo_frames_are_averaged_to_mono() {
    assert_eq!(downmix(&[0.2, 0.4, -1.0, 1.0], 2), vec![0.3, 0.0]);
    assert_eq!(downmix(&[0.1, 0.2], 1), vec![0.1, 0.2]);
}

#[test]
fn the_duration_follows_the_rate() {
    assert!((seconds(16_000, 16_000) - 1.0).abs() < 1e-6);
    assert_eq!(seconds(10, 0), 0.0);
}
