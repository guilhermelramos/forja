//! Som do aviso. O ganho está nas amostras; o volume do sistema não muda.

/// Fração da amplitude máxima. O aviso fica audível sem ocupar o mixer.
pub const OWN_GAIN: f32 = 0.4;

/// Multiplica cada amostra. `gain` 1 deixa o sinal como está; 0 silencia.
pub fn apply_gain(samples: &mut [i16], gain: f32) {
    let gain = gain.clamp(0.0, 1.0);
    for sample in samples.iter_mut() {
        *sample = (*sample as f32 * gain).round() as i16;
    }
}

/// Onda curta já com o ganho próprio aplicado.
pub fn beep_samples(gain: f32) -> Vec<i16> {
    let rate = 22_050.0_f32;
    let frames = (rate * 0.12) as usize;
    let mut samples = Vec::with_capacity(frames);
    for index in 0..frames {
        let time = index as f32 / rate;
        let envelope = (1.0 - index as f32 / frames as f32).powi(2);
        let wave = (time * 880.0 * std::f32::consts::TAU).sin();
        samples.push((wave * envelope * i16::MAX as f32).round() as i16);
    }
    apply_gain(&mut samples, gain);
    samples
}

/// PCM 16-bit mono em um WAV mínimo.
pub fn wav_bytes(samples: &[i16], rate: u32) -> Vec<u8> {
    let data_bytes = samples.len() * 2;
    let mut bytes = Vec::with_capacity(44 + data_bytes);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_bytes as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_bytes as u32).to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

/// Toca o bipe numa thread de trabalho. Não chama API de mixer.
pub fn play_beep() {
    let wav = wav_bytes(&beep_samples(OWN_GAIN), 22_050);
    play_wav(&wav);
}

#[cfg(windows)]
fn play_wav(wav: &[u8]) {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_NODEFAULT, SND_SYNC};
    unsafe {
        PlaySoundW(
            wav.as_ptr().cast(),
            std::ptr::null_mut(),
            SND_MEMORY | SND_NODEFAULT | SND_SYNC,
        );
    }
}

#[cfg(not(windows))]
fn play_wav(_wav: &[u8]) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_scales_samples_without_touching_a_mixer() {
        let mut samples = [i16::MAX, -i16::MAX, 0];
        apply_gain(&mut samples, OWN_GAIN);
        assert_eq!(samples[0], (i16::MAX as f32 * OWN_GAIN).round() as i16);
        assert_eq!(samples[1], (-i16::MAX as f32 * OWN_GAIN).round() as i16);
        assert_eq!(samples[2], 0);
        let beep = beep_samples(OWN_GAIN);
        let peak = beep.iter().map(|sample| sample.unsigned_abs()).max().unwrap_or(0);
        let limit = (i16::MAX as f32 * OWN_GAIN).round() as u16 + 1;
        assert!(peak <= limit, "peak {peak} passed the own gain");
    }

    #[test]
    fn wav_header_matches_the_pcm_length() {
        let wav = wav_bytes(&[1, -2, 3], 22_050);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(wav.len(), 44 + 6);
    }
}
