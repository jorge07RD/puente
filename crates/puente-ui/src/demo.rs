//! Reproduce un guion con datos de ejemplo y guarda cada cuadro, para generar las
//! imágenes del README. Lee solo lo que dibuja la app: ninguna otra ventana ni
//! notificación del escritorio puede acabar en las imágenes.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, ColorImage, ScreenshotCallback, TextureOptions, ViewportCommand};
use puente_core::wire::{MediaInfo, NotificationInfo};
use puente_core::{Config, Engine, Role, Status, View};

use crate::desktop::{self, DesktopState};
use crate::phone::{self, PhoneState};
use crate::theme::{self, GROUND};
use crate::{DesktopActions, PhoneActions, RecentFile, Transfer};

const SCREEN_WIDTH: usize = 540;
const SCREEN_HEIGHT: usize = 1170;

#[derive(Clone, Copy, PartialEq)]
pub enum DemoKind {
    Desktop,
    Phone,
}

#[derive(Clone, Copy, PartialEq)]
enum Scene {
    Searching,
    Pairing,
    Connected,
    Streaming,
}

impl DemoKind {
    fn frames_per_second(self) -> f32 {
        match self {
            Self::Desktop => 15.0,
            Self::Phone => 4.0,
        }
    }

    fn duration(self) -> f32 {
        match self {
            Self::Desktop => 11.0,
            Self::Phone => 4.0,
        }
    }

    fn scene(self, time: f32) -> Scene {
        let limits = match self {
            Self::Desktop => [2.4, 3.4, 5.6, 9.6],
            Self::Phone => [1.0, 2.0, 3.0, 4.0],
        };
        match limits.iter().position(|limit| time < *limit) {
            Some(0) => Scene::Searching,
            Some(1) => Scene::Pairing,
            Some(2) => Scene::Connected,
            Some(3) => Scene::Streaming,
            _ => Scene::Connected,
        }
    }
}

#[derive(Default)]
struct SampleDesktop {
    levels: Mutex<Vec<f32>>,
    recent: Mutex<Vec<RecentFile>>,
}

impl DesktopActions for SampleDesktop {
    fn pick_and_send_file(&self) {}

    fn send_files(&self, _paths: Vec<PathBuf>) {}

    fn transfer(&self) -> Option<Transfer> {
        None
    }

    fn recent_files(&self) -> Vec<RecentFile> {
        self.recent.lock().expect("muestra disponible").clone()
    }

    fn clipboard_sync(&self) -> bool {
        true
    }

    fn set_clipboard_sync(&self, _enabled: bool) {}

    fn audio_levels(&self) -> Vec<f32> {
        self.levels.lock().expect("muestra disponible").clone()
    }
}

struct SamplePhone;

impl PhoneActions for SamplePhone {
    fn start_capture(&self) {}

    fn stop_capture(&self) {}

    fn open_accessibility_settings(&self) {}

    fn open_notification_settings(&self) {}

    fn open_battery_settings(&self) {}

    fn battery_unrestricted(&self) -> bool {
        true
    }

    fn safe_area_pixels(&self) -> (f32, f32) {
        (0.0, 0.0)
    }

    fn font_scale(&self) -> f32 {
        1.0
    }
}

pub struct Demo {
    kind: DemoKind,
    engine: Arc<Engine>,
    output: PathBuf,
    frame: u32,
    waiting_for_shot: bool,
    shot_sender: Sender<Arc<ColorImage>>,
    shots: Receiver<Arc<ColorImage>>,
    texture: egui::TextureHandle,
    desktop_state: DesktopState,
    phone_state: PhoneState,
    desktop: SampleDesktop,
}

impl Demo {
    pub fn new(context: &egui::Context, kind: DemoKind, output: PathBuf) -> Result<Self, String> {
        theme::install(context);
        fs::create_dir_all(&output).map_err(|error| error.to_string())?;

        let data_dir = std::env::temp_dir().join(format!("puente-demo-{}", std::process::id()));
        let mut config = Config::new(Role::Desktop, data_dir, "Mi PC".to_string());
        config.port = 0;
        let engine = Engine::start(config, || {}).map_err(|error| error.to_string())?;

        let (shot_sender, shots) = channel();
        let texture = context.load_texture("muestra", sample_phone_screen(), TextureOptions::LINEAR);
        Ok(Self {
            kind,
            engine: Arc::new(engine),
            output,
            frame: 0,
            waiting_for_shot: false,
            shot_sender,
            shots,
            texture,
            desktop_state: DesktopState::default(),
            phone_state: PhoneState::default(),
            desktop: SampleDesktop::default(),
        })
    }

    fn sample_view(&self, scene: Scene) -> View {
        let mut view = self.engine.view();
        let streaming = scene == Scene::Streaming;
        let peer = match self.kind {
            DemoKind::Desktop => "Pixel de ejemplo",
            DemoKind::Phone => "Mi PC",
        };
        view.status = match scene {
            Scene::Searching => Status::Searching,
            Scene::Pairing => Status::Pairing {
                name: peer.to_string(),
                code: "482917".to_string(),
                answered: false,
            },
            Scene::Connected | Scene::Streaming => Status::Connected {
                name: peer.to_string(),
            },
        };
        view.mirroring = streaming;
        view.input_enabled = true;
        view.notifications_enabled = true;
        view.battery = Some((78, true));
        view.round_trip_ms = Some(6);
        view.video_fps = streaming.then_some(58);
        view.video_mbps = streaming.then_some(7.4);
        view.media = Some(MediaInfo {
            title: "Canción de ejemplo".to_string(),
            artist: "Artista de ejemplo".to_string(),
            playing: true,
        });
        view.notifications = vec![
            sample_notice("1", "Mensajes", "Ana", "¿Llegas a tiempo a la cena?"),
            sample_notice("2", "Correo", "Factura de noviembre", "Ya está disponible tu factura."),
            sample_notice("3", "Calendario", "Reunión de equipo", "Empieza en 15 minutos."),
        ];
        view
    }

    fn update_samples(&self, time: f32, scene: Scene) {
        let levels = if scene == Scene::Streaming {
            (0..34)
                .map(|index| {
                    let x = index as f32;
                    ((x * 0.55 + time * 3.0).sin() * (x * 0.17 - time * 1.3).cos()).abs().min(1.0)
                })
                .collect()
        } else {
            Vec::new()
        };
        *self.desktop.levels.lock().expect("muestra disponible") = levels;

        let recent = if (7.0..9.5).contains(&time) {
            vec![
                sample_sent("foto-de-ejemplo.jpg"),
                sample_sent("documento.pdf"),
            ]
        } else {
            Vec::new()
        };
        *self.desktop.recent.lock().expect("muestra disponible") = recent;
    }
}

fn sample_notice(key: &str, app: &str, title: &str, text: &str) -> NotificationInfo {
    NotificationInfo {
        key: key.to_string(),
        app: app.to_string(),
        title: title.to_string(),
        text: text.to_string(),
    }
}

fn sample_sent(name: &str) -> RecentFile {
    RecentFile {
        name: name.to_string(),
        delivered: true,
        opacity: 1.0,
    }
}

fn write_ppm(path: &Path, image: &ColorImage) -> std::io::Result<()> {
    let [width, height] = image.size;
    let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
    bytes.reserve(width * height * 3);
    for pixel in &image.pixels {
        bytes.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b()]);
    }
    fs::File::create(path)?.write_all(&bytes)
}

/// Una pantalla de inicio inventada, para no mostrar la de nadie.
fn sample_phone_screen() -> ColorImage {
    const ICON_COLORS: [Color32; 4] = [
        Color32::from_rgb(0x3b, 0x4a, 0x5c),
        Color32::from_rgb(0x5a, 0x4a, 0x3a),
        Color32::from_rgb(0x3a, 0x55, 0x4c),
        Color32::from_rgb(0x4c, 0x3f, 0x5e),
    ];
    let background = Color32::from_rgb(0x12, 0x15, 0x1a);
    let mut image = ColorImage::new([SCREEN_WIDTH, SCREEN_HEIGHT], vec![background; SCREEN_WIDTH * SCREEN_HEIGHT]);

    let paint_disc = |image: &mut ColorImage, cx: i32, cy: i32, radius: i32, color: Color32| {
        for y in (cy - radius)..=(cy + radius) {
            for x in (cx - radius)..=(cx + radius) {
                let inside = (x - cx).pow(2) + (y - cy).pow(2) <= radius.pow(2);
                if inside && x >= 0 && y >= 0 && (x as usize) < SCREEN_WIDTH && (y as usize) < SCREEN_HEIGHT {
                    image[(x as usize, y as usize)] = color;
                }
            }
        }
    };

    // La hora, con la misma matriz de puntos de la app.
    let pitch = 11;
    let mut x = 96;
    for character in "12:34".chars() {
        for (row, bits) in crate::dots::glyph_rows(character).iter().enumerate() {
            for column in 0..5 {
                if bits & (0b10000 >> column) != 0 {
                    let cy = 170 + row as i32 * pitch;
                    paint_disc(&mut image, x + column * pitch, cy, 4, Color32::from_rgb(0xf2, 0xf2, 0xef));
                }
            }
        }
        x += if character == ':' { pitch * 3 } else { pitch * 6 };
    }

    for row in 0..4_i32 {
        for column in 0..4_i32 {
            let color = ICON_COLORS[((row + column) % 4) as usize];
            paint_disc(&mut image, 96 + column * 116, 560 + row * 130, 40, color);
        }
    }
    image
}

impl eframe::App for Demo {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();

        let shots: Vec<Arc<ColorImage>> = self.shots.try_iter().collect();
        for image in shots {
            let path = self.output.join(format!("frame-{:04}.ppm", self.frame));
            if let Err(error) = write_ppm(&path, &image) {
                eprintln!("no se pudo guardar el cuadro: {error}");
            }
            self.frame += 1;
            self.waiting_for_shot = false;
        }

        let time = self.frame as f32 / self.kind.frames_per_second();
        if time >= self.kind.duration() {
            context.send_viewport_cmd(ViewportCommand::Close);
            return;
        }

        let scene = self.kind.scene(time);
        self.update_samples(time, scene);
        let view = self.sample_view(scene);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(GROUND))
            .show(ui, |ui| match self.kind {
                DemoKind::Desktop => desktop::draw(
                    ui,
                    &self.engine,
                    &view,
                    Some(&self.texture),
                    Some(&self.desktop),
                    &mut self.desktop_state,
                    time,
                ),
                DemoKind::Phone => phone::draw(
                    ui,
                    &self.engine,
                    &view,
                    Some(&SamplePhone),
                    &mut self.phone_state,
                    time,
                ),
            });

        if !self.waiting_for_shot {
            self.waiting_for_shot = true;
            let sender = self.shot_sender.clone();
            context.send_viewport_cmd(ViewportCommand::Screenshot(ScreenshotCallback::new(move |image| {
                let _ = sender.send(image);
            })));
        }
        context.request_repaint();
    }
}
