//! Conexión ya autenticada: envía y recibe, y detecta cortes.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncWrite, split};
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval, timeout};

use crate::wire::{Frame, Message, VideoPacket, read_frame, write_frame};
use crate::{Error, Result};

const PING_EVERY: Duration = Duration::from_secs(2);
const DEAD_AFTER: Duration = Duration::from_secs(15);

pub struct Outgoing {
    pub control: mpsc::Receiver<Message>,
    pub video: mpsc::Receiver<VideoPacket>,
    pub audio: mpsc::Receiver<Vec<u8>>,
    /// Oferta, trozos y final de cada archivo, en orden.
    pub files: mpsc::Receiver<Frame>,
}

/// Corre hasta que la conexión se corta o deja de responder. Lo recibido, salvo
/// los mensajes de mantenimiento, se entrega a `on_frame`; `on_round_trip` recibe
/// el tiempo que tarda en volver cada ping.
pub async fn run<S>(
    stream: S,
    outgoing: Outgoing,
    mut on_frame: impl FnMut(Frame),
    mut on_round_trip: impl FnMut(Duration),
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (mut reader, mut writer) = split(stream);
    let (replies, mut pending) = mpsc::channel::<Message>(16);
    let Outgoing {
        mut control,
        mut video,
        mut audio,
        mut files,
    } = outgoing;

    let ping_sent = Arc::new(Mutex::new(None::<Instant>));
    let ping_clock = ping_sent.clone();

    let sender = tokio::spawn(async move {
        let mut ticker = interval(PING_EVERY);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            // El control va primero: un vídeo pendiente no debe retrasar órdenes.
            let frame = tokio::select! {
                biased;
                reply = pending.recv() => reply.map(Frame::Control),
                message = control.recv() => message.map(Frame::Control),
                _ = ticker.tick() => Some(Frame::Control(Message::Ping)),
                samples = audio.recv() => samples.map(Frame::Audio),
                packet = video.recv() => packet.map(Frame::Video),
                piece = files.recv() => piece,
            };
            let Some(frame) = frame else { return };
            if frame == Frame::Control(Message::Ping) {
                *ping_clock.lock().expect("reloj disponible") = Some(Instant::now());
            }
            if write_frame(&mut writer, &frame).await.is_err() {
                return;
            }
        }
    });

    let result = loop {
        let frame = match timeout(DEAD_AFTER, read_frame(&mut reader)).await {
            Ok(Ok(frame)) => frame,
            Ok(Err(error)) => break Err(error),
            Err(_) => break Err(Error::Timeout),
        };
        match frame {
            Frame::Control(Message::Ping) => {
                if replies.send(Message::Pong).await.is_err() {
                    break Ok(());
                }
            }
            Frame::Control(Message::Pong) => {
                if let Some(sent) = ping_sent.lock().expect("reloj disponible").take() {
                    on_round_trip(sent.elapsed());
                }
            }
            other => on_frame(other),
        }
    };

    sender.abort();
    result
}
