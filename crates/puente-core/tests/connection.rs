//! Pruebas de conexión real por loopback, con TLS y sockets de verdad.

use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::sync::Mutex;
use std::thread::sleep;
use std::time::{Duration, Instant};

use puente_core::pairing::{PairPrompt, establish};
use puente_core::tls::TlsContext;
use puente_core::wire::{
    Input, MediaAction, Message, NavAction, NotificationInfo, PhoneCommand, VideoPacket, read_message,
    write_message,
};
use puente_core::DesktopEvent;
use puente_core::{Config, Engine, Error, Identity, PeerStore, Role, Status, View};
use tokio::sync::oneshot;

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn engine(role: Role, dir: &Path, port: u16) -> Engine {
    let mut config = Config::new(role, dir.to_path_buf(), format!("{role:?}"));
    config.port = port;
    config.probe_targets = vec![SocketAddr::from(([127, 0, 0, 1], port))];
    Engine::start(config, || {}).unwrap()
}

fn wait_for(engine: &Engine, what: &str, check: impl Fn(&View) -> bool) -> View {
    let start = Instant::now();
    loop {
        let view = engine.view();
        if check(&view) {
            return view;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "esperando {what}: {view:?}");
        sleep(Duration::from_millis(20));
    }
}

fn pairing_code(view: &View) -> Option<String> {
    match &view.status {
        Status::Pairing { code, .. } => Some(code.clone()),
        _ => None,
    }
}

fn connected(view: &View) -> bool {
    matches!(view.status, Status::Connected { .. })
}

/// Empareja una PC y un móvil, confirmando el código en ambos.
fn pair(desktop: &Engine, phone: &Engine) {
    let found = wait_for(phone, "la PC", |view| !view.found.is_empty());
    phone.connect(found.found[0].fingerprint);

    let phone_view = wait_for(phone, "código en el móvil", |v| pairing_code(v).is_some());
    let desktop_view = wait_for(desktop, "código en la PC", |v| pairing_code(v).is_some());
    assert_eq!(pairing_code(&phone_view), pairing_code(&desktop_view));

    desktop.answer_pairing(true);
    phone.answer_pairing(true);
    wait_for(desktop, "conexión en la PC", connected);
    wait_for(phone, "conexión en el móvil", connected);
}

#[test]
fn pairing_then_reconnecting_needs_no_prompt() {
    let desktop_dir = tempfile::tempdir().unwrap();
    let phone_dir = tempfile::tempdir().unwrap();
    let port = free_port();

    let desktop = engine(Role::Desktop, desktop_dir.path(), port);
    let phone = engine(Role::Phone, phone_dir.path(), port);
    pair(&desktop, &phone);

    drop(phone);
    wait_for(&desktop, "que la PC note el corte", |v| v.status == Status::Searching);

    let phone = engine(Role::Phone, phone_dir.path(), port);
    let view = wait_for(&phone, "reconexión automática", |v| {
        assert!(pairing_code(v).is_none(), "no debe volver a pedir código");
        connected(v)
    });
    assert_eq!(view.paired.len(), 1);
    wait_for(&desktop, "reconexión en la PC", connected);
}

#[test]
fn video_starts_at_a_keyframe_and_reaches_the_desktop() {
    let desktop_dir = tempfile::tempdir().unwrap();
    let phone_dir = tempfile::tempdir().unwrap();
    let port = free_port();

    let desktop = engine(Role::Desktop, desktop_dir.path(), port);
    let phone = engine(Role::Phone, phone_dir.path(), port);
    pair(&desktop, &phone);
    let received = desktop.take_video_receiver().unwrap();
    let packet = |flags, byte| VideoPacket {
        flags,
        data: vec![byte; 32],
    };

    // Sin fotograma clave previo, un fotograma normal no se envía y se pide uno.
    assert!(phone.send_video(packet(0, 1)));
    assert!(!phone.send_video(packet(VideoPacket::CONFIG, 2)));
    assert_eq!(received.recv_timeout(Duration::from_secs(5)).unwrap().data[0], 2);
    assert!(!desktop.view().mirroring, "los parámetros solos no son imagen");
    assert!(!phone.send_video(packet(VideoPacket::KEY, 3)));
    assert!(!phone.send_video(packet(0, 4)));

    let wait = Duration::from_secs(5);
    assert_eq!(received.recv_timeout(wait).unwrap().data[0], 3);
    assert_eq!(received.recv_timeout(wait).unwrap().data[0], 4);
    wait_for(&desktop, "vídeo activo", |v| v.mirroring);

    phone.set_mirroring(false);
    wait_for(&desktop, "fin del vídeo", |v| !v.mirroring);

    // Los últimos fotogramas en cola llegan después del aviso de fin.
    assert!(!phone.send_video(packet(VideoPacket::KEY, 5)));
    assert!(!phone.send_video(packet(0, 6)));
    sleep(Duration::from_millis(300));
    assert!(!desktop.view().mirroring, "un fotograma rezagado no reabre la transmisión");

    // Una transmisión nueva siempre empieza por los parámetros del codificador.
    assert!(!phone.send_video(packet(VideoPacket::CONFIG, 7)));
    assert!(!phone.send_video(packet(VideoPacket::KEY, 8)));
    wait_for(&desktop, "transmisión nueva", |v| v.mirroring);
}

#[test]
fn the_phone_only_obeys_while_sharing_with_control_enabled() {
    let desktop_dir = tempfile::tempdir().unwrap();
    let phone_dir = tempfile::tempdir().unwrap();
    let port = free_port();

    let desktop = engine(Role::Desktop, desktop_dir.path(), port);
    let phone = engine(Role::Phone, phone_dir.path(), port);
    pair(&desktop, &phone);

    let back = || Input::Navigate { action: NavAction::Back };
    let short = Duration::from_millis(300);
    let long = Duration::from_secs(5);

    // Sin compartir y sin permiso, la orden se descarta.
    desktop.send_input(back());
    assert_eq!(phone.next_input(short), None);

    // Con permiso pero sin compartir, también.
    phone.set_input_enabled(true);
    wait_for(&desktop, "permiso en la PC", |v| v.input_enabled);
    desktop.send_input(back());
    assert_eq!(phone.next_input(short), None);

    // Compartiendo y con permiso, llega.
    phone.set_mirroring(true);
    desktop.send_input(back());
    let received = phone.next_input(long).unwrap();
    let parsed: Input = serde_json::from_str(&received).unwrap();
    assert_eq!(parsed, back());

    // Al dejar de compartir se suelta cualquier gesto y se deja de obedecer.
    phone.set_mirroring(false);
    let cancel: Input = serde_json::from_str(&phone.next_input(long).unwrap()).unwrap();
    assert_eq!(cancel, Input::Cancel);
    desktop.send_input(back());
    assert_eq!(phone.next_input(short), None);
}

#[test]
fn phone_reports_reach_the_desktop() {
    let desktop_dir = tempfile::tempdir().unwrap();
    let phone_dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let desktop = engine(Role::Desktop, desktop_dir.path(), port);
    let phone = engine(Role::Phone, phone_dir.path(), port);
    let events = desktop.take_desktop_events().unwrap();
    pair(&desktop, &phone);

    let info = NotificationInfo {
        key: "k1".to_string(),
        app: "App".to_string(),
        title: "Hola".to_string(),
        text: "Mundo".to_string(),
    };
    phone.post_notification(info.clone());
    phone.set_battery(77, true);

    let view = wait_for(&desktop, "estado", |v| v.battery.is_some() && !v.notifications.is_empty());
    assert_eq!(view.battery, Some((77, true)));
    assert_eq!(view.notifications, vec![info.clone()]);
    assert_eq!(
        events.recv_timeout(Duration::from_secs(5)).unwrap(),
        DesktopEvent::Notification(info)
    );

    phone.remove_notification("k1".to_string());
    wait_for(&desktop, "retirada", |v| v.notifications.is_empty());

    phone.send_clipboard("desde el móvil".to_string());
    assert_eq!(
        events.recv_timeout(Duration::from_secs(5)).unwrap(),
        DesktopEvent::Clipboard("desde el móvil".to_string())
    );
}

#[test]
fn desktop_commands_and_clipboard_become_phone_tasks() {
    let desktop_dir = tempfile::tempdir().unwrap();
    let phone_dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let desktop = engine(Role::Desktop, desktop_dir.path(), port);
    let phone = engine(Role::Phone, phone_dir.path(), port);
    pair(&desktop, &phone);
    let wait = Duration::from_secs(5);

    desktop.send_command(PhoneCommand::Media {
        action: MediaAction::Next,
    });
    let task: serde_json::Value = serde_json::from_str(&phone.next_task(wait).unwrap()).unwrap();
    assert_eq!(task["command"], "media");
    assert_eq!(task["action"], "next");

    desktop.send_command(PhoneCommand::StopMirror);
    let task: serde_json::Value = serde_json::from_str(&phone.next_task(wait).unwrap()).unwrap();
    assert_eq!(task["command"], "stop_mirror");

    desktop.send_clipboard("secreto".to_string());
    let task: serde_json::Value = serde_json::from_str(&phone.next_task(wait).unwrap()).unwrap();
    assert_eq!(task["command"], "set_clipboard");
    assert_eq!(task["text"], "secreto");
}

#[test]
fn a_file_with_a_hostile_name_lands_inside_the_downloads_folder() {
    let desktop_dir = tempfile::tempdir().unwrap();
    let phone_dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let desktop = engine(Role::Desktop, desktop_dir.path(), port);
    let phone = engine(Role::Phone, phone_dir.path(), port);
    let events = desktop.take_desktop_events().unwrap();
    pair(&desktop, &phone);

    let content = vec![7u8; 200_000];
    let id = phone.begin_file("../../evil.txt", content.len() as u64).unwrap();
    for piece in content.chunks(65_536) {
        assert!(phone.send_file_chunk(id, piece.to_vec()));
    }
    phone.end_file(id, true);

    let event = events.recv_timeout(Duration::from_secs(5)).unwrap();
    let DesktopEvent::FileReceived(path, name) = event else {
        panic!("se esperaba un archivo: {event:?}");
    };
    assert_eq!(name, "evil.txt");
    assert_eq!(path, desktop_dir.path().join("incoming").join("evil.txt"));
    assert_eq!(std::fs::read(path).unwrap(), content);
}

#[test]
fn the_round_trip_is_measured_from_real_pings() {
    let desktop_dir = tempfile::tempdir().unwrap();
    let phone_dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let desktop = engine(Role::Desktop, desktop_dir.path(), port);
    let phone = engine(Role::Phone, phone_dir.path(), port);
    assert_eq!(desktop.view().round_trip_ms, None);
    pair(&desktop, &phone);

    let view = wait_for(&desktop, "latencia", |v| v.round_trip_ms.is_some());
    assert!(view.round_trip_ms.unwrap() < 5_000);

    drop(phone);
    wait_for(&desktop, "fin de la medición", |v| v.round_trip_ms.is_none());
}

#[test]
fn the_phone_log_reaches_the_desktop_even_if_written_before_connecting() {
    let desktop_dir = tempfile::tempdir().unwrap();
    let phone_dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let desktop = engine(Role::Desktop, desktop_dir.path(), port);
    let phone = engine(Role::Phone, phone_dir.path(), port);

    phone.log("antes de conectar".to_string());
    pair(&desktop, &phone);
    phone.log("ya conectado".to_string());

    let path = desktop_dir.path().join("phone.log");
    let start = Instant::now();
    loop {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        if text.contains("antes de conectar") && text.contains("ya conectado") {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "registro incompleto: {text}");
        sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_sender_learns_when_a_file_was_really_saved() {
    let desktop_dir = tempfile::tempdir().unwrap();
    let phone_dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let desktop = engine(Role::Desktop, desktop_dir.path(), port);
    let phone = engine(Role::Phone, phone_dir.path(), port);
    let events = desktop.take_desktop_events().unwrap();
    pair(&desktop, &phone);
    let wait = Duration::from_secs(5);

    let saved = desktop.begin_file("foto.jpg", 4).unwrap();
    assert!(desktop.send_file_chunk(saved, vec![1, 2, 3, 4]));
    desktop.end_file(saved, true);
    assert_eq!(events.recv_timeout(wait).unwrap(), DesktopEvent::FileDelivered(saved));

    // Un archivo que dice medir más de lo enviado llega incompleto.
    let short = desktop.begin_file("corta.jpg", 100).unwrap();
    assert!(desktop.send_file_chunk(short, vec![1]));
    desktop.end_file(short, true);
    assert_eq!(events.recv_timeout(wait).unwrap(), DesktopEvent::FileFailed(short));
}

#[test]
fn unpairing_forgets_on_both_sides_and_asks_for_a_code_again() {
    let desktop_dir = tempfile::tempdir().unwrap();
    let phone_dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let desktop = engine(Role::Desktop, desktop_dir.path(), port);
    let phone = engine(Role::Phone, phone_dir.path(), port);
    pair(&desktop, &phone);

    desktop.unpair().unwrap();

    let desktop_view = wait_for(&desktop, "fin en la PC", |v| v.status == Status::Searching);
    let phone_view = wait_for(&phone, "fin en el móvil", |v| v.status == Status::Searching && v.paired.is_empty());
    assert!(desktop_view.paired.is_empty());
    assert!(phone_view.paired.is_empty());

    // No se reconecta solo: hace falta emparejar de nuevo.
    sleep(Duration::from_secs(4));
    assert_eq!(phone.view().status, Status::Searching);
    pair(&desktop, &phone);
}

#[test]
fn the_audio_choice_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let first = engine(Role::Desktop, dir.path(), port);
    assert!(first.audio_enabled());
    first.set_audio_enabled(false);
    drop(first);
    let second = engine(Role::Desktop, dir.path(), free_port());
    assert!(!second.audio_enabled());
}

#[test]
fn rejecting_the_code_pairs_nobody() {
    let desktop_dir = tempfile::tempdir().unwrap();
    let phone_dir = tempfile::tempdir().unwrap();
    let port = free_port();

    let desktop = engine(Role::Desktop, desktop_dir.path(), port);
    let phone = engine(Role::Phone, phone_dir.path(), port);

    let found = wait_for(&phone, "la PC", |view| !view.found.is_empty());
    phone.connect(found.found[0].fingerprint);
    wait_for(&phone, "código", |v| pairing_code(v).is_some());
    wait_for(&desktop, "código", |v| pairing_code(v).is_some());

    desktop.answer_pairing(false);
    phone.answer_pairing(true);

    let desktop_view = wait_for(&desktop, "fin en la PC", |v| v.status == Status::Searching);
    let phone_view = wait_for(&phone, "fin en el móvil", |v| v.status == Status::Searching);
    assert!(desktop_view.paired.is_empty());
    assert!(phone_view.paired.is_empty());
}

#[test]
fn identity_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let first = Identity::load_or_create(dir.path(), "a").unwrap();
    let second = Identity::load_or_create(dir.path(), "a").unwrap();
    assert_eq!(first.fingerprint, second.fingerprint);

    use std::os::unix::fs::PermissionsExt;
    let key = std::fs::metadata(dir.path().join("identity.key.der")).unwrap();
    assert_eq!(key.permissions().mode() & 0o777, 0o600);
}

struct Device {
    identity: Identity,
    tls: TlsContext,
    store: Mutex<PeerStore>,
    _dir: tempfile::TempDir,
}

fn device(name: &str) -> Device {
    let dir = tempfile::tempdir().unwrap();
    let identity = Identity::generate(name).unwrap();
    let tls = TlsContext::new(&identity).unwrap();
    let store = PeerStore::load(dir.path().join("peers.json")).unwrap();
    Device {
        identity,
        tls,
        store: Mutex::new(store),
        _dir: dir,
    }
}

fn prompt_reports(
    shown: oneshot::Sender<String>,
) -> impl FnOnce(PairPrompt) -> oneshot::Receiver<bool> {
    move |request| {
        let _ = shown.send(request.code);
        let (answer, receiver) = oneshot::channel();
        let _ = answer.send(false);
        receiver
    }
}

#[tokio::test]
async fn an_impersonator_of_a_paired_peer_still_gets_the_code_prompt() {
    let desktop = device("pc");
    let genuine = device("móvil");
    let impostor = device("móvil");

    // La PC conoce al móvil auténtico, y el impostor afirma conocer a la PC.
    desktop
        .store
        .lock()
        .unwrap()
        .add("móvil", genuine.identity.fingerprint)
        .unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();

    let server = async {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut secure = desktop.tls.accept(tcp).await.unwrap();
        assert_eq!(secure.peer, impostor.identity.fingerprint);
        let (shown, code) = oneshot::channel();
        let result = establish(
            &mut secure.stream,
            Role::Desktop,
            &desktop.identity,
            secure.peer,
            &desktop.store,
            prompt_reports(shown),
        )
        .await;
        (result, code.await)
    };

    let client = async {
        let mut secure = impostor.tls.connect(address).await.unwrap();
        let hello = Message::Hello {
            protocol: puente_core::PROTOCOL_VERSION,
            name: "móvil".to_string(),
            known: true,
        };
        write_message(&mut secure.stream, &hello).await.unwrap();
        let _their_hello = read_message(&mut secure.stream).await.unwrap();
        // Si la PC lo diera por válido, aquí no llegaría el compromiso.
        let first = read_message(&mut secure.stream).await.unwrap();
        assert!(matches!(first, Message::PairCommit { .. }));
        let nonce = Message::PairNonce {
            nonce: "00".repeat(32),
        };
        write_message(&mut secure.stream, &nonce).await.unwrap();
        let reveal = read_message(&mut secure.stream).await.unwrap();
        assert!(matches!(reveal, Message::PairReveal { .. }));
        let decision = read_message(&mut secure.stream).await.unwrap();
        assert_eq!(decision, Message::PairDecision { accepted: false });
    };

    let ((result, code), ()) = tokio::join!(server, client);
    assert!(matches!(result, Err(Error::Rejected)));
    assert_eq!(code.unwrap().len(), 6);
    assert!(!desktop.store.lock().unwrap().is_paired(impostor.identity.fingerprint));
}

#[tokio::test]
async fn oversized_frames_are_refused_before_allocating() {
    let (mut near, mut far) = tokio::io::duplex(64);
    use tokio::io::AsyncWriteExt;
    far.write_all(&u32::MAX.to_be_bytes()).await.unwrap();
    assert!(matches!(read_message(&mut near).await, Err(Error::Protocol(_))));
}
