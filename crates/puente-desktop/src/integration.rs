//! Une el móvil con el escritorio: notificaciones, portapapeles y archivos.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use puente_core::wire::NotificationInfo;
use puente_core::{DesktopEvent, Engine};
use puente_ui::{DesktopActions, Transfer};

use crate::audio::AudioMeter;

const CLIPBOARD_CHECK_EVERY: Duration = Duration::from_secs(1);
const MAX_CLIPBOARD_BYTES: usize = 1 << 20;
const FILE_CHUNK_BYTES: usize = 64 * 1024;

pub struct Integration {
    clipboard_sync: Arc<AtomicBool>,
    audio: AudioMeter,
    outbox: Sender<PathBuf>,
    transfer: Arc<Mutex<Option<Transfer>>>,
}

pub fn start(engine: Arc<Engine>, audio: AudioMeter) -> Integration {
    let clipboard_sync = Arc::new(AtomicBool::new(true));
    if let Some(events) = engine.take_desktop_events() {
        let engine = engine.clone();
        let sync = clipboard_sync.clone();
        std::thread::spawn(move || run(engine, sync, events));
    }
    let (outbox, pending) = channel::<PathBuf>();
    let transfer = Arc::new(Mutex::new(None));
    {
        let engine = engine.clone();
        let transfer = transfer.clone();
        std::thread::spawn(move || {
            for path in pending {
                send_file(&engine, &path, &transfer);
            }
        });
    }
    Integration {
        clipboard_sync,
        audio,
        outbox,
        transfer,
    }
}

impl DesktopActions for Integration {
    fn pick_and_send_file(&self) {
        let outbox = self.outbox.clone();
        std::thread::spawn(move || {
            for path in rfd::FileDialog::new().pick_files().unwrap_or_default() {
                let _ = outbox.send(path);
            }
        });
    }

    fn send_files(&self, paths: Vec<PathBuf>) {
        for path in paths {
            let _ = self.outbox.send(path);
        }
    }

    fn transfer(&self) -> Option<Transfer> {
        self.transfer.lock().expect("envío disponible").clone()
    }

    fn clipboard_sync(&self) -> bool {
        self.clipboard_sync.load(Ordering::SeqCst)
    }

    fn set_clipboard_sync(&self, enabled: bool) {
        self.clipboard_sync.store(enabled, Ordering::SeqCst);
    }

    fn audio_levels(&self) -> Vec<f32> {
        self.audio.levels()
    }
}

/// Envía un archivo y deja constancia de su avance.
fn send_file(engine: &Engine, path: &Path, transfer: &Mutex<Option<Transfer>>) {
    upload(engine, path, transfer);
    *transfer.lock().expect("envío disponible") = None;
}

fn upload(engine: &Engine, path: &Path, transfer: &Mutex<Option<Transfer>>) {
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned());
    let (Some(name), Ok(mut file)) = (name, File::open(path)) else {
        return;
    };
    let Ok(metadata) = file.metadata() else {
        return;
    };
    if !metadata.is_file() {
        return;
    }
    let size = metadata.len();
    let Some(id) = engine.begin_file(&name, size) else {
        return;
    };

    let mut buffer = vec![0; FILE_CHUNK_BYTES];
    let mut sent = 0_u64;
    loop {
        match file.read(&mut buffer) {
            Ok(0) => {
                engine.end_file(id, true);
                return;
            }
            Ok(read) => {
                if !engine.send_file_chunk(id, buffer[..read].to_vec()) {
                    return;
                }
                sent += read as u64;
                *transfer.lock().expect("envío disponible") = Some(Transfer {
                    name: name.clone(),
                    fraction: progress(sent, size),
                });
            }
            Err(_) => {
                engine.end_file(id, false);
                return;
            }
        }
    }
}

fn progress(sent: u64, size: u64) -> f32 {
    if size == 0 { 1.0 } else { sent as f32 / size as f32 }
}

fn run(engine: Arc<Engine>, sync: Arc<AtomicBool>, events: Receiver<DesktopEvent>) {
    let mut clipboard = arboard::Clipboard::new().ok();
    let mut last_text: Option<String> = None;

    loop {
        match events.recv_timeout(CLIPBOARD_CHECK_EVERY) {
            Ok(DesktopEvent::Notification(info)) => show_notification(&info),
            Ok(DesktopEvent::FileReceived(path, name)) => show_file_received(&path, &name),
            Ok(DesktopEvent::Clipboard(text)) => {
                if sync.load(Ordering::SeqCst)
                    && let Some(clipboard) = clipboard.as_mut()
                    && clipboard.set_text(text.clone()).is_ok()
                {
                    last_text = Some(text);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }

        if sync.load(Ordering::SeqCst)
            && engine.is_connected()
            && let Some(text) = clipboard.as_mut().and_then(|c| c.get_text().ok())
            && last_text.as_ref() != Some(&text)
        {
            if text.len() <= MAX_CLIPBOARD_BYTES {
                engine.send_clipboard(text.clone());
            }
            last_text = Some(text);
        }
    }
}

/// El notificador del escritorio interpreta marcas en el cuerpo del texto.
fn escape_markup(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn show_notification(info: &NotificationInfo) {
    let _ = notify_rust::Notification::new()
        .appname("Puente")
        .summary(&format!("{}: {}", info.app, info.title))
        .body(&escape_markup(&info.text))
        .show();
}

fn show_file_received(path: &Path, name: &str) {
    let _ = notify_rust::Notification::new()
        .appname("Puente")
        .summary("Archivo recibido")
        .body(&escape_markup(&format!("{name}\n{}", path.display())))
        .show();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_the_true_share_and_an_empty_file_is_complete() {
        assert_eq!(progress(0, 0), 1.0);
        assert_eq!(progress(25, 100), 0.25);
    }

    #[test]
    fn markup_from_the_phone_is_shown_as_plain_text() {
        assert_eq!(escape_markup("<b>a & b</b>"), "&lt;b&gt;a &amp; b&lt;/b&gt;");
    }
}
