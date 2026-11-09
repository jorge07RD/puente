//! Identidad del dispositivo: certificado autofirmado, clave y huella.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use ring::digest;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Error, Result};

const CERT_FILE: &str = "identity.cert.der";
const KEY_FILE: &str = "identity.key.der";

/// SHA-256 del certificado: lo que identifica a un dispositivo.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint(pub [u8; 32]);

impl Fingerprint {
    pub fn of(certificate: &[u8]) -> Self {
        let hash = digest::digest(&digest::SHA256, certificate);
        let mut bytes = [0; 32];
        bytes.copy_from_slice(hash.as_ref());
        Self(bytes)
    }

    pub fn from_hex(text: &str) -> Option<Self> {
        let bytes = decode_hex(text)?;
        Some(Self(bytes.try_into().ok()?))
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&encode_hex(&self.0))
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({self})")
    }
}

impl Serialize for Fingerprint {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Fingerprint {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::from_hex(&text).ok_or_else(|| serde::de::Error::custom("huella no válida"))
    }
}

pub(crate) fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok())
        .collect()
}

pub struct Identity {
    pub name: String,
    pub fingerprint: Fingerprint,
    certificate: CertificateDer<'static>,
    key: Vec<u8>,
}

impl Identity {
    pub fn generate(name: &str) -> Result<Self> {
        let generated = rcgen::generate_simple_self_signed(vec!["puente.local".to_string()])?;
        Ok(Self::from_parts(
            name,
            generated.cert.der().to_vec(),
            generated.signing_key.serialize_der(),
        ))
    }

    /// Carga la identidad guardada o crea una nueva si no existe.
    pub fn load_or_create(dir: &Path, name: &str) -> Result<Self> {
        let cert_path = dir.join(CERT_FILE);
        let key_path = dir.join(KEY_FILE);

        if cert_path.exists() && key_path.exists() {
            return Ok(Self::from_parts(
                name,
                fs::read(cert_path)?,
                fs::read(key_path)?,
            ));
        }

        fs::create_dir_all(dir)?;
        let identity = Self::generate(name)?;
        write_private(&key_path, &identity.key)?;
        fs::write(cert_path, identity.certificate.as_ref())?;
        Ok(identity)
    }

    fn from_parts(name: &str, certificate: Vec<u8>, key: Vec<u8>) -> Self {
        Self {
            name: name.to_string(),
            fingerprint: Fingerprint::of(&certificate),
            certificate: CertificateDer::from(certificate),
            key,
        }
    }

    pub(crate) fn certificate(&self) -> CertificateDer<'static> {
        self.certificate.clone()
    }

    pub(crate) fn private_key(&self) -> PrivateKeyDer<'static> {
        PrivatePkcs8KeyDer::from(self.key.clone()).into()
    }
}

fn write_private(path: &Path, contents: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(Error::Io)?;
    file.write_all(contents)?;
    Ok(())
}
