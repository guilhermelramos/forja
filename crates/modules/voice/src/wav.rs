//! Áudio: conversão das amostras da captura, medidor de nível e o cabeçalho WAV (pelo `hound`).

use std::io::Cursor;

/// Nível do medidor de 0 a 1 para uma janela de amostras (RMS, limitado a 1).
pub fn level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mean_square = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    mean_square.sqrt().clamp(0.0, 1.0)
}

/// Amostra em ponto flutuante para inteiro de 16 bits, com limite nas bordas.
pub fn to_i16(sample: f32) -> i16 {
    let scaled = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round();
    scaled as i16
}

/// Mistura os canais de um quadro intercalado em um único valor por quadro (média).
pub fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

/// Codifica amostras mono de 16 bits como um arquivo WAV completo, em memória.
pub fn encode(samples: &[i16], sample_rate: u32) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(|error| error.to_string())?;
        for sample in samples {
            writer.write_sample(*sample).map_err(|error| error.to_string())?;
        }
        writer.finalize().map_err(|error| error.to_string())?;
    }
    Ok(cursor.into_inner())
}

/// Duração de um áudio de `samples` amostras mono a `sample_rate` Hz, em segundos.
pub fn seconds(samples: usize, sample_rate: u32) -> f32 {
    if sample_rate == 0 {
        return 0.0;
    }
    samples as f32 / sample_rate as f32
}

#[cfg(test)]
mod tests;
