#![cfg_attr(windows, windows_subsystem = "windows")]

mod audio;
mod decoder;
#[cfg(unix)]
mod handoff;
mod integration;

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use puente_core::settings::SettingsFile;
use puente_core::{Config, Engine, Role};
use puente_ui::i18n::{self, t};
use puente_ui::{Demo, DemoKind, Gallery, PuenteApp};

fn data_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("puente")
}

fn device_name() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|name| name.trim().to_string())
        .ok()
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .unwrap_or_else(|| "PC".to_string())
}

fn build_app(context: egui::Context) -> PuenteApp {
    let mut config = Config::new(Role::Desktop, data_dir(), device_name());
    if let Some(downloads) = dirs::download_dir() {
        config.downloads_dir = downloads;
    }
    let repaint = context.clone();
    let repaint_context = context.clone();
    let engine = match Engine::start(config, move || repaint.request_repaint()) {
        Ok(engine) => Arc::new(engine),
        Err(error) => return PuenteApp::new(&repaint_context, Role::Desktop, Err(error.to_string())),
    };
    let screen = decoder::start(engine.clone(), context);
    let audio = audio::start(&engine);
    let integration = integration::start(engine.clone(), audio);
    PuenteApp::new(&repaint_context, Role::Desktop, Ok(engine))
        .with_screen(Box::new(screen))
        .with_desktop_actions(Box::new(integration))
}

#[cfg(target_os = "linux")]
/// `winit` solo recibe archivos arrastrados por X11, así que se prefiere XWayland
/// si existe. `PUENTE_BACKEND=wayland` lo evita, a costa de no poder arrastrar;
/// sirve para pantallas con escalado fraccional, donde XWayland se ve borroso.
fn use_x11(backend: Option<&str>, has_x11_display: bool) -> bool {
    backend != Some("wayland") && has_x11_display
}

/// `--demo <carpeta>` y `--demo-phone <carpeta>` graban las imágenes del README.
fn demo_request(arguments: &[String]) -> Option<(DemoKind, PathBuf)> {
    arguments.windows(2).find_map(|pair| {
        let kind = match pair[0].as_str() {
            "--demo" => DemoKind::Desktop,
            "--demo-phone" => DemoKind::Phone,
            _ => return None,
        };
        Some((kind, PathBuf::from(&pair[1])))
    })
}

#[cfg(target_os = "linux")]
fn linux_event_loop() -> Option<eframe::EventLoopBuilderHook> {
    use winit::platform::x11::EventLoopBuilderExtX11;

    let backend = std::env::var("PUENTE_BACKEND").ok();
    let x11 = use_x11(backend.as_deref(), std::env::var_os("DISPLAY").is_some());
    x11.then(|| -> eframe::EventLoopBuilderHook {
        Box::new(|builder| {
            builder.with_x11();
        })
    })
}

/// `puente --send <archivos>`: los entrega a la ventana abierta y termina.
fn send_and_exit(paths: &[String]) -> ! {
    if paths.is_empty() {
        eprintln!("{}", t().send_usage);
        std::process::exit(2);
    }
    #[cfg(unix)]
    let result = handoff::send(paths);
    #[cfg(not(unix))]
    let result: Result<(), String> = Err(t().linux_only.to_string());
    match result {
        Ok(()) => {
            println!("{}", (t().sending_files)(paths.len()));
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("{}", (t().send_error)(&error));
            std::process::exit(1);
        }
    }
}

/// El idioma del sistema, salvo que `--lang <código>` diga otro. La elección guardada
/// en Ajustes manda sobre los dos.
fn set_up_language(arguments: &[String]) {
    let forced = arguments
        .windows(2)
        .find(|pair| pair[0] == "--lang")
        .map(|pair| pair[1].clone());
    let system = forced.or_else(sys_locale::get_locale).unwrap_or_default();
    i18n::set_system_language(&system);
    let settings = SettingsFile::load(data_dir().join("settings.json"));
    i18n::apply(settings.current.language);
}

fn main() -> eframe::Result {
    let arguments: Vec<String> = std::env::args().collect();
    set_up_language(&arguments);
    if let Some(at) = arguments.iter().position(|argument| argument == "--send") {
        send_and_exit(&arguments[at + 1..]);
    }
    let options = eframe::NativeOptions {
        #[cfg(target_os = "linux")]
        event_loop_builder: linux_event_loop(),
        viewport: egui::ViewportBuilder::default()
            .with_app_id("puente")
            .with_inner_size([1100.0, 860.0]),
        ..Default::default()
    };
    if let Some((kind, output)) = demo_request(&arguments) {
        let size = match kind {
            DemoKind::Desktop => [1280.0, 780.0],
            DemoKind::Phone => [420.0, 900.0],
        };
        let mut options = options;
        options.viewport = options.viewport.with_inner_size(size).with_resizable(false);
        return eframe::run_native(
            "Puente",
            options,
            Box::new(move |creation| {
                Demo::new(&creation.egui_ctx, kind, output)
                    .map(|demo| Box::new(demo) as Box<dyn eframe::App>)
                    .map_err(Into::into)
            }),
        );
    }

    if std::env::args().any(|argument| argument == "--gallery") {
        return eframe::run_native(
            "Puente",
            options,
            Box::new(|creation| Ok(Box::new(Gallery::new(&creation.egui_ctx)))),
        );
    }
    eframe::run_native(
        "Puente",
        options,
        Box::new(|creation| Ok(Box::new(build_app(creation.egui_ctx.clone())))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn xwayland_is_used_unless_asked_not_to_or_missing() {
        assert!(use_x11(None, true));
        assert!(!use_x11(Some("wayland"), true));
        assert!(!use_x11(None, false));
    }
}
