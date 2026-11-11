//! Formato de los mensajes: 4 bytes de longitud, 1 de tipo y el contenido.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::settings::VideoQuality;
use crate::{Error, Fingerprint, Result};

/// Límite durante el saludo, cuando el otro lado aún no está autenticado.
const MAX_CONTROL_FRAME: usize = 1 << 20;
/// Límite una vez autenticado: un fotograma clave puede ocupar varios megabytes.
const MAX_FRAME: usize = 8 << 20;

const KIND_CONTROL: u8 = 0;
const KIND_VIDEO: u8 = 1;
const KIND_AUDIO: u8 = 2;
const KIND_FILE: u8 = 3;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Hello {
        protocol: u32,
        name: String,
        known: bool,
    },
    PairCommit {
        commit: String,
    },
    PairNonce {
        nonce: String,
    },
    PairReveal {
        nonce: String,
    },
    PairDecision {
        accepted: bool,
    },
    Ping,
    Pong,
    RequestKeyframe,
    MirrorStopped,
    Input(Input),
    InputState {
        enabled: bool,
    },
    Notification(NotificationInfo),
    NotificationRemoved {
        key: String,
    },
    Battery {
        percent: u8,
        charging: bool,
    },
    Media {
        info: Option<MediaInfo>,
    },
    Clipboard {
        text: String,
    },
    FileOffer {
        id: u32,
        name: String,
        size: u64,
    },
    FileDone {
        id: u32,
    },
    FileFailed {
        id: u32,
    },
    /// El otro lado ya guardó el archivo completo.
    FileSaved {
        id: u32,
    },
    Command(PhoneCommand),
    Call(CallInfo),
    CallEnded {
        key: String,
    },
    ReplyResult(ReplyOutcome),
    /// El otro lado deshizo el emparejamiento.
    Unpair,
    /// Registro del móvil, que sin ADB no se podría leer desde la PC.
    Log {
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NotificationInfo {
    pub key: String,
    pub app: String,
    pub title: String,
    pub text: String,
    /// La app que la envió admite respuesta directa.
    #[serde(default)]
    pub can_reply: bool,
}

/// Una llamada entrante, de teléfono o de cualquier app.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CallInfo {
    pub key: String,
    pub app: String,
    pub caller: String,
    pub can_answer: bool,
    pub can_decline: bool,
}

/// Resultado de responder a una notificación.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReplyOutcome {
    pub key: String,
    pub delivered: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub playing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaAction {
    PlayPause,
    Next,
    Previous,
}

/// Órdenes de la PC al móvil que no dependen de que se comparta la pantalla.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum PhoneCommand {
    Dismiss { key: String },
    Ring { on: bool },
    Media { action: MediaAction },
    StopMirror,
    Reply { key: String, text: String },
    CallAction { key: String, answer: bool },
    /// Callar la música en el móvil porque ya suena en la PC.
    MutePhone { on: bool },
    /// Cambiar la calidad del vídeo; el móvil reinicia el codificador.
    Quality { level: VideoQuality },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TouchPhase {
    Down,
    Move,
    Up,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavAction {
    Back,
    Home,
    Recents,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyPress {
    Backspace,
    Enter,
}

/// Órdenes de la PC al móvil. Las coordenadas van de 0 a 1 sobre la pantalla.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "input", rename_all = "snake_case")]
pub enum Input {
    Touch { phase: TouchPhase, x: f32, y: f32 },
    Scroll { x: f32, y: f32, lines: f32 },
    Navigate { action: NavAction },
    Text { text: String },
    Key { key: KeyPress },
    Cancel,
}

/// Un paquete H.264 tal como sale del codificador del móvil.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoPacket {
    pub flags: u8,
    pub data: Vec<u8>,
}

impl VideoPacket {
    /// Los mismos valores que `MediaCodec.BUFFER_FLAG_*`.
    pub const KEY: u8 = 1;
    pub const CONFIG: u8 = 2;

    pub fn is_key(&self) -> bool {
        self.flags & Self::KEY != 0
    }

    pub fn is_config(&self) -> bool {
        self.flags & Self::CONFIG != 0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Frame {
    Control(Message),
    Video(VideoPacket),
    /// PCM de 16 bits, 48 kHz, estéreo.
    Audio(Vec<u8>),
    File { id: u32, data: Vec<u8> },
}

/// Mensajes UDP de descubrimiento. Solo son una pista de dónde conectarse:
/// la identidad real se comprueba después con TLS.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Discovery {
    Probe {
        protocol: u32,
    },
    Announce {
        protocol: u32,
        name: String,
        port: u16,
        fingerprint: Fingerprint,
    },
}

pub async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, frame: &Frame) -> Result<()> {
    let mut payload;
    let (kind, body): (u8, &[u8]) = match frame {
        Frame::Control(message) => {
            payload = serde_json::to_vec(message)?;
            (KIND_CONTROL, &payload)
        }
        Frame::Video(packet) => {
            payload = Vec::with_capacity(packet.data.len() + 1);
            payload.push(packet.flags);
            payload.extend_from_slice(&packet.data);
            (KIND_VIDEO, &payload)
        }
        Frame::Audio(samples) => (KIND_AUDIO, samples),
        Frame::File { id, data } => {
            payload = Vec::with_capacity(data.len() + 4);
            payload.extend_from_slice(&id.to_be_bytes());
            payload.extend_from_slice(data);
            (KIND_FILE, &payload)
        }
    };
    if body.len() > MAX_FRAME {
        return Err(Error::Protocol("mensaje demasiado grande".to_string()));
    }
    writer.write_all(&(body.len() as u32).to_be_bytes()).await?;
    writer.write_all(&[kind]).await?;
    writer.write_all(body).await?;
    writer.flush().await?;
    Ok(())
}

async fn read_raw<R: AsyncRead + Unpin>(reader: &mut R, limit: usize) -> Result<(u8, Vec<u8>)> {
    let mut prefix = [0; 4];
    reader.read_exact(&mut prefix).await?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length > limit {
        return Err(Error::Protocol("mensaje demasiado grande".to_string()));
    }
    let mut kind = [0; 1];
    reader.read_exact(&mut kind).await?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body).await?;
    Ok((kind[0], body))
}

pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Frame> {
    let (kind, body) = read_raw(reader, MAX_FRAME).await?;
    match kind {
        KIND_CONTROL => Ok(Frame::Control(serde_json::from_slice(&body)?)),
        KIND_VIDEO => {
            let (&flags, data) = body
                .split_first()
                .ok_or_else(|| Error::Protocol("paquete de vídeo vacío".to_string()))?;
            Ok(Frame::Video(VideoPacket {
                flags,
                data: data.to_vec(),
            }))
        }
        KIND_AUDIO => Ok(Frame::Audio(body)),
        KIND_FILE => {
            if body.len() < 4 {
                return Err(Error::Protocol("paquete de archivo vacío".to_string()));
            }
            let (id, data) = body.split_at(4);
            Ok(Frame::File {
                id: u32::from_be_bytes(id.try_into().expect("son 4 bytes")),
                data: data.to_vec(),
            })
        }
        _ => Err(Error::Protocol("tipo de mensaje desconocido".to_string())),
    }
}

pub async fn write_message<W: AsyncWrite + Unpin>(writer: &mut W, message: &Message) -> Result<()> {
    write_frame(writer, &Frame::Control(message.clone())).await
}

/// Lee un mensaje de control durante el saludo, con el límite pequeño.
pub async fn read_message<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Message> {
    let (kind, body) = read_raw(reader, MAX_CONTROL_FRAME).await?;
    if kind != KIND_CONTROL {
        return Err(Error::Protocol("mensaje inesperado durante el saludo".to_string()));
    }
    Ok(serde_json::from_slice(&body)?)
}
