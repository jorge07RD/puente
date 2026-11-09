mod audio;
mod decoder;
mod integration;

use std::path::PathBuf;
use std::sync::Arc;

use eframe::EventLoopBuilderHook;
use eframe::egui;
use winit::platform::x11::EventLoopBuilderExtX11;
use puente_core::{Config, Engine, Role};
use puente_ui::{Demo, DemoKind, Gallery, PuenteApp};

fn data_dir() -> PathBuf {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    config_home.join("puente")
}

fn device_name() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|name| name.trim().to_string())
        .unwrap_or_else(|_| "PC".to_string())
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

fn main() -> eframe::Result {
    let backend = std::env::var("PUENTE_BACKEND").ok();
    let x11 = use_x11(backend.as_deref(), std::env::var_os("DISPLAY").is_some());
    let options = eframe::NativeOptions {
        event_loop_builder: x11.then(|| -> EventLoopBuilderHook {
            Box::new(|builder| {
                builder.with_x11();
            })
        }),
        viewport: egui::ViewportBuilder::default()
            .with_app_id("puente")
            .with_inner_size([1100.0, 860.0]),
        ..Default::default()
    };
    let arguments: Vec<String> = std::env::args().collect();
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

    #[test]
    fn xwayland_is_used_unless_asked_not_to_or_missing() {
        assert!(use_x11(None, true));
        assert!(!use_x11(Some("wayland"), true));
        assert!(!use_x11(None, false));
    }
}
