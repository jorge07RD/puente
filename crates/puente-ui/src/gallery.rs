//! Muestra todas las piezas con datos de ejemplo, para revisarlas sin un móvil.
//! Se abre con `puente --gallery`; ningún dato de aquí llega a la app real.

use std::time::Duration;

use eframe::egui::{self, Rect, vec2};

use crate::RecentFile;
use crate::desktop::draw_recent_files;
use crate::dots::{self, BridgeState};
use crate::icons::Icon;
use crate::motion;
use crate::parts::{card_at, dot_figure, icon_button, label, pill_button, stat};
use crate::theme::{self, ACCENT, GAP, GROUND, INK};

const COLUMNS: usize = 3;
const ROWS: usize = 3;
const OUTER: f32 = 16.0;

#[derive(Default)]
pub struct Gallery;

impl Gallery {
    pub fn new(context: &egui::Context) -> Self {
        theme::install(context);
        Self
    }
}

fn sample_levels(time: f32) -> Vec<f32> {
    (0..34)
        .map(|index| {
            let x = index as f32;
            let wave = (x * 0.55 + time * 3.0).sin() * (x * 0.17 - time * 1.3).cos();
            (wave.abs() * 1.05).min(1.0)
        })
        .collect()
}

impl eframe::App for Gallery {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        context.request_repaint_after(Duration::from_millis(33));
        let time = context.input(|input| input.time) as f32;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(GROUND))
            .show(ui, |ui| {
                let area = ui.max_rect().shrink(OUTER);
                let width = (area.width() - GAP * (COLUMNS - 1) as f32) / COLUMNS as f32;
                let height = (area.height() - GAP * (ROWS - 1) as f32) / ROWS as f32;
                let cell = |column: usize, row: usize| {
                    let origin = area.left_top()
                        + vec2(column as f32 * (width + GAP), row as f32 * (height + GAP));
                    Rect::from_min_size(origin, vec2(width, height))
                };

                card_at(ui, cell(0, 0), "01", "Enlace", true, |ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 74.0), egui::Sense::hover());
                    let position = (time * 0.35).fract();
                    dots::paint_bridge(ui.painter(), rect, BridgeState::Traffic { position });
                    ui.add_space(14.0);
                    ui.columns(3, |columns| {
                        stat(&mut columns[0], "ms", "24");
                        stat(&mut columns[1], "fps", "58");
                        stat(&mut columns[2], "mbps", "7.4");
                    });
                });

                card_at(ui, cell(1, 0), "02", "Batería", false, |ui| {
                    let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
                    let radius = (rect.height().min(rect.width()) / 2.0 - 6.0).max(10.0);
                    let phase = (time / 2.4).fract();
                    dots::paint_charging_ring(ui.painter(), rect.center(), radius, 54, 0.78, Some(phase));
                    let size = dots::text_size("78", 6.0);
                    dots::paint_text(ui.painter(), rect.center() - size / 2.0, "78", 6.0, INK);
                });

                card_at(ui, cell(2, 0), "03", "Audio", true, |ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 96.0), egui::Sense::hover());
                    dots::paint_wave(ui.painter(), rect, &sample_levels(time), 7.0);
                    ui.add_space(8.0);
                    label(ui, "Sonando en la PC");
                });

                card_at(ui, cell(0, 1), "04", "Esperando", true, |ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 74.0), egui::Sense::hover());
                    dots::paint_bridge(ui.painter(), rect, motion::searching(time, true));
                    ui.add_space(14.0);
                    label(ui, "Buscando tu móvil");
                });

                card_at(ui, cell(1, 1), "05", "Código", false, |ui| {
                    dot_figure(ui, "482 917", "", 6.0, INK);
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        pill_button(ui, "Coinciden");
                        pill_button(ui, "Cancelar");
                    });
                });

                card_at(ui, cell(2, 1), "06", "Música", false, |ui| {
                    ui.label(egui::RichText::new("Título de ejemplo").size(16.0));
                    label(ui, "Artista de ejemplo");
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        icon_button(ui, Icon::Previous, 40.0, false);
                        icon_button(ui, Icon::Play, 52.0, true);
                        icon_button(ui, Icon::Next, 40.0, false);
                        icon_button(ui, Icon::Pause, 40.0, false);
                    });
                });

                card_at(ui, cell(0, 2), "07", "Archivos enviados", true, |ui| {
                    let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
                    let sample = |name: &str, delivered, opacity| RecentFile {
                        name: name.to_string(),
                        delivered,
                        opacity,
                    };
                    let files = [
                        sample("foto-de-ejemplo.jpg", true, 1.0),
                        sample("nombre-de-archivo-muy-largo-para-ver-como-se-recorta.pdf", true, 1.0),
                        sample("documento.zip", false, 1.0),
                        sample("desvaneciendo.png", true, 0.4),
                        sample("otro-1.png", true, 1.0),
                        sample("otro-2.png", true, 1.0),
                        sample("otro-3.png", true, 1.0),
                        sample("otro-4.png", true, 1.0),
                    ];
                    draw_recent_files(ui, rect, &files);
                });

                card_at(ui, cell(1, 2), "08", "Navegación", false, |ui| {
                    ui.horizontal(|ui| {
                        icon_button(ui, Icon::Back, 44.0, false);
                        icon_button(ui, Icon::Home, 44.0, false);
                        icon_button(ui, Icon::Recents, 44.0, false);
                        icon_button(ui, Icon::Close, 44.0, false);
                    });
                    ui.add_space(12.0);
                    dot_figure(ui, "100%", "", 5.0, ACCENT);
                });

                card_at(ui, cell(2, 2), "09", "Cifras y texto", false, |ui| {
                    dot_figure(ui, "0123456789", "", 5.0, INK);
                    ui.add_space(6.0);
                    dot_figure(ui, "12.4K", "ms", 6.0, INK);
                    ui.add_space(6.0);
                    ui.label("Texto normal con ñ, acentos y 1234567890.");
                    label(ui, "Datos de ejemplo");
                });
            });
    }
}
