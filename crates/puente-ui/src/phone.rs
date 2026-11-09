//! La app del móvil: emparejar, compartir pantalla y dar permisos.

use eframe::egui::{self, Align, Layout, Margin, RichText, Sense, vec2};
use puente_core::{Engine, Found, Status, View};

use crate::PhoneActions;
use crate::desktop::spaced_code;
use crate::dots::{self, BridgeState, MIN_TEXT_PITCH};
use crate::icons::Icon;
use crate::motion::{self, LinkMotion};
use crate::parts::{card, dot_figure, icon_button, label, pill, stat, switch_row};
use crate::theme::{ACCENT, GAP, INK, INK_SOFT};

const MARGIN: f32 = 14.0;
/// Área táctil mínima en Android.
const TOUCH: f32 = 48.0;
const BRIDGE_HEIGHT: f32 = 72.0;
const MAX_CODE_PITCH: f32 = 9.0;

#[derive(Default)]
pub struct PhoneState {
    link: LinkMotion,
}

impl PhoneState {
    pub fn is_settling(&self, time: f32) -> bool {
        self.link.settling(time)
    }
}

fn draw_bridge(ui: &mut egui::Ui, state: BridgeState) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), BRIDGE_HEIGHT), Sense::hover());
    dots::paint_bridge(ui.painter(), rect, state);
}

fn apply_system_settings(ui: &egui::Ui, phone: &dyn PhoneActions) {
    let scale = phone.font_scale().clamp(1.0, 1.4);
    if (ui.ctx().zoom_factor() - scale).abs() > 0.01 {
        ui.ctx().set_zoom_factor(scale);
    }
}

pub fn draw(
    ui: &mut egui::Ui,
    engine: &Engine,
    view: &View,
    phone: Option<&dyn PhoneActions>,
    state: &mut PhoneState,
    time: f32,
) {
    let bridge = if matches!(view.status, Status::Connected { .. }) {
        state.link.bridge(view.mirroring, time, view.animations)
    } else {
        state.link.disconnected();
        motion::searching(time, view.animations)
    };
    let (top, bottom) = match phone {
        Some(phone) => {
            apply_system_settings(ui, phone);
            let (top, bottom) = phone.safe_area_pixels();
            let scale = ui.ctx().pixels_per_point();
            (top / scale, bottom / scale)
        }
        None => (0.0, 0.0),
    };

    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        let margin = Margin {
            left: MARGIN as i8,
            right: MARGIN as i8,
            top: (top + MARGIN).min(127.0) as i8,
            bottom: (bottom + MARGIN).min(127.0) as i8,
        };
        egui::Frame::NONE.inner_margin(margin).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = GAP;
            dot_figure(ui, "PUENTE", "", MIN_TEXT_PITCH, INK);
            ui.add_space(4.0);

            if let Some(error) = &view.last_error {
                ui.colored_label(ACCENT, error);
            }
            match &view.status {
                Status::Searching => draw_searching(ui, engine, view, bridge),
                Status::Connecting { name } => {
                    card(ui, "01", "Enlace", true, |ui| {
                        draw_bridge(ui, bridge);
                        label(ui, &format!("Conectando con {name}"));
                    });
                }
                Status::Pairing {
                    name,
                    code,
                    answered,
                } => draw_pairing(ui, engine, name, code, *answered, bridge),
                Status::Connected { name } => draw_connected(ui, engine, view, phone, name, bridge),
            }
        });
    });
}

fn draw_searching(ui: &mut egui::Ui, engine: &Engine, view: &View, bridge: BridgeState) {
    card(ui, "01", "Enlace", true, |ui| {
        draw_bridge(ui, bridge);
        label(ui, "Buscando tu PC en la Wi-Fi");
    });

    let unpaired: Vec<&Found> = view.found.iter().filter(|found| !found.paired).collect();
    let middle_card = !unpaired.is_empty() || view.paired.is_empty();
    if !unpaired.is_empty() {
        card(ui, "02", "PCs encontradas", false, |ui| {
            for found in unpaired {
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(&found.name).truncate());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if pill(ui, "Emparejar", TOUCH, true).clicked() {
                            engine.connect(found.fingerprint);
                        }
                    });
                });
            }
        });
    } else if view.paired.is_empty() {
        card(ui, "02", "Primeros pasos", false, |ui| {
            ui.label("Abre Puente en tu PC, con este móvil en la misma Wi-Fi. Aparecerá aquí.");
        });
    }

    if !view.paired.is_empty() {
        let index = if middle_card { "03" } else { "02" };
        card(ui, index, "Emparejadas", false, |ui| {
            for peer in &view.paired {
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(&peer.name).truncate());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if pill(ui, "Olvidar", TOUCH, false).clicked() {
                            let _ = engine.forget(peer.fingerprint);
                        }
                    });
                });
            }
        });
    }
}

fn draw_pairing(ui: &mut egui::Ui, engine: &Engine, name: &str, code: &str, answered: bool, bridge: BridgeState) {
    card(ui, "01", "Emparejar", false, |ui| {
        label(ui, name);
        ui.add_space(6.0);

        let text = spaced_code(code);
        let unit = dots::text_size(&text, 1.0).x + 2.0;
        let pitch = (ui.available_width() / unit).clamp(MIN_TEXT_PITCH, MAX_CODE_PITCH);
        dot_figure(ui, &text, "", pitch, INK);
        ui.add_space(8.0);

        if answered {
            draw_bridge(ui, bridge);
            label(ui, "Esperando al otro dispositivo");
            return;
        }

        ui.label("Acepta solo si este código es igual en los dos dispositivos.");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if pill(ui, "Coinciden", TOUCH, true).clicked() {
                engine.answer_pairing(true);
            }
            if pill(ui, "Cancelar", TOUCH, false).clicked() {
                engine.answer_pairing(false);
            }
        });
    });
}

fn draw_connected(
    ui: &mut egui::Ui,
    engine: &Engine,
    view: &View,
    phone: Option<&dyn PhoneActions>,
    name: &str,
    bridge: BridgeState,
) {
    card(ui, "01", "Enlace", true, |ui| {
        draw_bridge(ui, bridge);
        ui.add(egui::Label::new(RichText::new(name).color(INK_SOFT)).truncate());
        let round_trip = view
            .round_trip_ms
            .map_or_else(|| "—".to_string(), |value| value.to_string());
        stat(ui, "ms", &round_trip);
    });

    let Some(phone) = phone else {
        return;
    };

    card(ui, "02", "Pantalla", view.mirroring, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(4.0);
            if view.mirroring {
                if icon_button(ui, Icon::Stop, 96.0, true).clicked() {
                    phone.stop_capture();
                }
                ui.add_space(4.0);
                label(ui, "Compartiendo. Toca para detener");
                ui.label(RichText::new("La pantalla no se apagará mientras la PC esté conectada.").color(INK_SOFT));
            } else {
                if icon_button(ui, Icon::Play, 96.0, true).clicked() {
                    phone.start_capture();
                }
                ui.add_space(4.0);
                label(ui, "Compartir pantalla");
                ui.label(RichText::new("Android te pedirá confirmarlo cada vez.").color(INK_SOFT));
            }
        });
    });

    card(ui, "03", "Permisos", false, |ui| {
        permission_row(ui, "Control desde la PC", view.input_enabled, || {
            phone.open_accessibility_settings();
        });
        permission_row(ui, "Avisos en la PC", view.notifications_enabled, || {
            phone.open_notification_settings();
        });
        permission_row(ui, "Seguir en segundo plano", phone.battery_unrestricted(), || {
            phone.open_battery_settings();
        });
        if !view.input_enabled || !view.notifications_enabled {
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Si Android lo bloquea: Ajustes, Aplicaciones, Puente, menú de arriba, \
                     Permitir ajustes restringidos.",
                )
                .color(INK_SOFT),
            );
        }
    });

    card(ui, "04", "Ajustes", false, |ui| {
        let animations = view.animations;
        if switch_row(ui, "Animaciones", animations, TOUCH).clicked() {
            engine.set_animations(!animations);
        }
    });
}

fn permission_row(ui: &mut egui::Ui, title: &str, enabled: bool, open: impl FnOnce()) {
    ui.horizontal(|ui| {
        ui.set_min_height(TOUCH);
        ui.add(egui::Label::new(title).truncate());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if enabled {
                label(ui, "Activo");
            } else if pill(ui, "Activar", TOUCH, true).clicked() {
                open();
            }
        });
    });
}
