//! Reproduce en la PC el audio del móvil.

use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use puente_core::Engine;

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
/// Más de 200 ms en cola sería retraso perceptible: se descarta lo más antiguo.
const MAX_QUEUED_SAMPLES: usize = (SAMPLE_RATE as usize) * (CHANNELS as usize) / 5;

/// Cuántos paquetes recientes se dibujan en la onda.
const LEVEL_HISTORY: usize = 34;
/// Sin audio durante este tiempo, la onda se apaga.
const SILENT_AFTER: Duration = Duration::from_millis(400);

#[derive(Default)]
struct Levels {
    recent: VecDeque<f32>,
    updated: Option<Instant>,
}

/// Los niveles reales del audio que está sonando, para dibujar la onda.
#[derive(Clone, Default)]
pub struct AudioMeter(Arc<Mutex<Levels>>);

impl AudioMeter {
    pub fn levels(&self) -> Vec<f32> {
        let levels = self.0.lock().expect("niveles disponibles");
        match levels.updated {
            Some(updated) if updated.elapsed() < SILENT_AFTER => levels.recent.iter().copied().collect(),
            _ => Vec::new(),
        }
    }

    fn record(&self, level: f32) {
        let mut levels = self.0.lock().expect("niveles disponibles");
        levels.recent.push_back(level);
        if levels.recent.len() > LEVEL_HISTORY {
            levels.recent.pop_front();
        }
        levels.updated = Some(Instant::now());
    }
}

/// Pico del paquete, en una escala que se parece más a lo que se oye.
fn level_of(packet: &[u8]) -> f32 {
    let peak = packet
        .as_chunks::<2>()
        .0
        .iter()
        .map(|bytes| i32::from(i16::from_le_bytes(*bytes)).abs())
        .max()
        .unwrap_or(0);
    (peak as f32 / f32::from(i16::MAX)).min(1.0).sqrt()
}

pub fn start(engine: &Arc<Engine>) -> AudioMeter {
    let meter = AudioMeter::default();
    if let Some(packets) = engine.take_audio_receiver() {
        let levels = meter.clone();
        let engine = engine.clone();
        std::thread::spawn(move || {
            if let Err(error) = play(packets, levels, &engine) {
                eprintln!("no se pudo reproducir el audio: {error}");
            }
        });
    }
    meter
}

fn play(packets: Receiver<Vec<u8>>, meter: AudioMeter, engine: &Engine) -> Result<(), Box<dyn std::error::Error>> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no hay dispositivo de salida")?;
    let config = cpal::StreamConfig {
        channels: CHANNELS,
        sample_rate: SAMPLE_RATE,
        buffer_size: cpal::BufferSize::Default,
    };

    let queue = Arc::new(Mutex::new(VecDeque::<i16>::new()));
    let source = queue.clone();
    let stream = device.build_output_stream(
        config,
        move |output: &mut [i16], _| {
            let mut queue = source.lock().expect("cola disponible");
            for sample in output {
                *sample = queue.pop_front().unwrap_or(0);
            }
        },
        |error| eprintln!("error de audio: {error}"),
        None,
    )?;
    stream.play()?;

    for packet in packets {
        let mut queue = queue.lock().expect("cola disponible");
        if !engine.audio_enabled() {
            queue.clear();
            continue;
        }
        meter.record(level_of(&packet));
        queue.extend(
            packet
                .as_chunks::<2>()
                .0
                .iter()
                .map(|bytes| i16::from_le_bytes(*bytes)),
        );
        let excess = queue.len().saturating_sub(MAX_QUEUED_SAMPLES);
        queue.drain(..excess);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(sample: i16) -> Vec<u8> {
        [sample, -sample].iter().flat_map(|value| value.to_le_bytes()).collect()
    }

    #[test]
    fn silence_measures_zero_and_a_full_signal_one() {
        assert_eq!(level_of(&packet(0)), 0.0);
        assert!((level_of(&packet(i16::MAX)) - 1.0).abs() < 0.001);
    }

    #[test]
    fn history_is_capped_and_a_new_meter_is_silent() {
        let meter = AudioMeter::default();
        assert!(meter.levels().is_empty());
        for _ in 0..(LEVEL_HISTORY + 10) {
            meter.record(0.5);
        }
        assert_eq!(meter.levels().len(), LEVEL_HISTORY);
    }
}
