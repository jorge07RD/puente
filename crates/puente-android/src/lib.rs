#![cfg(target_os = "android")]

mod bridge;

use std::ffi::CStr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, Once, OnceLock};

use android_activity::AndroidApp;
use eframe::egui;
use puente_core::{Config, Engine, Role};
use puente_ui::PuenteApp;

/// El motor vive tanto como el proceso: el servicio de captura debe seguir
/// enviando vídeo aunque la pantalla de la app ya no exista.
static ENGINE: OnceLock<Result<Arc<Engine>, String>> = OnceLock::new();
static CONTEXT: Mutex<Option<egui::Context>> = Mutex::new(None);
static PENDING_REPORTS: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Android puede activar el servicio de Accesibilidad antes de que exista el motor.
pub(crate) static INPUT_ENABLED: AtomicBool = AtomicBool::new(false);
pub(crate) static NOTIFICATIONS_ENABLED: AtomicBool = AtomicBool::new(false);
pub(crate) static BATTERY_UNRESTRICTED: AtomicBool = AtomicBool::new(false);
/// Píxeles de las barras del sistema, arriba y abajo.
pub(crate) static SAFE_TOP: AtomicU32 = AtomicU32::new(0);
pub(crate) static SAFE_BOTTOM: AtomicU32 = AtomicU32::new(0);
/// Tamaño de letra de Android, en bits de un f32.
pub(crate) static FONT_SCALE: AtomicU32 = AtomicU32::new(0x3f80_0000);

fn device_name() -> String {
    let mut value = [0 as libc::c_char; libc::PROP_VALUE_MAX as usize];
    // SAFETY: `value` tiene PROP_VALUE_MAX bytes, el máximo que escribe esta función.
    let length = unsafe { libc::__system_property_get(c"ro.product.model".as_ptr(), value.as_mut_ptr()) };
    if length <= 0 {
        return "Android".to_string();
    }
    // SAFETY: la función deja la cadena terminada en NUL dentro de `value`.
    unsafe { CStr::from_ptr(value.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

fn start_engine(data_dir: PathBuf) -> Result<Arc<Engine>, String> {
    let config = Config::new(Role::Phone, data_dir, device_name());
    let repaint = || {
        if let Some(context) = CONTEXT.lock().expect("contexto disponible").as_ref() {
            context.request_repaint();
        }
    };
    let engine = Engine::start(config, repaint)
        .map(Arc::new)
        .map_err(|error| error.to_string())?;
    engine.set_input_enabled(INPUT_ENABLED.load(Ordering::SeqCst));
    engine.set_notifications_enabled(NOTIFICATIONS_ENABLED.load(Ordering::SeqCst));
    Ok(engine)
}

/// Lo puede arrancar la pantalla o cualquier servicio, según cuál exista primero.
pub(crate) fn ensure_engine(data_dir: PathBuf) -> Result<Arc<Engine>, String> {
    install_panic_report();
    let engine = ENGINE.get_or_init(|| start_engine(data_dir)).clone();
    if engine.is_ok() {
        for line in PENDING_REPORTS.lock().expect("registro disponible").drain(..) {
            report(line);
        }
    }
    engine
}

/// Deja una línea en el registro que guarda la PC.
pub(crate) fn report(text: String) {
    log::info!("{text}");
    match running_engine() {
        Some(engine) => engine.log(text),
        None => PENDING_REPORTS.lock().expect("registro disponible").push(text),
    }
}

/// Un pánico en el hilo de la pantalla la deja congelada sin cerrar la app, así
/// que hay que contarlo o sería invisible.
fn install_panic_report() {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        std::panic::set_hook(Box::new(|panic| {
            let thread = std::thread::current();
            report(format!("PÁNICO en {}: {panic}", thread.name().unwrap_or("sin nombre")));
        }));
    });
}

pub(crate) fn request_repaint() {
    if let Some(context) = CONTEXT.lock().expect("contexto disponible").as_ref() {
        context.request_repaint();
    }
}

pub(crate) fn running_engine() -> Option<Arc<Engine>> {
    ENGINE.get()?.as_ref().ok().cloned()
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("puente"),
    );

    install_panic_report();
    report("android_main: inicio".to_string());
    let data_dir = app.internal_data_path().unwrap_or_else(|| PathBuf::from("."));
    let actions = bridge::ActivityActions::new(&app);
    let options = eframe::NativeOptions {
        android_app: Some(app),
        ..Default::default()
    };
    let result = eframe::run_native(
        "Puente",
        options,
        Box::new(move |creation| {
            *CONTEXT.lock().expect("contexto disponible") = Some(creation.egui_ctx.clone());
            let engine = ensure_engine(data_dir);
            let mut puente = PuenteApp::new(&creation.egui_ctx, Role::Phone, engine);
            if let Some(actions) = actions {
                puente = puente.with_phone_actions(Box::new(actions));
            }
            Ok(Box::new(puente))
        }),
    );
    match result {
        Ok(()) => report("android_main: la interfaz terminó".to_string()),
        Err(error) => report(format!("android_main: no se pudo iniciar la interfaz: {error}")),
    }
}
