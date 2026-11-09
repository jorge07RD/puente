//! Motor de conexión que usa la interfaz.
//!
//! El móvil pregunta por UDP quién hay en la red, la PC responde y el móvil abre
//! la conexión TLS. El móvil solo se conecta sin intervención a PCs ya
//! emparejadas; emparejar una nueva siempre lo inicia el usuario.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, mpsc as std_mpsc};
use std::time::{Duration, Instant};

use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::runtime::Runtime;
use tokio::sync::{Semaphore, mpsc, oneshot};
use tokio::time::{MissedTickBehavior, interval, timeout};

use crate::pairing::{self, PairPrompt};
use crate::tls::{Secure, TlsContext};
use crate::session::Outgoing;
use crate::settings::SettingsFile;
use crate::telemetry::RateMeter;
use crate::transfers::Incoming;
use crate::wire::{
    Discovery, Frame, Input, MediaInfo, Message, NotificationInfo, PhoneCommand, VideoPacket,
};
use crate::{Error, Fingerprint, Identity, PROTOCOL_VERSION, Peer, PeerStore, Result, Role, session};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const PROBE_EVERY: Duration = Duration::from_secs(3);
const FOUND_EXPIRES: Duration = Duration::from_secs(10);
/// Pocos paquetes en cola: si la red se atrasa es mejor saltar que acumular retraso.
const VIDEO_QUEUE: usize = 8;
const DECODER_QUEUE: usize = 16;
const CONTROL_QUEUE: usize = 256;
const INPUT_QUEUE: usize = 256;
const TASK_QUEUE: usize = 64;
const AUDIO_QUEUE: usize = 16;
const FILE_QUEUE: usize = 8;
const MAX_PENDING_LOGS: usize = 64;
const MAX_LOG_CHARACTERS: usize = 4000;
/// Al llegar a este tamaño el registro empieza de nuevo.
const MAX_LOG_BYTES: u64 = 1 << 20;

pub struct Config {
    pub role: Role,
    pub data_dir: PathBuf,
    /// Dónde se guardan los archivos recibidos.
    pub downloads_dir: PathBuf,
    pub name: String,
    pub port: u16,
    /// Direcciones a las que el móvil pregunta quién hay en la red.
    pub probe_targets: Vec<SocketAddr>,
}

impl Config {
    pub fn new(role: Role, data_dir: PathBuf, name: String) -> Self {
        Self {
            role,
            downloads_dir: data_dir.join("incoming"),
            data_dir,
            name,
            port: crate::PORT,
            probe_targets: vec![SocketAddr::from((Ipv4Addr::BROADCAST, crate::PORT))],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Searching,
    Connecting { name: String },
    Pairing { name: String, code: String, answered: bool },
    Connected { name: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub name: String,
    pub fingerprint: Fingerprint,
    pub address: SocketAddr,
    pub paired: bool,
}

#[derive(Clone, Debug)]
pub struct View {
    pub status: Status,
    pub found: Vec<Found>,
    pub paired: Vec<Peer>,
    pub last_error: Option<String>,
    /// PC: está llegando vídeo. Móvil: la pantalla se está compartiendo.
    pub mirroring: bool,
    /// El permiso de Accesibilidad del móvil está activo.
    pub input_enabled: bool,
    /// Móvil: el permiso de acceso a notificaciones está activo.
    pub notifications_enabled: bool,
    pub notifications: Vec<NotificationInfo>,
    pub battery: Option<(u8, bool)>,
    pub media: Option<MediaInfo>,
    pub last_file: Option<String>,
    /// Ida y vuelta de la conexión, medido con los pings.
    pub round_trip_ms: Option<u32>,
    /// PC: paquetes de vídeo por segundo que están llegando.
    pub video_fps: Option<u32>,
    /// PC: megabits por segundo de vídeo que están llegando.
    pub video_mbps: Option<f32>,
    pub animations: bool,
    /// PC: el audio del móvil se reproduce aquí.
    pub audio_enabled: bool,
}

/// PC: lo que hay que reflejar en el escritorio.
#[derive(Clone, Debug, PartialEq)]
pub enum DesktopEvent {
    Notification(NotificationInfo),
    Clipboard(String),
    FileReceived(PathBuf, String),
    /// El móvil confirmó que guardó este envío.
    FileDelivered(u32),
    /// El móvil no pudo guardar este envío.
    FileFailed(u32),
}

enum Command {
    Connect(Fingerprint),
}

struct SessionHandle {
    peer: Fingerprint,
    control: mpsc::Sender<Message>,
    video: mpsc::Sender<VideoPacket>,
    audio: mpsc::Sender<Vec<u8>>,
    files: mpsc::Sender<Frame>,
}

/// Lo que el móvil debe volver a contar cada vez que se conecta.
#[derive(Default)]
struct PhoneState {
    battery: Option<(u8, bool)>,
    media: Option<MediaInfo>,
    notifications: Vec<NotificationInfo>,
    /// Registros escritos mientras no había PC conectada.
    logs: Vec<String>,
}

struct SessionState {
    peer: Fingerprint,
    waiting_for_key: bool,
    /// Tras el aviso de fin, lo que aún llegue son restos de la cola del móvil.
    stream_ended: bool,
    video_rate: RateMeter,
    incoming: Incoming,
}

/// Estado del envío de vídeo en el móvil.
struct VideoOut {
    /// Hasta el próximo fotograma clave no sirve de nada enviar los demás.
    need_key: AtomicBool,
    /// Parámetros del codificador, para quien se conecte con el vídeo en marcha.
    config: Mutex<Option<VideoPacket>>,
}

struct Shared {
    role: Role,
    identity: Identity,
    tls: TlsContext,
    store: Mutex<PeerStore>,
    view: Mutex<View>,
    pending_answer: Mutex<Option<oneshot::Sender<bool>>>,
    attempt_active: AtomicBool,
    session: Mutex<Option<SessionHandle>>,
    video_in: std_mpsc::SyncSender<VideoPacket>,
    video_out: VideoOut,
    input_out: std_mpsc::SyncSender<String>,
    task_out: std_mpsc::SyncSender<String>,
    audio_in: std_mpsc::SyncSender<Vec<u8>>,
    desktop_out: std_mpsc::Sender<DesktopEvent>,
    phone_state: Mutex<PhoneState>,
    downloads_dir: PathBuf,
    phone_log: PathBuf,
    settings: Mutex<SettingsFile>,
    next_file_id: AtomicU32,
    repaint: Box<dyn Fn() + Send + Sync>,
}

impl Shared {
    fn update(&self, change: impl FnOnce(&mut View)) {
        change(&mut self.view.lock().expect("vista disponible"));
        (self.repaint)();
    }

    fn paired(&self) -> Vec<Peer> {
        self.store.lock().expect("almacén disponible").list()
    }

    fn is_paired(&self, fingerprint: Fingerprint) -> bool {
        self.store
            .lock()
            .expect("almacén disponible")
            .is_paired(fingerprint)
    }

    fn forget_peer(&self, fingerprint: Fingerprint) -> Result<()> {
        self.store
            .lock()
            .expect("almacén disponible")
            .remove(fingerprint)?;
        let paired = self.paired();
        self.update(|view| {
            view.paired = paired;
            for found in &mut view.found {
                if found.fingerprint == fingerprint {
                    found.paired = false;
                }
            }
        });
        Ok(())
    }

    fn send_control(&self, message: Message) {
        if let Some(session) = self.session.lock().expect("sesión disponible").as_ref() {
            let _ = session.control.try_send(message);
        }
    }

    fn on_frame(&self, frame: Frame, state: &mut SessionState) {
        match frame {
            Frame::Control(message) => self.on_control(message, state),
            Frame::Video(packet) => {
                if state.stream_ended {
                    if !packet.is_config() {
                        return;
                    }
                    state.stream_ended = false;
                }
                self.measure_video(&packet, state);
                self.on_video(packet, &mut state.waiting_for_key);
            }
            Frame::Audio(samples) => {
                let _ = self.audio_in.try_send(samples);
            }
            Frame::File { id, data } => {
                if state.incoming.chunk(id, &data).is_err() {
                    state.incoming.fail(id);
                    self.send_control(Message::FileFailed { id });
                }
            }
        }
    }

    fn measure_video(&self, packet: &VideoPacket, state: &mut SessionState) {
        if packet.is_config() {
            return;
        }
        if let Some(rate) = state.video_rate.record(Instant::now(), packet.data.len()) {
            self.update(|view| {
                view.video_fps = Some(rate.per_second);
                view.video_mbps = Some(rate.megabits_per_second);
            });
        }
    }

    fn on_control(&self, message: Message, state: &mut SessionState) {
        match message {
            Message::RequestKeyframe => {
                self.video_out.need_key.store(true, Ordering::SeqCst);
            }
            Message::MirrorStopped => {
                state.stream_ended = true;
                self.update(|view| {
                    view.mirroring = false;
                    view.video_fps = None;
                    view.video_mbps = None;
                });
            }
            Message::Input(input) => self.queue_input(input),
            Message::InputState { enabled } => {
                self.update(|view| view.input_enabled = enabled);
            }
            Message::FileOffer { id, name, size } => {
                if state.incoming.offer(id, &name, size).is_err() {
                    self.send_control(Message::FileFailed { id });
                }
            }
            Message::FileDone { id } => self.on_file_done(id, state),
            Message::FileFailed { id } => {
                // Si no se estaba recibiendo, es un envío nuestro que el otro lado rechazó.
                if !state.incoming.fail(id) && self.role == Role::Desktop {
                    let _ = self.desktop_out.send(DesktopEvent::FileFailed(id));
                }
                self.update(|view| view.last_file = Some("Falló un envío de archivo".to_string()));
            }
            Message::FileSaved { id } => {
                if self.role == Role::Desktop {
                    let _ = self.desktop_out.send(DesktopEvent::FileDelivered(id));
                }
            }
            Message::Clipboard { text } => self.on_clipboard(text),
            Message::Unpair => {
                let _ = self.forget_peer(state.peer);
            }
            other => match self.role {
                Role::Desktop => self.on_phone_report(other),
                Role::Phone => {
                    if let Message::Command(command) = other
                        && let Ok(task) = serde_json::to_string(&command)
                    {
                        let _ = self.task_out.try_send(task);
                    }
                }
            },
        }
    }

    fn on_clipboard(&self, text: String) {
        match self.role {
            Role::Desktop => {
                let _ = self.desktop_out.send(DesktopEvent::Clipboard(text));
            }
            Role::Phone => {
                let task = serde_json::json!({ "command": "set_clipboard", "text": text });
                let _ = self.task_out.try_send(task.to_string());
            }
        }
    }

    fn on_file_done(&self, id: u32, state: &mut SessionState) {
        let received = match state.incoming.finish(id) {
            Ok(received) => received,
            Err(_) => {
                self.send_control(Message::FileFailed { id });
                self.update(|view| view.last_file = Some("Un archivo llegó incompleto".to_string()));
                return;
            }
        };
        self.send_control(Message::FileSaved { id });
        match self.role {
            Role::Desktop => {
                let _ = self
                    .desktop_out
                    .send(DesktopEvent::FileReceived(received.path, received.name.clone()));
            }
            Role::Phone => {
                let task = serde_json::json!({
                    "command": "save_file",
                    "path": received.path,
                    "name": received.name,
                });
                let _ = self.task_out.try_send(task.to_string());
            }
        }
        self.update(|view| view.last_file = Some(format!("Recibido: {}", received.name)));
    }

    fn append_phone_log(&self, text: &str) {
        use std::io::Write;

        let too_big = std::fs::metadata(&self.phone_log).is_ok_and(|file| file.len() > MAX_LOG_BYTES);
        if too_big {
            let _ = std::fs::remove_file(&self.phone_log);
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.phone_log);
        if let Ok(mut file) = file {
            let limited: String = text.chars().take(MAX_LOG_CHARACTERS).collect();
            let _ = writeln!(file, "{limited}");
        }
    }

    /// PC: lo que cuenta el móvil sobre su estado.
    fn on_phone_report(&self, message: Message) {
        match message {
            Message::Notification(info) => {
                let shown = info.clone();
                self.update(|view| {
                    view.notifications.retain(|other| other.key != info.key);
                    view.notifications.insert(0, info);
                });
                let _ = self.desktop_out.send(DesktopEvent::Notification(shown));
            }
            Message::NotificationRemoved { key } => {
                self.update(|view| view.notifications.retain(|other| other.key != key));
            }
            Message::Battery { percent, charging } => {
                self.update(|view| view.battery = Some((percent, charging)));
            }
            Message::Media { info } => self.update(|view| view.media = info),
            Message::Log { text } => self.append_phone_log(&text),
            _ => {}
        }
    }

    /// El móvil solo obedece mientras se comparte la pantalla: quien lo controla
    /// tiene que poder ver lo que hace, y "Detener" corta también el control.
    fn queue_input(&self, input: Input) {
        if self.role != Role::Phone {
            return;
        }
        let allowed = {
            let view = self.view.lock().expect("vista disponible");
            view.mirroring && view.input_enabled
        };
        if allowed {
            self.deliver_input(&input);
        }
    }

    fn deliver_input(&self, input: &Input) {
        if let Ok(text) = serde_json::to_string(input) {
            let _ = self.input_out.try_send(text);
        }
    }

    fn on_video(&self, packet: VideoPacket, waiting_for_key: &mut bool) {
        // Los parámetros del codificador llegan al conectar aunque no haya imagen.
        if !packet.is_config() && !self.view.lock().expect("vista disponible").mirroring {
            self.update(|view| view.mirroring = true);
        }
        if *waiting_for_key && !packet.is_key() && !packet.is_config() {
            return;
        }
        if packet.is_key() {
            *waiting_for_key = false;
        }
        if self.video_in.try_send(packet).is_err() {
            *waiting_for_key = true;
            self.send_control(Message::RequestKeyframe);
        }
    }
}

pub struct Engine {
    shared: Arc<Shared>,
    commands: mpsc::UnboundedSender<Command>,
    video_receiver: Mutex<Option<std_mpsc::Receiver<VideoPacket>>>,
    input_receiver: Mutex<std_mpsc::Receiver<String>>,
    task_receiver: Mutex<std_mpsc::Receiver<String>>,
    audio_receiver: Mutex<Option<std_mpsc::Receiver<Vec<u8>>>>,
    desktop_receiver: Mutex<Option<std_mpsc::Receiver<DesktopEvent>>>,
    _runtime: Runtime,
}

impl Engine {
    pub fn start(config: Config, repaint: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        let identity = Identity::load_or_create(&config.data_dir, &config.name)?;
        let store = PeerStore::load(config.data_dir.join("peers.json"))?;
        let tls = TlsContext::new(&identity)?;
        let settings = SettingsFile::load(config.data_dir.join("settings.json"));
        let view = View {
            status: Status::Searching,
            found: Vec::new(),
            paired: store.list(),
            last_error: None,
            mirroring: false,
            input_enabled: false,
            notifications_enabled: false,
            notifications: Vec::new(),
            battery: None,
            media: None,
            last_file: None,
            round_trip_ms: None,
            video_fps: None,
            video_mbps: None,
            animations: settings.current.animations,
            audio_enabled: settings.current.audio,
        };
        let (task_out, task_receiver) = std_mpsc::sync_channel(TASK_QUEUE);
        let (audio_in, audio_receiver) = std_mpsc::sync_channel(AUDIO_QUEUE);
        let (desktop_out, desktop_receiver) = std_mpsc::channel();
        let (input_out, input_receiver) = std_mpsc::sync_channel(INPUT_QUEUE);
        let (video_in, video_receiver) = std_mpsc::sync_channel(DECODER_QUEUE);
        let shared = Arc::new(Shared {
            role: config.role,
            identity,
            tls,
            store: Mutex::new(store),
            view: Mutex::new(view),
            pending_answer: Mutex::new(None),
            attempt_active: AtomicBool::new(false),
            session: Mutex::new(None),
            video_in,
            video_out: VideoOut {
                need_key: AtomicBool::new(true),
                config: Mutex::new(None),
            },
            input_out,
            task_out,
            audio_in,
            desktop_out,
            phone_state: Mutex::new(PhoneState::default()),
            downloads_dir: config.downloads_dir.clone(),
            phone_log: config.data_dir.join("phone.log"),
            settings: Mutex::new(settings),
            next_file_id: AtomicU32::new(1),
            repaint: Box::new(repaint),
        });

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let (commands, receiver) = mpsc::unbounded_channel();

        match config.role {
            Role::Desktop => {
                let (listener, udp) = runtime.block_on(async {
                    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.port));
                    Ok::<_, Error>((TcpListener::bind(address).await?, UdpSocket::bind(address).await?))
                })?;
                runtime.spawn(answer_probes(shared.clone(), udp, config.port));
                runtime.spawn(accept_connections(shared.clone(), listener));
            }
            Role::Phone => {
                let udp = runtime.block_on(async {
                    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
                    socket.set_broadcast(true)?;
                    Ok::<_, Error>(socket)
                })?;
                runtime.spawn(search_for_desktop(
                    shared.clone(),
                    udp,
                    receiver,
                    config.port,
                    config.probe_targets,
                ));
            }
        }

        Ok(Self {
            shared,
            commands,
            video_receiver: Mutex::new(Some(video_receiver)),
            input_receiver: Mutex::new(input_receiver),
            task_receiver: Mutex::new(task_receiver),
            audio_receiver: Mutex::new(Some(audio_receiver)),
            desktop_receiver: Mutex::new(Some(desktop_receiver)),
            _runtime: runtime,
        })
    }

    pub fn fingerprint(&self) -> Fingerprint {
        self.shared.identity.fingerprint
    }

    pub fn view(&self) -> View {
        self.shared.view.lock().expect("vista disponible").clone()
    }

    /// El móvil inicia la conexión con una PC encontrada.
    pub fn connect(&self, fingerprint: Fingerprint) {
        let _ = self.commands.send(Command::Connect(fingerprint));
    }

    /// Responde al código que se está mostrando.
    pub fn answer_pairing(&self, accepted: bool) {
        let sender = self
            .shared
            .pending_answer
            .lock()
            .expect("respuesta disponible")
            .take();
        if let Some(sender) = sender {
            let _ = sender.send(accepted);
            self.shared.update(|view| {
                if let Status::Pairing { answered, .. } = &mut view.status {
                    *answered = true;
                }
            });
        }
    }

    /// PC: entrega el vídeo recibido. Solo se puede tomar una vez.
    pub fn take_video_receiver(&self) -> Option<std_mpsc::Receiver<VideoPacket>> {
        self.video_receiver
            .lock()
            .expect("receptor disponible")
            .take()
    }

    /// PC: pide un fotograma clave, por ejemplo tras un error de decodificación.
    pub fn request_keyframe(&self) {
        self.shared.send_control(Message::RequestKeyframe);
    }

    /// Móvil: envía un paquete del codificador. Devuelve `true` si hace falta
    /// que el codificador genere un fotograma clave.
    pub fn send_video(&self, packet: VideoPacket) -> bool {
        let out = &self.shared.video_out;
        if packet.is_config() {
            *out.config.lock().expect("configuración disponible") = Some(packet.clone());
        }

        let session = self.shared.session.lock().expect("sesión disponible");
        let Some(session) = session.as_ref() else {
            out.need_key.store(true, Ordering::SeqCst);
            return false;
        };

        if out.need_key.load(Ordering::SeqCst) && !packet.is_key() && !packet.is_config() {
            return true;
        }
        if packet.is_key() {
            out.need_key.store(false, Ordering::SeqCst);
        }
        if session.video.try_send(packet).is_err() {
            out.need_key.store(true, Ordering::SeqCst);
            return true;
        }
        false
    }

    /// Móvil: informa de que la captura empezó o terminó.
    pub fn set_mirroring(&self, active: bool) {
        if !active {
            self.shared.deliver_input(&Input::Cancel);
            *self
                .shared
                .video_out
                .config
                .lock()
                .expect("configuración disponible") = None;
            self.shared.send_control(Message::MirrorStopped);
        }
        self.shared.update(|view| view.mirroring = active);
    }

    /// PC: envía una orden al móvil.
    pub fn send_input(&self, input: Input) {
        self.shared.send_control(Message::Input(input));
    }

    /// Móvil: informa de si el servicio de Accesibilidad está activo.
    pub fn set_input_enabled(&self, enabled: bool) {
        self.shared.update(|view| view.input_enabled = enabled);
        self.shared.send_control(Message::InputState { enabled });
    }

    /// Móvil: espera la siguiente orden, en JSON, hasta `wait`.
    pub fn next_input(&self, wait: Duration) -> Option<String> {
        self.input_receiver
            .lock()
            .expect("receptor disponible")
            .recv_timeout(wait)
            .ok()
    }

    /// PC: audio recibido. Solo se puede tomar una vez.
    pub fn take_audio_receiver(&self) -> Option<std_mpsc::Receiver<Vec<u8>>> {
        self.audio_receiver.lock().expect("receptor disponible").take()
    }

    /// PC: eventos para el escritorio. Solo se pueden tomar una vez.
    pub fn take_desktop_events(&self) -> Option<std_mpsc::Receiver<DesktopEvent>> {
        self.desktop_receiver.lock().expect("receptor disponible").take()
    }

    /// Deshace el emparejamiento con el dispositivo conectado, en los dos lados.
    pub fn unpair(&self) -> Result<()> {
        let peer = {
            let session = self.shared.session.lock().expect("sesión disponible");
            let Some(session) = session.as_ref() else {
                return Ok(());
            };
            let _ = session.control.try_send(Message::Unpair);
            session.peer
        };
        self.shared.forget_peer(peer)
    }

    pub fn audio_enabled(&self) -> bool {
        self.shared.view.lock().expect("vista disponible").audio_enabled
    }

    pub fn set_audio_enabled(&self, enabled: bool) {
        {
            let mut settings = self.shared.settings.lock().expect("ajustes disponibles");
            settings.current.audio = enabled;
            let _ = settings.save();
        }
        self.shared.update(|view| view.audio_enabled = enabled);
    }

    pub fn set_animations(&self, enabled: bool) {
        {
            let mut settings = self.shared.settings.lock().expect("ajustes disponibles");
            settings.current.animations = enabled;
            let _ = settings.save();
        }
        self.shared.update(|view| view.animations = enabled);
    }

    pub fn is_connected(&self) -> bool {
        self.shared.session.lock().expect("sesión disponible").is_some()
    }

    /// Móvil: envía audio capturado. Se descarta si la red va atrasada.
    pub fn send_audio(&self, samples: Vec<u8>) {
        if let Some(session) = self.shared.session.lock().expect("sesión disponible").as_ref() {
            let _ = session.audio.try_send(samples);
        }
    }

    /// Móvil: espera la siguiente tarea para Android, en JSON, hasta `wait`.
    pub fn next_task(&self, wait: Duration) -> Option<String> {
        self.task_receiver
            .lock()
            .expect("receptor disponible")
            .recv_timeout(wait)
            .ok()
    }

    pub fn send_command(&self, command: PhoneCommand) {
        self.shared.send_control(Message::Command(command));
    }

    pub fn send_clipboard(&self, text: String) {
        self.shared.send_control(Message::Clipboard { text });
    }

    pub fn set_notifications_enabled(&self, enabled: bool) {
        self.shared.update(|view| view.notifications_enabled = enabled);
    }

    fn phone_state(&self) -> std::sync::MutexGuard<'_, PhoneState> {
        self.shared.phone_state.lock().expect("estado disponible")
    }

    pub fn post_notification(&self, info: NotificationInfo) {
        {
            let mut state = self.phone_state();
            state.notifications.retain(|other| other.key != info.key);
            state.notifications.push(info.clone());
        }
        self.shared.send_control(Message::Notification(info));
    }

    /// Móvil: deja una línea en el registro que guarda la PC.
    pub fn log(&self, text: String) {
        if self.is_connected() {
            self.shared.send_control(Message::Log { text });
            return;
        }
        let mut state = self.phone_state();
        if state.logs.len() < MAX_PENDING_LOGS {
            state.logs.push(text);
        }
    }

    pub fn remove_notification(&self, key: String) {
        self.phone_state().notifications.retain(|other| other.key != key);
        self.shared.send_control(Message::NotificationRemoved { key });
    }

    pub fn set_battery(&self, percent: u8, charging: bool) {
        self.phone_state().battery = Some((percent, charging));
        self.shared.send_control(Message::Battery { percent, charging });
    }

    pub fn set_media(&self, info: Option<MediaInfo>) {
        self.phone_state().media = info.clone();
        self.shared.send_control(Message::Media { info });
    }

    fn files_sender(&self) -> Option<mpsc::Sender<Frame>> {
        let session = self.shared.session.lock().expect("sesión disponible");
        session.as_ref().map(|session| session.files.clone())
    }

    /// Anuncia un archivo. Las tres funciones de archivos bloquean si la red va
    /// lenta, así que deben llamarse fuera del motor, desde un hilo propio.
    pub fn begin_file(&self, name: &str, size: u64) -> Option<u32> {
        let id = self.shared.next_file_id.fetch_add(1, Ordering::SeqCst);
        let offer = Message::FileOffer {
            id,
            name: name.to_string(),
            size,
        };
        self.files_sender()?
            .blocking_send(Frame::Control(offer))
            .ok()?;
        Some(id)
    }

    pub fn send_file_chunk(&self, id: u32, data: Vec<u8>) -> bool {
        self.files_sender()
            .is_some_and(|files| files.blocking_send(Frame::File { id, data }).is_ok())
    }

    pub fn end_file(&self, id: u32, success: bool) {
        let message = if success {
            Message::FileDone { id }
        } else {
            Message::FileFailed { id }
        };
        if let Some(files) = self.files_sender() {
            let _ = files.blocking_send(Frame::Control(message));
        }
    }

    pub fn forget(&self, fingerprint: Fingerprint) -> Result<()> {
        self.shared.forget_peer(fingerprint)
    }
}

async fn answer_probes(shared: Arc<Shared>, socket: UdpSocket, port: u16) {
    let announce = Discovery::Announce {
        protocol: PROTOCOL_VERSION,
        name: shared.identity.name.clone(),
        port,
        fingerprint: shared.identity.fingerprint,
    };
    let Ok(reply) = serde_json::to_vec(&announce) else {
        return;
    };

    let mut buffer = [0; 1024];
    loop {
        let Ok((length, source)) = socket.recv_from(&mut buffer).await else {
            continue;
        };
        let is_probe = matches!(
            serde_json::from_slice(&buffer[..length]),
            Ok(Discovery::Probe { protocol: PROTOCOL_VERSION })
        );
        if is_probe {
            let _ = socket.send_to(&reply, source).await;
        }
    }
}

async fn accept_connections(shared: Arc<Shared>, listener: TcpListener) {
    let slot = Arc::new(Semaphore::new(1));
    loop {
        let Ok((tcp, address)) = listener.accept().await else {
            continue;
        };
        let Ok(permit) = slot.clone().try_acquire_owned() else {
            continue;
        };
        let shared = shared.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let result = accept_one(&shared, tcp, address).await;
            finish(&shared, result);
        });
    }
}

async fn accept_one(shared: &Arc<Shared>, tcp: TcpStream, address: SocketAddr) -> Result<()> {
    let secure = timeout(CONNECT_TIMEOUT, shared.tls.accept(tcp))
        .await
        .map_err(|_| Error::Timeout)??;
    drive(shared, secure, address).await
}

async fn search_for_desktop(
    shared: Arc<Shared>,
    socket: UdpSocket,
    mut commands: mpsc::UnboundedReceiver<Command>,
    port: u16,
    targets: Vec<SocketAddr>,
) {
    let Ok(probe) = serde_json::to_vec(&Discovery::Probe {
        protocol: PROTOCOL_VERSION,
    }) else {
        return;
    };
    let mut seen: Vec<(Found, Instant)> = Vec::new();
    let mut ticker = interval(PROBE_EVERY);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut buffer = [0; 1024];

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                seen.retain(|(_, at)| at.elapsed() < FOUND_EXPIRES);
                publish_found(&shared, &seen);
                if shared.attempt_active.load(Ordering::SeqCst) {
                    continue;
                }
                for target in &targets {
                    let _ = socket.send_to(&probe, target).await;
                }
                let remembered = shared.paired().into_iter().filter_map(|peer| peer.last_ip);
                for ip in remembered {
                    let _ = socket.send_to(&probe, SocketAddr::new(ip, port)).await;
                }
            }
            received = socket.recv_from(&mut buffer) => {
                let Ok((length, source)) = received else { continue };
                let Ok(Discovery::Announce { protocol: PROTOCOL_VERSION, name, port, fingerprint }) =
                    serde_json::from_slice(&buffer[..length]) else { continue };
                let found = Found {
                    name,
                    fingerprint,
                    address: SocketAddr::new(source.ip(), port),
                    paired: shared.is_paired(fingerprint),
                };
                seen.retain(|(other, _)| other.fingerprint != fingerprint);
                seen.push((found.clone(), Instant::now()));
                publish_found(&shared, &seen);
                if found.paired {
                    start_attempt(&shared, found);
                }
            }
            Some(Command::Connect(fingerprint)) = commands.recv() => {
                let target = seen.iter().find(|(found, _)| found.fingerprint == fingerprint);
                if let Some((found, _)) = target {
                    start_attempt(&shared, found.clone());
                }
            }
        }
    }
}

fn publish_found(shared: &Shared, seen: &[(Found, Instant)]) {
    let found: Vec<Found> = seen.iter().map(|(found, _)| found.clone()).collect();
    let changed = shared.view.lock().expect("vista disponible").found != found;
    if changed {
        shared.update(|view| view.found = found);
    }
}

fn start_attempt(shared: &Arc<Shared>, found: Found) {
    if shared.attempt_active.swap(true, Ordering::SeqCst) {
        return;
    }
    shared.update(|view| {
        view.status = Status::Connecting {
            name: found.name.clone(),
        };
    });
    let shared = shared.clone();
    tokio::spawn(async move {
        let result = connect_to(&shared, &found).await;
        finish(&shared, result);
        shared.attempt_active.store(false, Ordering::SeqCst);
    });
}

async fn connect_to(shared: &Arc<Shared>, found: &Found) -> Result<()> {
    let secure = timeout(CONNECT_TIMEOUT, shared.tls.connect(found.address))
        .await
        .map_err(|_| Error::Timeout)??;
    // El anuncio UDP no está autenticado: si quien responde no tiene la huella
    // anunciada, se corta antes de que pueda mostrar un código en el móvil.
    if secure.peer != found.fingerprint {
        return Err(Error::Protocol(
            "La PC no coincide con la que se anunció en la red".to_string(),
        ));
    }
    drive(shared, secure, found.address).await
}

async fn drive(shared: &Arc<Shared>, secure: Secure, address: SocketAddr) -> Result<()> {
    let Secure { mut stream, peer } = secure;

    let prompt_shared = shared.clone();
    let prompt = move |request: PairPrompt| {
        let (sender, receiver) = oneshot::channel();
        *prompt_shared
            .pending_answer
            .lock()
            .expect("respuesta disponible") = Some(sender);
        prompt_shared.update(|view| {
            view.status = Status::Pairing {
                name: request.peer_name,
                code: request.code,
                answered: false,
            };
        });
        receiver
    };

    let name = pairing::establish(
        &mut stream,
        shared.role,
        &shared.identity,
        peer,
        &shared.store,
        prompt,
    )
    .await?;

    shared
        .store
        .lock()
        .expect("almacén disponible")
        .remember_ip(peer, address.ip())?;
    let paired = shared.paired();
    shared.update(|view| {
        view.status = Status::Connected { name };
        view.paired = paired;
        view.last_error = None;
    });

    let (control, control_rx) = mpsc::channel(CONTROL_QUEUE);
    let (video, video_rx) = mpsc::channel(VIDEO_QUEUE);
    let (audio, audio_rx) = mpsc::channel(AUDIO_QUEUE);
    let (files, files_rx) = mpsc::channel(FILE_QUEUE);
    if shared.role == Role::Phone {
        let config = shared
            .video_out
            .config
            .lock()
            .expect("configuración disponible")
            .clone();
        if let Some(config) = config {
            let _ = video.try_send(config);
        }
        shared.video_out.need_key.store(true, Ordering::SeqCst);
        let enabled = shared.view.lock().expect("vista disponible").input_enabled;
        let _ = control.try_send(Message::InputState { enabled });

        let mut state = shared.phone_state.lock().expect("estado disponible");
        for text in state.logs.drain(..) {
            let _ = control.try_send(Message::Log { text });
        }
        if let Some((percent, charging)) = state.battery {
            let _ = control.try_send(Message::Battery { percent, charging });
        }
        let _ = control.try_send(Message::Media {
            info: state.media.clone(),
        });
        for info in &state.notifications {
            let _ = control.try_send(Message::Notification(info.clone()));
        }
    }
    *shared.session.lock().expect("sesión disponible") = Some(SessionHandle {
        peer,
        control,
        video,
        audio,
        files,
    });

    let outgoing = Outgoing {
        control: control_rx,
        video: video_rx,
        audio: audio_rx,
        files: files_rx,
    };
    let events = shared.clone();
    let mut state = SessionState {
        peer,
        waiting_for_key: true,
        stream_ended: false,
        video_rate: RateMeter::default(),
        incoming: Incoming::new(shared.downloads_dir.clone()),
    };
    // Un corte de la sesión es normal y la interfaz ya lo refleja al volver a buscar.
    let clock = shared.clone();
    let _ = session::run(
        stream,
        outgoing,
        move |frame| events.on_frame(frame, &mut state),
        move |round_trip| {
            let milliseconds = u32::try_from(round_trip.as_millis()).unwrap_or(u32::MAX);
            clock.update(|view| view.round_trip_ms = Some(milliseconds));
        },
    )
    .await;

    *shared.session.lock().expect("sesión disponible") = None;
    shared.update(|view| {
        view.round_trip_ms = None;
        view.video_fps = None;
        view.video_mbps = None;
    });
    match shared.role {
        Role::Desktop => shared.update(|view| {
            view.mirroring = false;
            view.input_enabled = false;
            view.notifications.clear();
            view.battery = None;
            view.media = None;
        }),
        // Un gesto a medias no puede quedarse "pulsando" si se corta la conexión.
        Role::Phone => shared.deliver_input(&Input::Cancel),
    }
    Ok(())
}

fn finish(shared: &Shared, result: Result<()>) {
    *shared
        .pending_answer
        .lock()
        .expect("respuesta disponible") = None;
    shared.update(|view| {
        view.status = Status::Searching;
        if let Err(error) = result {
            view.last_error = Some(error.to_string());
        }
    });
}
