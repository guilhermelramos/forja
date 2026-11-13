//! Gravação local pelo microfone padrão (`cpal`). O stream vive numa thread própria: ela o cria,
//! toca, espera o sinal de parada e o solta. Assim o stream não cruza threads. A captura chega
//! como amostras de 16 bits mono, e o nível atual fica num contador atômico para a tela.

use crate::wav;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;

/// Limite de uma gravação: 5 minutos de áudio mono, para a memória não crescer sem fim.
pub const MAX_SECONDS: u32 = 300;

/// O que a gravação devolve ao parar.
pub struct Capture {
    pub samples: Vec<i16>,
    pub sample_rate: u32,
}

/// Uma gravação em andamento. `stop` consome a gravação e devolve o áudio.
pub struct Recorder {
    stop: mpsc::Sender<()>,
    worker: Option<JoinHandle<Result<Capture, &'static str>>>,
    level: Arc<AtomicU32>,
}

impl Recorder {
    /// Abre o microfone padrão e começa a gravar. Erro se não houver microfone ou se o áudio não abrir.
    pub fn start() -> Result<Self, &'static str> {
        let level = Arc::new(AtomicU32::new(0));
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), &'static str>>();
        let worker_level = Arc::clone(&level);
        let worker = std::thread::Builder::new()
            .name("forja-voice-capture".to_owned())
            .spawn(move || capture(stop_rx, ready_tx, worker_level))
            .map_err(|_| "voice-error-audio")?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                stop: stop_tx,
                worker: Some(worker),
                level,
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(_) => {
                let _ = worker.join();
                Err("voice-error-audio")
            }
        }
    }

    /// Nível atual de 0 a 1, lido do contador que a thread de áudio atualiza.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    /// Para a gravação e devolve o áudio capturado.
    pub fn stop(mut self) -> Result<Capture, &'static str> {
        let _ = self.stop.send(());
        match self.worker.take() {
            Some(handle) => handle.join().unwrap_or(Err("voice-error-audio")),
            None => Err("voice-error-audio"),
        }
    }
}

/// Corpo da thread: abre o stream, espera a parada e devolve o que foi gravado.
fn capture(
    stop_rx: mpsc::Receiver<()>,
    ready_tx: mpsc::Sender<Result<(), &'static str>>,
    level: Arc<AtomicU32>,
) -> Result<Capture, &'static str> {
    let opened = open_stream(Arc::clone(&level));
    let (stream, sink, sample_rate) = match opened {
        Ok(opened) => opened,
        Err(error) => {
            let _ = ready_tx.send(Err(error));
            return Err(error);
        }
    };
    if stream.play().is_err() {
        let _ = ready_tx.send(Err("voice-error-audio"));
        return Err("voice-error-audio");
    }
    let _ = ready_tx.send(Ok(()));
    let _ = stop_rx.recv();
    drop(stream);
    let samples = sink.lock().map(|mut buffer| std::mem::take(&mut *buffer)).unwrap_or_default();
    Ok(Capture {
        samples,
        sample_rate,
    })
}

type Sink = Arc<Mutex<Vec<i16>>>;

/// Abre o microfone padrão no formato que ele oferece. Devolve o stream (ainda parado), o buffer
/// onde as amostras vão cair e a taxa de amostragem.
fn open_stream(level: Arc<AtomicU32>) -> Result<(cpal::Stream, Sink, u32), &'static str> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("voice-error-no-mic")?;
    let supported = device
        .default_input_config()
        .map_err(|_| "voice-error-audio")?;
    let format = supported.sample_format();
    let config = supported.config();
    let channels = usize::from(config.channels.max(1));
    let rate = config.sample_rate;
    let max_samples = (rate as usize).saturating_mul(MAX_SECONDS as usize);
    let sink: Sink = Arc::new(Mutex::new(Vec::new()));
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(
            &device,
            &config,
            channels,
            max_samples,
            Arc::clone(&sink),
            level,
            |sample| sample,
        )?,
        cpal::SampleFormat::I16 => build::<i16>(
            &device,
            &config,
            channels,
            max_samples,
            Arc::clone(&sink),
            level,
            |sample| sample as f32 / i16::MAX as f32,
        )?,
        _ => return Err("voice-error-audio"),
    };
    Ok((stream, sink, rate))
}

/// Monta o stream de entrada para um tipo de amostra. Cada bloco vira mono, atualiza o nível e
/// entra no buffer até o limite.
fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    max_samples: usize,
    sink: Sink,
    level: Arc<AtomicU32>,
    to_f32: fn(T) -> f32,
) -> Result<cpal::Stream, &'static str>
where
    T: cpal::SizedSample + Send + 'static,
{
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _info| {
                let floats: Vec<f32> = data.iter().map(|sample| to_f32(*sample)).collect();
                let mono = wav::downmix(&floats, channels);
                level.store(wav::level(&mono).to_bits(), Ordering::Relaxed);
                if let Ok(mut buffer) = sink.lock() {
                    let room = max_samples.saturating_sub(buffer.len());
                    buffer.extend(mono.iter().take(room).map(|sample| wav::to_i16(*sample)));
                }
            },
            |_error| {},
            None,
        )
        .map_err(|_| "voice-error-audio")
}
