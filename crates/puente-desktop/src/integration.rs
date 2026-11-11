//! Une el móvil con el escritorio: notificaciones, portapapeles y archivos.

use std::fs::File;
use std::io::Read;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use puente_core::wire::{CallInfo, NotificationInfo};
use puente_core::{DesktopEvent, Engine};
use puente_ui::i18n::t;
use puente_ui::{DesktopActions, RecentFile, Transfer};

use crate::audio::AudioMeter;

const CLIPBOARD_CHECK_EVERY: Duration = Duration::from_secs(1);
const MAX_CLIPBOARD_BYTES: usize = 1 << 20;
const FILE_CHUNK_BYTES: usize = 64 * 1024;
/// Cuánto se ve cada envío en la lista.
const RECENT_FOR: Duration = Duration::from_secs(12);
/// Parte final de ese tiempo en la que la línea se desvanece.
const FADE_FOR: Duration = Duration::from_millis(1500);
/// Sin confirmación del móvil en este tiempo, el envío se da por fallido.
const ACK_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_RECENT: usize = 32;

struct Finished {
    name: String,
    delivered: bool,
    at: Instant,
}

/// Lleva la cuenta de lo enviado hasta que el móvil confirma que lo guardó.
#[derive(Default)]
struct SendLog {
    awaiting: HashMap<u32, (String, Instant)>,
    finished: Vec<Finished>,
}

impl SendLog {
    fn await_confirmation(&mut self, id: u32, name: String, now: Instant) {
        self.awaiting.insert(id, (name, now));
    }

    fn finish(&mut self, name: String, delivered: bool, now: Instant) {
        self.finished.push(Finished { name, delivered, at: now });
        if self.finished.len() > MAX_RECENT {
            self.finished.remove(0);
        }
    }

    fn resolve(&mut self, id: u32, delivered: bool, now: Instant) {
        if let Some((name, _)) = self.awaiting.remove(&id) {
            self.finish(name, delivered, now);
        }
    }

    fn recent(&mut self, now: Instant) -> Vec<RecentFile> {
        let overdue: Vec<u32> = self
            .awaiting
            .iter()
            .filter(|(_, (_, sent))| now.duration_since(*sent) >= ACK_TIMEOUT)
            .map(|(id, _)| *id)
            .collect();
        for id in overdue {
            self.resolve(id, false, now);
        }

        self.finished.retain(|file| now.duration_since(file.at) < RECENT_FOR);
        self.finished
            .iter()
            .rev()
            .map(|file| RecentFile {
                name: file.name.clone(),
                delivered: file.delivered,
                opacity: opacity(now.duration_since(file.at)),
            })
            .collect()
    }
}

fn opacity(age: Duration) -> f32 {
    let remaining = RECENT_FOR.saturating_sub(age);
    (remaining.as_secs_f32() / FADE_FOR.as_secs_f32()).min(1.0)
}

pub struct Integration {
    clipboard_sync: Arc<AtomicBool>,
    audio: AudioMeter,
    outbox: Sender<PathBuf>,
    transfer: Arc<Mutex<Option<Transfer>>>,
    sends: Arc<Mutex<SendLog>>,
}

pub fn start(engine: Arc<Engine>, audio: AudioMeter, window: Arc<crate::window::WindowControl>) -> Integration {
    let clipboard_sync = Arc::new(AtomicBool::new(true));
    let sends = Arc::new(Mutex::new(SendLog::default()));
    if let Some(events) = engine.take_desktop_events() {
        let engine = engine.clone();
        let sync = clipboard_sync.clone();
        let sends = sends.clone();
        std::thread::spawn(move || run(engine, sync, events, sends));
    }
    let (outbox, pending) = channel::<PathBuf>();
    #[cfg(unix)]
    crate::handoff::listen(engine.clone(), outbox.clone(), window);
    #[cfg(not(unix))]
    let _ = window;
    let transfer = Arc::new(Mutex::new(None));
    {
        let engine = engine.clone();
        let transfer = transfer.clone();
        let sends = sends.clone();
        std::thread::spawn(move || {
            for path in pending {
                send_file(&engine, &path, &transfer, &sends);
            }
        });
    }
    Integration {
        clipboard_sync,
        audio,
        outbox,
        transfer,
        sends,
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

    fn recent_files(&self) -> Vec<RecentFile> {
        self.sends.lock().expect("envíos disponibles").recent(Instant::now())
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

/// Envía un archivo y deja constancia de su avance y de su resultado.
fn send_file(engine: &Engine, path: &Path, transfer: &Mutex<Option<Transfer>>, sends: &Mutex<SendLog>) {
    let name = path
        .file_name()
        .map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned());
    if upload(engine, path, &name, transfer, sends).is_none() {
        sends
            .lock()
            .expect("envíos disponibles")
            .finish(name, false, Instant::now());
    }
    *transfer.lock().expect("envío disponible") = None;
}

/// Devuelve `None` si el archivo no llegó a enviarse entero.
fn upload(
    engine: &Engine,
    path: &Path,
    name: &str,
    transfer: &Mutex<Option<Transfer>>,
    sends: &Mutex<SendLog>,
) -> Option<()> {
    let mut file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() {
        return None;
    }
    let size = metadata.len();
    let id = engine.begin_file(name, size)?;

    let mut buffer = vec![0; FILE_CHUNK_BYTES];
    let mut sent = 0_u64;
    loop {
        match file.read(&mut buffer) {
            Ok(0) => {
                // Se registra antes de avisar: la confirmación puede llegar enseguida.
                sends
                    .lock()
                    .expect("envíos disponibles")
                    .await_confirmation(id, name.to_string(), Instant::now());
                engine.end_file(id, true);
                return Some(());
            }
            Ok(read) => {
                if !engine.send_file_chunk(id, buffer[..read].to_vec()) {
                    return None;
                }
                sent += read as u64;
                *transfer.lock().expect("envío disponible") = Some(Transfer {
                    name: name.to_string(),
                    fraction: progress(sent, size),
                });
            }
            Err(_) => {
                engine.end_file(id, false);
                return None;
            }
        }
    }
}

fn progress(sent: u64, size: u64) -> f32 {
    if size == 0 { 1.0 } else { sent as f32 / size as f32 }
}

fn run(engine: Arc<Engine>, sync: Arc<AtomicBool>, events: Receiver<DesktopEvent>, sends: Arc<Mutex<SendLog>>) {
    let mut clipboard = arboard::Clipboard::new().ok();
    let mut last_text: Option<String> = None;

    loop {
        match events.recv_timeout(CLIPBOARD_CHECK_EVERY) {
            Ok(DesktopEvent::Notification(info)) => show_notification(&info),
            Ok(DesktopEvent::Call(call)) => show_call(&call),
            Ok(DesktopEvent::FileReceived(path, name)) => show_file_received(&path, &name),
            Ok(DesktopEvent::FileDelivered(id)) => {
                sends.lock().expect("envíos disponibles").resolve(id, true, Instant::now());
            }
            Ok(DesktopEvent::FileFailed(id)) => {
                sends.lock().expect("envíos disponibles").resolve(id, false, Instant::now());
            }
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

fn show_call(call: &CallInfo) {
    let _ = notify_rust::Notification::new()
        .appname("Puente")
        .summary(&(t().call_from)(&call.caller))
        .body(&escape_markup(&call.app))
        .show();
}

fn show_file_received(path: &Path, name: &str) {
    let _ = notify_rust::Notification::new()
        .appname("Puente")
        .summary(t().file_received_title)
        .body(&escape_markup(&format!("{name}\n{}", path.display())))
        .show();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_send_is_listed_only_once_the_phone_confirms_it() {
        let mut log = SendLog::default();
        let start = Instant::now();
        log.await_confirmation(1, "foto.jpg".to_string(), start);
        assert!(log.recent(start).is_empty());

        log.resolve(1, true, start);
        let shown = log.recent(start);
        assert_eq!(shown.len(), 1);
        assert!(shown[0].delivered);
        assert_eq!(shown[0].opacity, 1.0);
    }

    #[test]
    fn a_send_without_confirmation_ends_up_as_failed() {
        let mut log = SendLog::default();
        let start = Instant::now();
        log.await_confirmation(1, "foto.jpg".to_string(), start);
        let shown = log.recent(start + ACK_TIMEOUT);
        assert_eq!(shown.len(), 1);
        assert!(!shown[0].delivered);
    }

    #[test]
    fn lines_fade_and_then_disappear_newest_first() {
        let mut log = SendLog::default();
        let start = Instant::now();
        log.finish("a".to_string(), true, start);
        log.finish("b".to_string(), true, start + Duration::from_secs(5));

        let shown = log.recent(start + Duration::from_secs(5));
        assert_eq!(shown[0].name, "b");
        assert_eq!(shown[1].name, "a");

        let fading = log.recent(start + RECENT_FOR - FADE_FOR / 2);
        assert!(fading[1].opacity > 0.4 && fading[1].opacity < 0.6);

        let later = log.recent(start + RECENT_FOR);
        assert_eq!(later.len(), 1);
        assert_eq!(later[0].name, "b");
    }

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
