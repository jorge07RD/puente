//! Protocolo, red y seguridad compartidos entre la PC y el móvil.

pub mod engine;
pub mod identity;
pub mod link;
pub mod pairing;
pub mod peers;
pub mod session;
pub mod settings;
pub mod telemetry;
pub mod tls;
pub mod transfers;
pub mod wire;

pub use engine::{Config, DesktopEvent, Engine, FileNote, Found, Problem, Status, View};
pub use settings::{LanguageChoice, NoticeMode, VideoQuality};
pub use identity::{Fingerprint, Identity};
pub use pairing::Role;
pub use peers::{Peer, PeerStore};

pub const PROTOCOL_VERSION: u32 = 1;
/// La versión de esta app, la misma en la PC y en el móvil.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Puerto TCP/UDP de la app. Está dentro del rango abierto para KDE Connect,
/// pero KDE Connect solo usa de 1716 en adelante.
pub const PORT: u16 = 1714;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("error de red: {0}")]
    Io(#[from] std::io::Error),
    #[error("error de TLS: {0}")]
    Tls(#[from] rustls::Error),
    #[error("no se pudo generar el certificado: {0}")]
    Certificate(#[from] rcgen::Error),
    #[error("mensaje no válido: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Protocol(String),
    #[error("El emparejamiento fue rechazado")]
    Rejected,
    #[error("Se agotó el tiempo de espera")]
    Timeout,
}

pub type Result<T> = std::result::Result<T, Error>;
