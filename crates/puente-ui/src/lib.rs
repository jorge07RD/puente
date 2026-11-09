//! Pantallas egui compartidas entre la PC y el móvil.

mod demo;
mod desktop;
pub mod dots;
mod gallery;
pub mod icons;
mod motion;
pub mod parts;
mod phone;
pub mod theme;

pub use demo::{Demo, DemoKind};
pub use gallery::Gallery;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;
use puente_core::{Engine, Role, Status, View};

use crate::parts::label;
use crate::theme::{ACCENT, GROUND};

/// Cada cuánto se redibuja mientras hay algo animándose.
const ANIMATION_FRAME: Duration = Duration::from_millis(40);
/// Android descarta los cuadros dibujados antes de que su superficie esté lista:
/// si no se vuelve a dibujar, la pantalla de arranque se queda puesta.
const PHONE_REFRESH: Duration = Duration::from_millis(250);

/// Una imagen ya decodificada de la pantalla del móvil.
pub struct ScreenFrame {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// PC: de dónde sale la última imagen decodificada.
pub trait ScreenSource {
    fn take_latest(&self) -> Option<ScreenFrame>;
}

/// Móvil: acciones que solo el sistema operativo puede hacer.
pub trait PhoneActions {
    fn start_capture(&self);
    fn stop_capture(&self);
    fn open_accessibility_settings(&self);
    fn open_notification_settings(&self);
    fn open_battery_settings(&self);
    /// Android tiene permitido no dormir la app en segundo plano.
    fn battery_unrestricted(&self) -> bool;
    /// Píxeles que ocupan arriba y abajo las barras del sistema.
    fn safe_area_pixels(&self) -> (f32, f32);
    /// Tamaño de letra elegido en los ajustes de Android.
    fn font_scale(&self) -> f32;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Transfer {
    pub name: String,
    pub fraction: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecentFile {
    pub name: String,
    pub delivered: bool,
    /// De 1 a 0 cuando la línea está a punto de desaparecer.
    pub opacity: f32,
}

/// PC: acciones que dependen del escritorio.
pub trait DesktopActions {
    fn pick_and_send_file(&self);
    fn send_files(&self, paths: Vec<PathBuf>);
    /// El archivo que se está enviando ahora, si hay alguno.
    fn transfer(&self) -> Option<Transfer>;
    /// Los envíos terminados hace poco, del más reciente al más antiguo.
    fn recent_files(&self) -> Vec<RecentFile>;
    fn clipboard_sync(&self) -> bool;
    fn set_clipboard_sync(&self, enabled: bool);
    /// Niveles recientes del audio, de 0 a 1; vacío si no suena nada.
    fn audio_levels(&self) -> Vec<f32>;
}

pub struct PuenteApp {
    role: Role,
    engine: Result<Arc<Engine>, String>,
    screen: Option<Box<dyn ScreenSource>>,
    phone: Option<Box<dyn PhoneActions>>,
    desktop: Option<Box<dyn DesktopActions>>,
    texture: Option<egui::TextureHandle>,
    desktop_state: desktop::DesktopState,
    phone_state: phone::PhoneState,
}

impl PuenteApp {
    pub fn new(context: &egui::Context, role: Role, engine: Result<Arc<Engine>, String>) -> Self {
        theme::install(context);
        Self {
            role,
            engine,
            screen: None,
            phone: None,
            desktop: None,
            texture: None,
            desktop_state: desktop::DesktopState::default(),
            phone_state: phone::PhoneState::default(),
        }
    }

    pub fn with_screen(mut self, screen: Box<dyn ScreenSource>) -> Self {
        self.screen = Some(screen);
        self
    }

    pub fn with_desktop_actions(mut self, desktop: Box<dyn DesktopActions>) -> Self {
        self.desktop = Some(desktop);
        self
    }

    pub fn with_phone_actions(mut self, phone: Box<dyn PhoneActions>) -> Self {
        self.phone = Some(phone);
        self
    }

    fn update_texture(&mut self, context: &egui::Context) {
        let Some(frame) = self.screen.as_ref().and_then(|screen| screen.take_latest()) else {
            return;
        };
        let image = egui::ColorImage::from_rgba_unmultiplied([frame.width, frame.height], &frame.rgba);
        let options = egui::TextureOptions::LINEAR;
        match &mut self.texture {
            Some(texture) => texture.set(image, options),
            None => self.texture = Some(context.load_texture("pantalla", image, options)),
        }
    }
}

fn is_animating(view: &View) -> bool {
    view.animations
        && (view.mirroring
            || view.battery.is_some_and(|(_, charging)| charging)
            || match &view.status {
                Status::Searching | Status::Connecting { .. } => true,
                Status::Pairing { answered, .. } => *answered,
                Status::Connected { .. } => false,
            })
}

fn draw_failure(ui: &mut egui::Ui, error: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(96.0);
        ui.colored_label(ACCENT, "No se pudo iniciar Puente");
        ui.add_space(8.0);
        label(ui, error);
    });
}

impl eframe::App for PuenteApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        self.update_texture(&context);
        let time = context.input(|input| input.time) as f32;
        let engine = self.engine.clone();

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(GROUND))
            .show(ui, |ui| match &engine {
                Err(error) => draw_failure(ui, error),
                Ok(engine) => {
                    let view = engine.view();
                    // Sin transmisión no debe quedar a la vista la última imagen recibida.
                    if !view.mirroring {
                        self.texture = None;
                    }
                    if self.role == Role::Phone {
                        context.request_repaint_after(PHONE_REFRESH);
                    }
                    let settling = view.animations
                        && (self.desktop_state.is_settling(time) || self.phone_state.is_settling(time));
                    if is_animating(&view) || settling {
                        context.request_repaint_after(ANIMATION_FRAME);
                    }
                    match self.role {
                        Role::Desktop => desktop::draw(
                            ui,
                            engine,
                            &view,
                            self.texture.as_ref(),
                            self.desktop.as_deref(),
                            &mut self.desktop_state,
                            time,
                        ),
                        Role::Phone => phone::draw(
                            ui,
                            engine,
                            &view,
                            self.phone.as_deref(),
                            &mut self.phone_state,
                            time,
                        ),
                    }
                }
            });
    }
}
