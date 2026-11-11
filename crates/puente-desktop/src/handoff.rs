//! `puente --send <archivos>` entrega archivos a la ventana que ya está abierta,
//! por un socket en `XDG_RUNTIME_DIR`, que solo puede abrir el propio usuario.
//! Por el mismo socket, abrir Puente otra vez muestra la ventana existente.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use puente_core::Engine;
use puente_ui::i18n::t;

use crate::window::WindowControl;

const CONNECTED: &str = "conectado";
const NO_PHONE: &str = "sin móvil";
/// Las rutas que se envían son absolutas, así que no pueden confundirse con esto.
const SHOW: &str = ":show";
const SHOWN: &str = "visible";

fn socket_path() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR").map(|dir| PathBuf::from(dir).join("puente.sock"))
}

/// La ventana abierta escucha. Si ya hay otra escuchando, la deja en paz.
pub fn listen(engine: Arc<Engine>, outbox: Sender<PathBuf>, window: Arc<WindowControl>) {
    let Some(path) = socket_path() else { return };
    if UnixStream::connect(&path).is_ok() {
        return;
    }
    let _ = std::fs::remove_file(&path);
    let Ok(listener) = UnixListener::bind(&path) else { return };
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = serve(stream, &engine, &outbox, &window);
        }
    });
}

/// Una ruta por línea; una línea vacía cierra la lista.
fn serve(stream: UnixStream, engine: &Engine, outbox: &Sender<PathBuf>, window: &WindowControl) -> io::Result<()> {
    let mut reply = stream.try_clone()?;
    let mut paths = Vec::new();
    for line in BufReader::new(stream).lines() {
        let line = line?;
        if line.is_empty() {
            break;
        }
        if line == SHOW {
            window.show();
            return writeln!(reply, "{SHOWN}");
        }
        paths.push(PathBuf::from(line));
    }
    if !engine.is_connected() {
        return writeln!(reply, "{NO_PHONE}");
    }
    for path in paths.into_iter().filter(|path| path.is_file()) {
        let _ = outbox.send(path);
    }
    writeln!(reply, "{CONNECTED}")
}

pub fn is_running() -> bool {
    socket_path().is_some_and(|socket| UnixStream::connect(socket).is_ok())
}

/// Si ya hay una ventana abierta, la muestra y devuelve `true`.
pub fn show_existing() -> bool {
    let Some(socket) = socket_path() else { return false };
    let Ok(mut stream) = UnixStream::connect(&socket) else { return false };
    if writeln!(stream, "{SHOW}").is_err() {
        return false;
    }
    let mut answer = String::new();
    BufReader::new(stream).read_line(&mut answer).is_ok() && answer.trim() == SHOWN
}

pub fn send(paths: &[String]) -> Result<(), String> {
    let socket = socket_path().ok_or("XDG_RUNTIME_DIR no está definido")?;
    let mut stream = UnixStream::connect(&socket).map_err(|_| t().not_open.to_string())?;
    let mut list = String::new();
    for path in paths {
        let full = std::fs::canonicalize(path).map_err(|error| format!("{path}: {error}"))?;
        if !full.is_file() {
            return Err(format!("{path}: {}", t().not_a_file));
        }
        let text = full.to_str().ok_or_else(|| format!("{path}: {}", t().invalid_name))?;
        if text.contains('\n') {
            return Err(format!("{path}: {}", t().invalid_name));
        }
        list.push_str(text);
        list.push('\n');
    }
    list.push('\n');
    stream.write_all(list.as_bytes()).map_err(|error| error.to_string())?;

    let mut answer = String::new();
    BufReader::new(stream)
        .read_line(&mut answer)
        .map_err(|error| error.to_string())?;
    match answer.trim() {
        CONNECTED => Ok(()),
        NO_PHONE => Err(t().phone_not_connected.to_string()),
        _ => Err(t().no_answer.to_string()),
    }
}
