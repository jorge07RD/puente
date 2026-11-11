//! La ventana puede esconderse en la bandeja sin cortar la conexión con el móvil.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};

use eframe::egui;
use puente_ui::PuenteApp;
use puente_ui::i18n::{self, Language};

const NOTHING: u8 = 0;
const SHOW: u8 = 1;
const QUIT: u8 = 2;

/// Lo que la bandeja y otras instancias le piden a la ventana desde otros hilos.
#[derive(Default)]
pub struct WindowControl {
    context: OnceLock<egui::Context>,
    request: AtomicU8,
    /// Hay una bandeja donde volver a abrirla: cerrar solo la esconde.
    tray: AtomicBool,
}

impl WindowControl {
    pub fn show(&self) {
        self.ask(SHOW);
    }

    pub fn quit(&self) {
        self.ask(QUIT);
    }

    pub fn set_tray_available(&self, available: bool) {
        self.tray.store(available, Ordering::Relaxed);
    }

    pub fn tray_available(&self) -> bool {
        self.tray.load(Ordering::Relaxed)
    }

    fn ask(&self, request: u8) {
        self.request.store(request, Ordering::Relaxed);
        if let Some(context) = self.context.get() {
            context.request_repaint();
        }
    }
}

/// Envuelve la app para decidir qué pasa al cerrar la ventana.
pub struct Shell {
    app: PuenteApp,
    control: Arc<WindowControl>,
    quitting: bool,
    /// eframe muestra la ventana tras el primer cuadro: se esconde en el siguiente.
    start_hidden: bool,
    frames: u32,
    /// Para rehacer el menú de la bandeja cuando cambia el idioma.
    language: Language,
    on_language_change: Box<dyn Fn()>,
}

impl Shell {
    /// `hidden`: arranca escondida en la bandeja, como al iniciar la sesión.
    pub fn new(
        context: &egui::Context,
        app: PuenteApp,
        control: Arc<WindowControl>,
        hidden: bool,
        on_language_change: Box<dyn Fn()>,
    ) -> Self {
        let _ = control.context.set(context.clone());
        let start_hidden = hidden && control.tray_available();
        Self {
            app,
            control,
            quitting: false,
            start_hidden,
            frames: 0,
            language: i18n::language(),
            on_language_change,
        }
    }
}

impl eframe::App for Shell {
    /// Corre aunque la ventana esté escondida, así que aquí se atienden las peticiones.
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.frames = self.frames.saturating_add(1);
        if self.start_hidden {
            if self.frames > 1 {
                self.start_hidden = false;
                context.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            } else {
                context.request_repaint();
            }
        }
        if i18n::language() != self.language {
            self.language = i18n::language();
            (self.on_language_change)();
        }
        match self.control.request.swap(NOTHING, Ordering::Relaxed) {
            SHOW => {
                context.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                context.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                context.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            QUIT => {
                self.quitting = true;
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            _ => {}
        }
        let closing = context.input(|input| input.viewport().close_requested());
        if closing && !self.quitting && self.control.tray_available() {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            context.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        eframe::App::ui(&mut self.app, ui, frame);
    }
}
