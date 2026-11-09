//! Recepción de archivos.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{Error, Result};

const MAX_PARALLEL_FILES: usize = 16;
const MAX_NAME_BYTES: usize = 200;

/// El nombre lo envía el otro dispositivo: se reduce a un nombre simple para que
/// no pueda escribir fuera de la carpeta de destino.
pub fn safe_name(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next()?;
    let mut clean: String = base.chars().filter(|c| !c.is_control()).collect();
    clean = clean.trim().to_string();
    if clean.is_empty() || clean.chars().all(|c| c == '.') {
        return None;
    }
    while clean.len() > MAX_NAME_BYTES {
        clean.pop();
    }
    Some(clean)
}

fn unused_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let extension = path.extension().and_then(|e| e.to_str());
    (1..)
        .map(|number| match extension {
            Some(extension) => dir.join(format!("{stem} ({number}).{extension}")),
            None => dir.join(format!("{stem} ({number})")),
        })
        .find(|candidate| !candidate.exists())
        .expect("hay nombres libres")
}

#[derive(Clone, Debug, PartialEq)]
pub struct Received {
    pub path: PathBuf,
    pub name: String,
}

struct Partial {
    file: File,
    temporary: PathBuf,
    name: String,
    expected: u64,
    written: u64,
}

/// Archivos a medio recibir durante una sesión.
pub struct Incoming {
    dir: PathBuf,
    partial: HashMap<u32, Partial>,
}

impl Incoming {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            partial: HashMap::new(),
        }
    }

    pub fn offer(&mut self, id: u32, name: &str, size: u64) -> Result<()> {
        if self.partial.len() >= MAX_PARALLEL_FILES {
            return Err(Error::Protocol("demasiados archivos a la vez".to_string()));
        }
        let name = safe_name(name)
            .ok_or_else(|| Error::Protocol("nombre de archivo no válido".to_string()))?;
        fs::create_dir_all(&self.dir)?;
        let temporary = self.dir.join(format!(".puente-{id}.part"));
        let file = File::create(&temporary)?;
        self.partial.insert(
            id,
            Partial {
                file,
                temporary,
                name,
                expected: size,
                written: 0,
            },
        );
        Ok(())
    }

    pub fn chunk(&mut self, id: u32, data: &[u8]) -> Result<()> {
        let partial = self
            .partial
            .get_mut(&id)
            .ok_or_else(|| Error::Protocol("archivo desconocido".to_string()))?;
        let written = partial.written + data.len() as u64;
        if written > partial.expected {
            return Err(Error::Protocol("el archivo supera su tamaño".to_string()));
        }
        partial.file.write_all(data)?;
        partial.written = written;
        Ok(())
    }

    pub fn finish(&mut self, id: u32) -> Result<Received> {
        let partial = self
            .partial
            .remove(&id)
            .ok_or_else(|| Error::Protocol("archivo desconocido".to_string()))?;
        if partial.written != partial.expected {
            let _ = fs::remove_file(&partial.temporary);
            return Err(Error::Protocol("el archivo llegó incompleto".to_string()));
        }
        partial.file.sync_all()?;
        let path = unused_path(&self.dir, &partial.name);
        fs::rename(&partial.temporary, &path)?;
        Ok(Received {
            path,
            name: partial.name,
        })
    }

    pub fn fail(&mut self, id: u32) {
        if let Some(partial) = self.partial.remove(&id) {
            let _ = fs::remove_file(partial.temporary);
        }
    }

    pub fn fail_all(&mut self) {
        let ids: Vec<u32> = self.partial.keys().copied().collect();
        for id in ids {
            self.fail(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_cannot_escape_the_folder() {
        assert_eq!(safe_name("../../.ssh/authorized_keys").unwrap(), "authorized_keys");
        assert_eq!(safe_name("..\\..\\win.ini").unwrap(), "win.ini");
        assert_eq!(safe_name("/etc/passwd").unwrap(), "passwd");
        assert_eq!(safe_name("foto\n.jpg").unwrap(), "foto.jpg");
        assert_eq!(safe_name(".."), None);
        assert_eq!(safe_name("a/.."), None);
        assert_eq!(safe_name("   "), None);
    }

    #[test]
    fn long_names_are_cut_on_a_character_boundary() {
        let name = "ñ".repeat(150);
        let safe = safe_name(&name).unwrap();
        assert!(safe.len() <= MAX_NAME_BYTES);
    }

    #[test]
    fn a_file_arrives_and_never_overwrites_another() {
        let dir = tempfile::tempdir().unwrap();
        let mut incoming = Incoming::new(dir.path().to_path_buf());

        for expected in ["a.txt", "a (1).txt"] {
            incoming.offer(1, "a.txt", 5).unwrap();
            incoming.chunk(1, b"hel").unwrap();
            incoming.chunk(1, b"lo").unwrap();
            let received = incoming.finish(1).unwrap();
            assert_eq!(received.path, dir.path().join(expected));
            assert_eq!(fs::read(received.path).unwrap(), b"hello");
        }
    }

    #[test]
    fn a_file_cannot_exceed_or_fall_short_of_its_size() {
        let dir = tempfile::tempdir().unwrap();
        let mut incoming = Incoming::new(dir.path().to_path_buf());

        incoming.offer(1, "a", 2).unwrap();
        assert!(incoming.chunk(1, b"abc").is_err());
        incoming.fail(1);

        incoming.offer(2, "b", 5).unwrap();
        incoming.chunk(2, b"ab").unwrap();
        assert!(incoming.finish(2).is_err());

        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
