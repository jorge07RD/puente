//! Saludo inicial y emparejamiento.
//!
//! El código de 6 dígitos es corto, así que por sí solo no basta: un atacante en
//! la red podría generar certificados hasta que los códigos coincidieran. Por eso
//! la PC se compromete con su valor aleatorio antes de ver el del móvil, y ninguno
//! de los dos lados puede ajustar el resultado.

use std::sync::Mutex;
use std::time::Duration;

use ring::digest;
use ring::rand::{SecureRandom, SystemRandom};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::oneshot;
use tokio::time::timeout;

use crate::identity::{decode_hex, encode_hex};
use crate::wire::{Message, read_message, write_message};
use crate::{APP_VERSION, Error, Fingerprint, Identity, PROTOCOL_VERSION, PeerStore, Result};

const STEP_TIMEOUT: Duration = Duration::from_secs(10);
const USER_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Desktop,
    Phone,
}

pub struct PairPrompt {
    pub peer_name: String,
    pub code: String,
}

/// Lo que el otro lado contó de sí mismo al saludar.
#[derive(Clone, Debug, PartialEq)]
pub struct Greeting {
    pub name: String,
    /// Vacía si el otro lado es anterior a la 0.3.1.
    pub version: String,
}

/// Completa el saludo y, si hace falta, el emparejamiento.
pub async fn establish<S>(
    stream: &mut S,
    role: Role,
    identity: &Identity,
    peer: Fingerprint,
    store: &Mutex<PeerStore>,
    prompt: impl FnOnce(PairPrompt) -> oneshot::Receiver<bool>,
) -> Result<Greeting>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let known = store.lock().expect("almacén disponible").is_paired(peer);
    let hello = Message::Hello {
        protocol: PROTOCOL_VERSION,
        name: identity.name.clone(),
        known,
        version: APP_VERSION.to_string(),
    };
    write_message(stream, &hello).await?;

    let Message::Hello {
        protocol,
        name,
        known: peer_knows_us,
        version,
    } = read_step(stream).await?
    else {
        return Err(unexpected());
    };
    if protocol != PROTOCOL_VERSION {
        return Err(Error::Protocol(
            "Las dos apps tienen versiones incompatibles".to_string(),
        ));
    }

    let greeting = Greeting {
        name: name.clone(),
        version,
    };
    if known && peer_knows_us {
        return Ok(greeting);
    }

    let nonces = exchange_nonces(stream, role).await?;
    let (desktop, phone) = match role {
        Role::Desktop => (identity.fingerprint, peer),
        Role::Phone => (peer, identity.fingerprint),
    };
    let code = short_code(desktop, phone, &nonces.desktop, &nonces.phone);

    let answer = prompt(PairPrompt {
        peer_name: name.clone(),
        code,
    });
    let accepted = matches!(timeout(USER_TIMEOUT, answer).await, Ok(Ok(true)));
    write_message(stream, &Message::PairDecision { accepted }).await?;
    if !accepted {
        return Err(Error::Rejected);
    }

    let reply = timeout(USER_TIMEOUT, read_message(stream))
        .await
        .map_err(|_| Error::Timeout)??;
    let Message::PairDecision { accepted: true } = reply else {
        return Err(Error::Rejected);
    };

    store.lock().expect("almacén disponible").add(&name, peer)?;
    Ok(greeting)
}

struct Nonces {
    desktop: Vec<u8>,
    phone: Vec<u8>,
}

async fn exchange_nonces<S>(stream: &mut S, role: Role) -> Result<Nonces>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let own = random_nonce()?;
    match role {
        Role::Desktop => {
            let commit = encode_hex(digest::digest(&digest::SHA256, &own).as_ref());
            write_message(stream, &Message::PairCommit { commit }).await?;
            let Message::PairNonce { nonce } = read_step(stream).await? else {
                return Err(unexpected());
            };
            let reveal = Message::PairReveal {
                nonce: encode_hex(&own),
            };
            write_message(stream, &reveal).await?;
            Ok(Nonces {
                desktop: own,
                phone: parse_nonce(&nonce)?,
            })
        }
        Role::Phone => {
            let Message::PairCommit { commit } = read_step(stream).await? else {
                return Err(unexpected());
            };
            let nonce = Message::PairNonce {
                nonce: encode_hex(&own),
            };
            write_message(stream, &nonce).await?;
            let Message::PairReveal { nonce } = read_step(stream).await? else {
                return Err(unexpected());
            };
            let desktop = parse_nonce(&nonce)?;
            verify_commit(&commit, &desktop)?;
            Ok(Nonces {
                desktop,
                phone: own,
            })
        }
    }
}

fn verify_commit(commit: &str, nonce: &[u8]) -> Result<()> {
    let expected = encode_hex(digest::digest(&digest::SHA256, nonce).as_ref());
    if expected == commit {
        Ok(())
    } else {
        Err(Error::Protocol(
            "La comprobación del emparejamiento falló".to_string(),
        ))
    }
}

fn short_code(desktop: Fingerprint, phone: Fingerprint, nonce_d: &[u8], nonce_p: &[u8]) -> String {
    let mut context = digest::Context::new(&digest::SHA256);
    context.update(b"puente-pairing-v1");
    context.update(&desktop.0);
    context.update(&phone.0);
    context.update(nonce_d);
    context.update(nonce_p);
    let hash = context.finish();

    let mut prefix = [0; 4];
    prefix.copy_from_slice(&hash.as_ref()[..4]);
    format!("{:06}", u32::from_be_bytes(prefix) % 1_000_000)
}

fn random_nonce() -> Result<Vec<u8>> {
    let mut nonce = vec![0; 32];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| Error::Protocol("no se pudieron generar datos aleatorios".to_string()))?;
    Ok(nonce)
}

fn parse_nonce(text: &str) -> Result<Vec<u8>> {
    match decode_hex(text) {
        Some(nonce) if nonce.len() == 32 => Ok(nonce),
        _ => Err(Error::Protocol("valor de emparejamiento no válido".to_string())),
    }
}

async fn read_step<S: AsyncRead + Unpin>(stream: &mut S) -> Result<Message> {
    timeout(STEP_TIMEOUT, read_message(stream))
        .await
        .map_err(|_| Error::Timeout)?
}

fn unexpected() -> Error {
    Error::Protocol("mensaje inesperado durante el saludo".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(byte: u8) -> Fingerprint {
        Fingerprint([byte; 32])
    }

    #[test]
    fn code_has_six_digits_and_is_deterministic() {
        let a = short_code(fingerprint(1), fingerprint(2), &[3; 32], &[4; 32]);
        let b = short_code(fingerprint(1), fingerprint(2), &[3; 32], &[4; 32]);
        assert_eq!(a, b);
        assert_eq!(a.len(), 6);
        assert!(a.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn code_depends_on_certificates_and_nonces() {
        let base = short_code(fingerprint(1), fingerprint(2), &[3; 32], &[4; 32]);
        assert_ne!(base, short_code(fingerprint(9), fingerprint(2), &[3; 32], &[4; 32]));
        assert_ne!(base, short_code(fingerprint(1), fingerprint(9), &[3; 32], &[4; 32]));
        assert_ne!(base, short_code(fingerprint(1), fingerprint(2), &[9; 32], &[4; 32]));
        assert_ne!(base, short_code(fingerprint(1), fingerprint(2), &[3; 32], &[9; 32]));
    }

    #[test]
    fn a_reveal_that_differs_from_the_commit_is_refused() {
        let commit = encode_hex(digest::digest(&digest::SHA256, &[1; 32]).as_ref());
        assert!(verify_commit(&commit, &[1; 32]).is_ok());
        assert!(verify_commit(&commit, &[2; 32]).is_err());
    }
}
