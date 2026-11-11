//! Icono en la bandeja del sistema, por StatusNotifierItem sobre D-Bus.

use std::sync::Arc;

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{CheckmarkItem, StandardItem};
use ksni::{MenuItem, ToolTip, Tray};
use puente_ui::i18n::t;

use crate::autostart;
use crate::window::WindowControl;

struct PuenteTray {
    window: Arc<WindowControl>,
}

impl Tray for PuenteTray {
    fn id(&self) -> String {
        "puente".into()
    }

    fn title(&self) -> String {
        "Puente".into()
    }

    fn icon_name(&self) -> String {
        "puente".into()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Puente".into(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.window.show();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let texts = t();
        vec![
            StandardItem {
                label: texts.tray_show.into(),
                activate: Box::new(|tray: &mut Self| tray.window.show()),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: texts.tray_autostart.into(),
                checked: autostart::is_enabled(),
                activate: Box::new(|_: &mut Self| {
                    if let Err(error) = autostart::set_enabled(!autostart::is_enabled()) {
                        eprintln!("puente: no se pudo cambiar el arranque automático: {error}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: texts.tray_quit.into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Self| tray.window.quit()),
                ..Default::default()
            }
            .into(),
        ]
    }

    /// Si la barra se reinicia, el icono vuelve cuando reaparezca.
    fn watcher_offline(&self, _reason: ksni::OfflineReason) -> bool {
        true
    }
}

pub struct TrayIcon(Handle<PuenteTray>);

impl TrayIcon {
    /// Rehace el menú, por ejemplo tras cambiar de idioma.
    pub fn refresh(&self) {
        self.0.update(|_| {});
    }
}

/// Sin una barra que muestre iconos, devuelve `None` y cerrar la ventana sale de la app.
pub fn start(window: Arc<WindowControl>) -> Option<TrayIcon> {
    let handle = PuenteTray { window: window.clone() }.spawn().ok()?;
    window.set_tray_available(true);
    Some(TrayIcon(handle))
}
