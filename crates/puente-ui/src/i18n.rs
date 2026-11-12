//! Los textos de la interfaz en español, inglés y portugués.
//!
//! Cada idioma es una tabla con los mismos campos: si falta una traducción, no compila.

use std::sync::atomic::{AtomicU8, Ordering};

use puente_core::{FileNote, LanguageChoice, NoticeMode, Problem, VideoQuality};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Spanish,
    English,
    Portuguese,
}

impl Language {
    /// Lee etiquetas como `es_DO.UTF-8`, `pt-BR` o `en`. Lo que no conoce, en inglés.
    pub fn from_tag(tag: &str) -> Self {
        let code: String = tag.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        match code.to_ascii_lowercase().as_str() {
            "es" => Self::Spanish,
            "pt" => Self::Portuguese,
            _ => Self::English,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Spanish => "es",
            Self::English => "en",
            Self::Portuguese => "pt",
        }
    }

    /// Cada idioma con su propio nombre, para reconocerlo aunque no se entienda el resto.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::Spanish => "Español",
            Self::English => "English",
            Self::Portuguese => "Português",
        }
    }

    fn to_index(self) -> u8 {
        match self {
            Self::Spanish => 0,
            Self::English => 1,
            Self::Portuguese => 2,
        }
    }

    fn from_index(index: u8) -> Self {
        match index {
            0 => Self::Spanish,
            2 => Self::Portuguese,
            _ => Self::English,
        }
    }
}

static SYSTEM: AtomicU8 = AtomicU8::new(1);
static CURRENT: AtomicU8 = AtomicU8::new(1);

/// El idioma del sistema, que se usa cuando la elección es automática.
pub fn set_system_language(tag: &str) {
    SYSTEM.store(Language::from_tag(tag).to_index(), Ordering::Relaxed);
}

pub fn system_language() -> Language {
    Language::from_index(SYSTEM.load(Ordering::Relaxed))
}

pub fn resolve(choice: LanguageChoice) -> Language {
    match choice {
        LanguageChoice::Auto => system_language(),
        LanguageChoice::Spanish => Language::Spanish,
        LanguageChoice::English => Language::English,
        LanguageChoice::Portuguese => Language::Portuguese,
    }
}

pub fn apply(choice: LanguageChoice) {
    CURRENT.store(resolve(choice).to_index(), Ordering::Relaxed);
}

pub fn language() -> Language {
    Language::from_index(CURRENT.load(Ordering::Relaxed))
}

pub fn t() -> &'static Texts {
    match language() {
        Language::Spanish => &SPANISH,
        Language::English => &ENGLISH,
        Language::Portuguese => &PORTUGUESE,
    }
}

/// Lo que muestra el selector de idioma.
pub fn choice_label(choice: LanguageChoice) -> String {
    match choice {
        LanguageChoice::Auto => format!("Auto · {}", system_language().code().to_uppercase()),
        other => resolve(other).native_name().to_string(),
    }
}

pub fn quality_label(quality: VideoQuality) -> &'static str {
    let texts = t();
    match quality {
        VideoQuality::Auto => texts.quality_auto,
        VideoQuality::Saver => texts.quality_saver,
        VideoQuality::Balanced => texts.quality_balanced,
        VideoQuality::High => texts.quality_high,
    }
}

pub fn notice_mode_label(mode: NoticeMode) -> &'static str {
    let texts = t();
    match mode {
        NoticeMode::On => texts.notices_on,
        NoticeMode::Silent => texts.notices_silent,
        NoticeMode::Off => texts.notices_off,
    }
}

pub fn file_note(note: &FileNote) -> String {
    let texts = t();
    match note {
        FileNote::SendFailed => texts.file_send_failed.to_string(),
        FileNote::ArrivedIncomplete => texts.file_incomplete.to_string(),
        FileNote::Received(name) => (texts.file_received)(name),
    }
}

pub fn problem(problem: &Problem) -> &'static str {
    let texts = t();
    match problem {
        Problem::PairingRejected => texts.problem_rejected,
        Problem::TimedOut => texts.problem_timeout,
        Problem::Connection(_) => texts.problem_connection,
    }
}

pub struct Texts {
    // Tarjetas.
    pub card_link: &'static str,
    pub card_battery: &'static str,
    pub card_audio: &'static str,
    pub card_find: &'static str,
    pub card_music: &'static str,
    pub card_notices: &'static str,
    pub card_files: &'static str,
    pub card_settings: &'static str,
    pub card_screen: &'static str,
    pub card_permissions: &'static str,
    pub card_found: &'static str,
    pub card_first_steps: &'static str,
    pub card_paired: &'static str,
    pub card_pair: &'static str,

    // Conexión y emparejamiento.
    pub connecting_to: fn(&str) -> String,
    pub this_pc_address: fn(&str) -> String,
    pub show_address: &'static str,
    pub hide_address: &'static str,
    pub card_address: &'static str,
    pub address_help: &'static str,
    pub type_address: &'static str,
    pub scan_code: &'static str,
    pub scan_from_phone: &'static str,
    pub address_placeholder: &'static str,
    pub connect: &'static str,
    pub remove: &'static str,
    pub searching_at: fn(&str) -> String,
    pub no_answer_from: fn(&str) -> String,
    pub pair_with: fn(&str) -> String,
    pub waiting_for_phone: &'static str,
    pub install_steps: [&'static str; 3],
    pub open_on_phone: &'static str,
    pub forget: &'static str,
    pub waiting_other: &'static str,
    pub accept_if_same: &'static str,
    pub codes_match: &'static str,
    pub cancel: &'static str,
    pub searching_pc: &'static str,
    pub pair: &'static str,
    pub first_steps_text: &'static str,
    pub startup_failed: &'static str,
    pub problem_rejected: &'static str,
    pub problem_timeout: &'static str,
    pub problem_connection: &'static str,

    // Transmisión.
    pub cards: &'static str,
    pub show_cards: &'static str,
    pub hide_cards: &'static str,
    pub shortcut_f2: &'static str,
    pub view_phone: &'static str,
    pub stop_stream: &'static str,
    pub enable_control: &'static str,
    pub back: &'static str,
    pub home: &'static str,
    pub recents: &'static str,
    pub share_from_phone: &'static str,
    pub drop_to_send_phone: &'static str,

    // Llamadas.
    pub decline: &'static str,
    pub answer: &'static str,
    pub call_in_app: fn(&str) -> String,
    pub call_from: fn(&str) -> String,

    // Enlace y batería.
    pub sure: &'static str,
    pub unpair: &'static str,
    pub unpair_this_phone: &'static str,
    pub charging: &'static str,

    // Audio.
    pub audio_off: &'static str,
    pub audio_playing: &'static str,
    pub audio_silent: &'static str,
    pub mute_phone: &'static str,
    pub mute_phone_hint: &'static str,

    // Música.
    pub nothing_playing: &'static str,
    pub untitled: &'static str,
    pub previous: &'static str,
    pub pause: &'static str,
    pub play: &'static str,
    pub next: &'static str,

    // Avisos.
    pub no_notices: &'static str,
    pub reply_hint: &'static str,
    pub send: &'static str,
    pub reply: &'static str,
    pub reply_sent: &'static str,
    pub reply_failed: &'static str,
    pub dismiss: &'static str,
    pub dismiss_all: &'static str,
    pub notices_on: &'static str,
    pub notices_silent: &'static str,
    pub notices_off: &'static str,
    pub notices_are_off: &'static str,
    pub notices_mode_hint: &'static str,

    // Buscar móvil.
    pub stop_ring: &'static str,
    pub ring_phone: &'static str,
    pub ring: &'static str,
    pub ringing: fn(f32) -> String,

    // Archivos.
    pub drop_to_send: &'static str,
    pub drag_files_here: &'static str,
    pub choose: &'static str,
    pub not_sent: fn(&str) -> String,
    pub file_count: fn(usize) -> String,
    pub and_more: fn(usize) -> String,
    pub file_send_failed: &'static str,
    pub file_incomplete: &'static str,
    pub file_received: fn(&str) -> String,
    pub file_received_title: &'static str,

    // Ajustes.
    pub clipboard: &'static str,
    pub animations: &'static str,
    pub language: &'static str,
    pub quality: &'static str,
    pub quality_hint: &'static str,
    pub quality_auto: &'static str,
    pub quality_saver: &'static str,
    pub quality_balanced: &'static str,
    pub quality_high: &'static str,
    /// Atajo de ratón de cada botón de navegación, en su ayuda.
    pub mouse_back: &'static str,
    pub mouse_home: &'static str,
    pub mouse_recents: &'static str,

    // Bandeja del sistema.
    pub tray_show: &'static str,
    pub tray_autostart: &'static str,
    pub tray_quit: &'static str,

    // Versiones distintas.
    pub versions_differ: &'static str,
    pub phone_side: &'static str,
    /// Lo que se muestra cuando el otro lado no dice su versión.
    pub version_old: &'static str,
    pub update_phone: &'static str,
    pub update_pc: &'static str,

    // Móvil.
    pub sharing_tap_stop: &'static str,
    pub screen_stays_on: &'static str,
    pub share_screen: &'static str,
    pub android_asks: &'static str,
    pub control_from_pc: &'static str,
    pub notices_on_pc: &'static str,
    pub keep_running: &'static str,
    pub restricted_help: &'static str,
    pub active: &'static str,
    pub enable: &'static str,

    // Terminal.
    pub send_usage: &'static str,
    pub sending_files: fn(usize) -> String,
    pub send_error: fn(&str) -> String,
    pub not_open: &'static str,
    pub phone_not_connected: &'static str,
    pub not_a_file: &'static str,
    pub invalid_name: &'static str,
    pub no_answer: &'static str,
    pub linux_only: &'static str,
}

static SPANISH: Texts = Texts {
    card_link: "Enlace",
    card_battery: "Batería",
    card_audio: "Audio",
    card_find: "Buscar móvil",
    card_music: "Música",
    card_notices: "Avisos",
    card_files: "Archivos",
    card_settings: "Ajustes",
    card_screen: "Pantalla",
    card_permissions: "Permisos",
    card_found: "PCs encontradas",
    card_first_steps: "Primeros pasos",
    card_paired: "Emparejadas",
    card_pair: "Emparejar",

    connecting_to: |name| format!("Conectando con {name}"),
    this_pc_address: |ip| format!("IP de esta PC: {ip}"),
    show_address: "Mostrar IP y código QR",
    hide_address: "Ocultar",
    card_address: "Conectar por IP",
    address_help: "Si tu PC no aparece sola, en la PC pulsa «Mostrar IP y código QR» y escanéalo, o escribe la IP.",
    type_address: "Escribir IP",
    scan_code: "Escanear QR",
    scan_from_phone: "Escanéalo desde Puente en el móvil",
    address_placeholder: "IP de la PC",
    connect: "Conectar",
    remove: "Quitar",
    searching_at: |ip| format!("Buscando en {ip}"),
    no_answer_from: |ip| format!("{ip} no responde"),
    pair_with: |name| format!("Emparejar con {name}"),
    waiting_for_phone: "Esperando al móvil",
    install_steps: [
        "1. Instala Puente en tu móvil Android.",
        "2. Ábrela con el móvil en esta misma Wi-Fi.",
        "3. Toca esta PC y compara el código.",
    ],
    open_on_phone: "Abre Puente en el móvil y se conectará solo.",
    forget: "Olvidar",
    waiting_other: "Esperando al otro dispositivo",
    accept_if_same: "Acepta solo si este código es igual en los dos dispositivos.",
    codes_match: "Coinciden",
    cancel: "Cancelar",
    searching_pc: "Buscando tu PC en la Wi-Fi",
    pair: "Emparejar",
    first_steps_text: "Abre Puente en tu PC, con este móvil en la misma Wi-Fi. Aparecerá aquí.",
    startup_failed: "No se pudo iniciar Puente",
    problem_rejected: "El emparejamiento fue rechazado",
    problem_timeout: "Se agotó el tiempo de espera",
    problem_connection: "Se perdió la conexión",

    cards: "Tarjetas",
    show_cards: "Mostrar tarjetas",
    hide_cards: "Ocultar tarjetas (F2)",
    shortcut_f2: "Atajo: F2",
    view_phone: "Ver móvil",
    stop_stream: "Cortar la transmisión",
    enable_control: "Activa el control en el móvil",
    back: "Atrás",
    home: "Inicio",
    recents: "Recientes",
    share_from_phone: "Comparte la pantalla desde el móvil",
    drop_to_send_phone: "Suelta para enviar al móvil",

    decline: "Rechazar",
    answer: "Contestar",
    call_in_app: |app| format!("Llamada · {app}"),
    call_from: |caller| format!("Llamada de {caller}"),

    sure: "¿Seguro?",
    unpair: "Desvincular",
    unpair_this_phone: "Desvincular este móvil",
    charging: "Cargando",

    audio_off: "Desactivado",
    audio_playing: "Sonando en la PC",
    audio_silent: "Sin audio",
    mute_phone: "Silenciar móvil",
    mute_phone_hint: "La música se oye solo en la PC. Llamadas y alarmas siguen sonando en el móvil",

    nothing_playing: "Nada sonando",
    untitled: "Sin título",
    previous: "Anterior",
    pause: "Pausa",
    play: "Reproducir",
    next: "Siguiente",

    no_notices: "Sin avisos",
    reply_hint: "Escribe tu respuesta",
    send: "Enviar",
    reply: "Responder",
    reply_sent: "Respuesta enviada",
    reply_failed: "No se pudo enviar",
    dismiss: "Descartar",
    dismiss_all: "Borrar todo",
    notices_on: "Activos",
    notices_silent: "Silencio",
    notices_off: "Apagados",
    notices_are_off: "Avisos apagados",
    notices_mode_hint: "Activos: en esta lista y como aviso de la PC. Silencio: solo en esta lista. Apagados: no se muestran",

    stop_ring: "Parar",
    ring_phone: "Hacer sonar el móvil",
    ring: "Hacer sonar",
    ringing: |seconds| format!("Sonando {seconds:.0} s"),

    drop_to_send: "Suelta para enviar",
    drag_files_here: "Arrastra archivos aquí",
    choose: "Elegir",
    not_sent: |name| format!("No se pudo: {name}"),
    file_count: |count| format!("{count} archivos"),
    and_more: |count| format!("Y {count} más"),
    file_send_failed: "Falló un envío de archivo",
    file_incomplete: "Un archivo llegó incompleto",
    file_received: |name| format!("Recibido: {name}"),
    file_received_title: "Archivo recibido",

    clipboard: "Portapapeles",
    animations: "Animaciones",
    language: "Idioma",
    quality: "Calidad",
    quality_hint: "Auto: 1920 px a 60 fps, con los datos que aguante la red. Ahorro: 1280 px a 30 fps. Normal: 1920 px a 60 fps. Alta: 2560 px a 60 fps, con el doble de datos",
    quality_auto: "Auto",
    quality_saver: "Ahorro",
    quality_balanced: "Normal",
    quality_high: "Alta",
    mouse_back: "Clic derecho sobre la pantalla",
    mouse_home: "Clic central sobre la pantalla",
    mouse_recents: "Botón lateral de avance",

    tray_show: "Mostrar Puente",
    tray_autostart: "Iniciar con la sesión",
    tray_quit: "Salir",

    versions_differ: "Versiones distintas",
    phone_side: "Móvil",
    version_old: "0.3.0 o anterior",
    update_phone: "Actualiza Puente en el móvil",
    update_pc: "Actualiza Puente en la PC",

    sharing_tap_stop: "Compartiendo. Toca para detener",
    screen_stays_on: "La pantalla no se apagará mientras la PC esté conectada.",
    share_screen: "Compartir pantalla",
    android_asks: "Android te pedirá confirmarlo cada vez.",
    control_from_pc: "Control desde la PC",
    notices_on_pc: "Avisos en la PC",
    keep_running: "Seguir en segundo plano",
    restricted_help: "Si Android lo bloquea: Ajustes, Aplicaciones, Puente, menú de arriba, \
                      Permitir ajustes restringidos.",
    active: "Activo",
    enable: "Activar",

    send_usage: "uso: puente --send <archivo>...",
    sending_files: |count| format!("Enviando {count} archivo(s) al móvil; míralo en la tarjeta 07"),
    send_error: |error| format!("No se pudo enviar: {error}"),
    not_open: "Puente no está abierto",
    phone_not_connected: "el móvil no está conectado a Puente",
    not_a_file: "no es un archivo",
    invalid_name: "nombre no válido",
    no_answer: "Puente no respondió",
    linux_only: "solo disponible en Linux",
};

static ENGLISH: Texts = Texts {
    card_link: "Link",
    card_battery: "Battery",
    card_audio: "Audio",
    card_find: "Find phone",
    card_music: "Music",
    card_notices: "Notifications",
    card_files: "Files",
    card_settings: "Settings",
    card_screen: "Screen",
    card_permissions: "Permissions",
    card_found: "PCs found",
    card_first_steps: "Getting started",
    card_paired: "Paired",
    card_pair: "Pair",

    connecting_to: |name| format!("Connecting to {name}"),
    this_pc_address: |ip| format!("This PC's IP: {ip}"),
    show_address: "Show IP and QR code",
    hide_address: "Hide",
    card_address: "Connect by IP",
    address_help: "If your PC does not show up by itself, press \"Show IP and QR code\" on the PC and scan it, or type the IP.",
    type_address: "Type IP",
    scan_code: "Scan QR",
    scan_from_phone: "Scan it from Puente on the phone",
    address_placeholder: "PC's IP",
    connect: "Connect",
    remove: "Remove",
    searching_at: |ip| format!("Looking at {ip}"),
    no_answer_from: |ip| format!("{ip} does not answer"),
    pair_with: |name| format!("Pair with {name}"),
    waiting_for_phone: "Waiting for the phone",
    install_steps: [
        "1. Install Puente on your Android phone.",
        "2. Open it with the phone on this same Wi-Fi.",
        "3. Tap this PC and compare the code.",
    ],
    open_on_phone: "Open Puente on the phone and it will connect by itself.",
    forget: "Forget",
    waiting_other: "Waiting for the other device",
    accept_if_same: "Accept only if this code is the same on both devices.",
    codes_match: "They match",
    cancel: "Cancel",
    searching_pc: "Looking for your PC on the Wi-Fi",
    pair: "Pair",
    first_steps_text: "Open Puente on your PC, with this phone on the same Wi-Fi. It will show up here.",
    startup_failed: "Puente could not start",
    problem_rejected: "Pairing was rejected",
    problem_timeout: "The wait timed out",
    problem_connection: "The connection was lost",

    cards: "Cards",
    show_cards: "Show cards",
    hide_cards: "Hide cards (F2)",
    shortcut_f2: "Shortcut: F2",
    view_phone: "Show phone",
    stop_stream: "Stop the stream",
    enable_control: "Turn on control on the phone",
    back: "Back",
    home: "Home",
    recents: "Recents",
    share_from_phone: "Share the screen from the phone",
    drop_to_send_phone: "Drop to send to the phone",

    decline: "Decline",
    answer: "Answer",
    call_in_app: |app| format!("Call · {app}"),
    call_from: |caller| format!("Call from {caller}"),

    sure: "Sure?",
    unpair: "Unpair",
    unpair_this_phone: "Unpair this phone",
    charging: "Charging",

    audio_off: "Off",
    audio_playing: "Playing on the PC",
    audio_silent: "No audio",
    mute_phone: "Mute phone",
    mute_phone_hint: "Music plays only on the PC. Calls and alarms still ring on the phone",

    nothing_playing: "Nothing playing",
    untitled: "Untitled",
    previous: "Previous",
    pause: "Pause",
    play: "Play",
    next: "Next",

    no_notices: "No notifications",
    reply_hint: "Write your reply",
    send: "Send",
    reply: "Reply",
    reply_sent: "Reply sent",
    reply_failed: "Could not send",
    dismiss: "Dismiss",
    dismiss_all: "Clear all",
    notices_on: "On",
    notices_silent: "Silent",
    notices_off: "Off",
    notices_are_off: "Notifications off",
    notices_mode_hint: "On: in this list and as a PC notification. Silent: only in this list. Off: not shown",

    stop_ring: "Stop",
    ring_phone: "Make the phone ring",
    ring: "Ring",
    ringing: |seconds| format!("Ringing {seconds:.0} s"),

    drop_to_send: "Drop to send",
    drag_files_here: "Drag files here",
    choose: "Choose",
    not_sent: |name| format!("Failed: {name}"),
    file_count: |count| format!("{count} files"),
    and_more: |count| format!("And {count} more"),
    file_send_failed: "A file transfer failed",
    file_incomplete: "A file arrived incomplete",
    file_received: |name| format!("Received: {name}"),
    file_received_title: "File received",

    clipboard: "Clipboard",
    animations: "Animations",
    language: "Language",
    quality: "Quality",
    quality_hint: "Auto: 1920 px at 60 fps, with as much data as the network handles. Saver: 1280 px at 30 fps. Normal: 1920 px at 60 fps. High: 2560 px at 60 fps, with twice the data",
    quality_auto: "Auto",
    quality_saver: "Saver",
    quality_balanced: "Normal",
    quality_high: "High",
    mouse_back: "Right click on the screen",
    mouse_home: "Middle click on the screen",
    mouse_recents: "Forward side button",

    tray_show: "Show Puente",
    tray_autostart: "Start with the session",
    tray_quit: "Quit",

    versions_differ: "Different versions",
    phone_side: "Phone",
    version_old: "0.3.0 or older",
    update_phone: "Update Puente on the phone",
    update_pc: "Update Puente on the PC",

    sharing_tap_stop: "Sharing. Tap to stop",
    screen_stays_on: "The screen will stay on while the PC is connected.",
    share_screen: "Share screen",
    android_asks: "Android will ask you to confirm every time.",
    control_from_pc: "Control from the PC",
    notices_on_pc: "Notifications on the PC",
    keep_running: "Keep running in the background",
    restricted_help: "If Android blocks it: Settings, Apps, Puente, top menu, \
                      Allow restricted settings.",
    active: "On",
    enable: "Turn on",

    send_usage: "usage: puente --send <file>...",
    sending_files: |count| format!("Sending {count} file(s) to the phone; see card 07"),
    send_error: |error| format!("Could not send: {error}"),
    not_open: "Puente is not open",
    phone_not_connected: "the phone is not connected to Puente",
    not_a_file: "not a file",
    invalid_name: "invalid name",
    no_answer: "Puente did not answer",
    linux_only: "only available on Linux",
};

static PORTUGUESE: Texts = Texts {
    card_link: "Conexão",
    card_battery: "Bateria",
    card_audio: "Áudio",
    card_find: "Achar celular",
    card_music: "Música",
    card_notices: "Notificações",
    card_files: "Arquivos",
    card_settings: "Ajustes",
    card_screen: "Tela",
    card_permissions: "Permissões",
    card_found: "PCs encontrados",
    card_first_steps: "Primeiros passos",
    card_paired: "Pareados",
    card_pair: "Parear",

    connecting_to: |name| format!("Conectando a {name}"),
    this_pc_address: |ip| format!("IP deste PC: {ip}"),
    show_address: "Mostrar IP e código QR",
    hide_address: "Ocultar",
    card_address: "Conectar por IP",
    address_help: "Se o seu PC não aparecer sozinho, no PC toque em «Mostrar IP e código QR» e escaneie, ou digite o IP.",
    type_address: "Digitar IP",
    scan_code: "Escanear QR",
    scan_from_phone: "Escaneie pelo Puente no celular",
    address_placeholder: "IP do PC",
    connect: "Conectar",
    remove: "Remover",
    searching_at: |ip| format!("Procurando em {ip}"),
    no_answer_from: |ip| format!("{ip} não responde"),
    pair_with: |name| format!("Parear com {name}"),
    waiting_for_phone: "Aguardando o celular",
    install_steps: [
        "1. Instale o Puente no seu celular Android.",
        "2. Abra-o com o celular nesta mesma Wi-Fi.",
        "3. Toque neste PC e compare o código.",
    ],
    open_on_phone: "Abra o Puente no celular e ele se conectará sozinho.",
    forget: "Esquecer",
    waiting_other: "Aguardando o outro dispositivo",
    accept_if_same: "Aceite só se este código for igual nos dois dispositivos.",
    codes_match: "São iguais",
    cancel: "Cancelar",
    searching_pc: "Procurando seu PC na Wi-Fi",
    pair: "Parear",
    first_steps_text: "Abra o Puente no seu PC, com este celular na mesma Wi-Fi. Ele aparecerá aqui.",
    startup_failed: "Não foi possível iniciar o Puente",
    problem_rejected: "O pareamento foi recusado",
    problem_timeout: "O tempo de espera acabou",
    problem_connection: "A conexão caiu",

    cards: "Cartões",
    show_cards: "Mostrar cartões",
    hide_cards: "Ocultar cartões (F2)",
    shortcut_f2: "Atalho: F2",
    view_phone: "Ver celular",
    stop_stream: "Parar a transmissão",
    enable_control: "Ative o controle no celular",
    back: "Voltar",
    home: "Início",
    recents: "Recentes",
    share_from_phone: "Compartilhe a tela pelo celular",
    drop_to_send_phone: "Solte para enviar ao celular",

    decline: "Recusar",
    answer: "Atender",
    call_in_app: |app| format!("Chamada · {app}"),
    call_from: |caller| format!("Chamada de {caller}"),

    sure: "Certeza?",
    unpair: "Desparear",
    unpair_this_phone: "Desparear este celular",
    charging: "Carregando",

    audio_off: "Desativado",
    audio_playing: "Tocando no PC",
    audio_silent: "Sem áudio",
    mute_phone: "Silenciar celular",
    mute_phone_hint: "A música toca só no PC. Chamadas e alarmes continuam tocando no celular",

    nothing_playing: "Nada tocando",
    untitled: "Sem título",
    previous: "Anterior",
    pause: "Pausar",
    play: "Tocar",
    next: "Próxima",

    no_notices: "Sem notificações",
    reply_hint: "Escreva sua resposta",
    send: "Enviar",
    reply: "Responder",
    reply_sent: "Resposta enviada",
    reply_failed: "Não foi possível enviar",
    dismiss: "Dispensar",
    dismiss_all: "Limpar tudo",
    notices_on: "Ativas",
    notices_silent: "Silêncio",
    notices_off: "Desligadas",
    notices_are_off: "Notificações desligadas",
    notices_mode_hint: "Ativas: nesta lista e como notificação do PC. Silêncio: só nesta lista. Desligadas: não aparecem",

    stop_ring: "Parar",
    ring_phone: "Fazer o celular tocar",
    ring: "Fazer tocar",
    ringing: |seconds| format!("Tocando {seconds:.0} s"),

    drop_to_send: "Solte para enviar",
    drag_files_here: "Arraste arquivos aqui",
    choose: "Escolher",
    not_sent: |name| format!("Falhou: {name}"),
    file_count: |count| format!("{count} arquivos"),
    and_more: |count| format!("E mais {count}"),
    file_send_failed: "Um envio de arquivo falhou",
    file_incomplete: "Um arquivo chegou incompleto",
    file_received: |name| format!("Recebido: {name}"),
    file_received_title: "Arquivo recebido",

    clipboard: "Área de transferência",
    animations: "Animações",
    language: "Idioma",
    quality: "Qualidade",
    quality_hint: "Auto: 1920 px a 60 fps, com os dados que a rede aguentar. Economia: 1280 px a 30 fps. Normal: 1920 px a 60 fps. Alta: 2560 px a 60 fps, com o dobro de dados",
    quality_auto: "Auto",
    quality_saver: "Economia",
    quality_balanced: "Normal",
    quality_high: "Alta",
    mouse_back: "Clique direito na tela",
    mouse_home: "Clique do meio na tela",
    mouse_recents: "Botão lateral de avançar",

    tray_show: "Mostrar o Puente",
    tray_autostart: "Iniciar com a sessão",
    tray_quit: "Sair",

    versions_differ: "Versões diferentes",
    phone_side: "Celular",
    version_old: "0.3.0 ou anterior",
    update_phone: "Atualize o Puente no celular",
    update_pc: "Atualize o Puente no PC",

    sharing_tap_stop: "Compartilhando. Toque para parar",
    screen_stays_on: "A tela não vai apagar enquanto o PC estiver conectado.",
    share_screen: "Compartilhar tela",
    android_asks: "O Android vai pedir sua confirmação toda vez.",
    control_from_pc: "Controle pelo PC",
    notices_on_pc: "Notificações no PC",
    keep_running: "Continuar em segundo plano",
    restricted_help: "Se o Android bloquear: Configurações, Aplicativos, Puente, menu de cima, \
                      Permitir configurações restritas.",
    active: "Ativo",
    enable: "Ativar",

    send_usage: "uso: puente --send <arquivo>...",
    sending_files: |count| format!("Enviando {count} arquivo(s) ao celular; veja o cartão 07"),
    send_error: |error| format!("Não foi possível enviar: {error}"),
    not_open: "O Puente não está aberto",
    phone_not_connected: "o celular não está conectado ao Puente",
    not_a_file: "não é um arquivo",
    invalid_name: "nome inválido",
    no_answer: "O Puente não respondeu",
    linux_only: "disponível só no Linux",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_tags_pick_the_language_and_anything_else_is_english() {
        assert_eq!(Language::from_tag("es_DO.UTF-8"), Language::Spanish);
        assert_eq!(Language::from_tag("pt-BR"), Language::Portuguese);
        assert_eq!(Language::from_tag("en_US.UTF-8"), Language::English);
        assert_eq!(Language::from_tag("fr_FR"), Language::English);
        assert_eq!(Language::from_tag(""), Language::English);
    }

    #[test]
    fn a_fixed_choice_wins_over_the_system() {
        set_system_language("en_US");
        assert_eq!(resolve(LanguageChoice::Portuguese), Language::Portuguese);
        assert_eq!(resolve(LanguageChoice::Auto), Language::English);
    }
}
