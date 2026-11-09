//! Decodifica el vídeo del móvil en un hilo aparte.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use openh264::decoder::Decoder;
use openh264::formats::YUVSource;
use openh264::nal_units;
use puente_core::Engine;
use puente_core::wire::VideoPacket;
use puente_ui::{ScreenFrame, ScreenSource};

const KEYFRAME_REQUEST_EVERY: Duration = Duration::from_secs(1);

pub struct LatestFrame(Arc<Mutex<Option<ScreenFrame>>>);

impl ScreenSource for LatestFrame {
    fn take_latest(&self) -> Option<ScreenFrame> {
        self.0.lock().expect("imagen disponible").take()
    }
}

pub fn start(engine: Arc<Engine>, context: egui::Context) -> LatestFrame {
    let latest = Arc::new(Mutex::new(None));
    if let Some(packets) = engine.take_video_receiver() {
        let target = latest.clone();
        std::thread::spawn(move || decode(packets, target, engine, context));
    }
    LatestFrame(latest)
}

fn decode(
    packets: Receiver<VideoPacket>,
    latest: Arc<Mutex<Option<ScreenFrame>>>,
    engine: Arc<Engine>,
    context: egui::Context,
) {
    let mut decoder = match Decoder::new() {
        Ok(decoder) => decoder,
        Err(error) => {
            eprintln!("no se pudo crear el decodificador H.264: {error}");
            return;
        }
    };
    let mut last_request = Instant::now() - KEYFRAME_REQUEST_EVERY;

    for packet in packets {
        for unit in nal_units(&packet.data) {
            match decoder.decode(unit) {
                Ok(Some(image)) => {
                    let (width, height) = image.dimensions();
                    let mut rgba = vec![0; width * height * 4];
                    image.write_rgba8(&mut rgba);
                    *latest.lock().expect("imagen disponible") = Some(ScreenFrame {
                        width,
                        height,
                        rgba,
                    });
                    context.request_repaint();
                }
                Ok(None) => {}
                Err(_) if last_request.elapsed() >= KEYFRAME_REQUEST_EVERY => {
                    last_request = Instant::now();
                    engine.request_keyframe();
                }
                Err(_) => {}
            }
        }
    }
}
