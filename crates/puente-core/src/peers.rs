//! Dispositivos emparejados, guardados en disco.

use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{Fingerprint, Result};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Peer {
    pub name: String,
    pub fingerprint: Fingerprint,
    #[serde(default)]
    pub last_ip: Option<IpAddr>,
}

pub struct PeerStore {
    path: PathBuf,
    peers: Vec<Peer>,
}

impl PeerStore {
    pub fn load(path: PathBuf) -> Result<Self> {
        let peers = match fs::read(&path) {
            Ok(contents) => serde_json::from_slice(&contents)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error.into()),
        };
        Ok(Self { path, peers })
    }

    pub fn list(&self) -> Vec<Peer> {
        self.peers.clone()
    }

    pub fn is_paired(&self, fingerprint: Fingerprint) -> bool {
        self.peers.iter().any(|p| p.fingerprint == fingerprint)
    }

    pub fn add(&mut self, name: &str, fingerprint: Fingerprint) -> Result<()> {
        self.peers.retain(|p| p.fingerprint != fingerprint);
        self.peers.push(Peer {
            name: name.to_string(),
            fingerprint,
            last_ip: None,
        });
        self.save()
    }

    pub fn remember_ip(&mut self, fingerprint: Fingerprint, ip: IpAddr) -> Result<()> {
        let Some(peer) = self.peers.iter_mut().find(|p| p.fingerprint == fingerprint) else {
            return Ok(());
        };
        if peer.last_ip == Some(ip) {
            return Ok(());
        }
        peer.last_ip = Some(ip);
        self.save()
    }

    pub fn remove(&mut self, fingerprint: Fingerprint) -> Result<()> {
        self.peers.retain(|p| p.fingerprint != fingerprint);
        self.save()
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = self.path.with_extension("tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(&self.peers)?)?;
        fs::rename(temporary, &self.path)?;
        Ok(())
    }
}
