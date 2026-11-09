//! Preferencias del usuario, guardadas en disco.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Result;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub animations: bool,
    /// Reproducir en la PC el audio del móvil.
    pub audio: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            animations: true,
            audio: true,
        }
    }
}

pub struct SettingsFile {
    path: PathBuf,
    pub current: Settings,
}

impl SettingsFile {
    /// Un archivo dañado no debe impedir que la app arranque: se usan los valores por defecto.
    pub fn load(path: PathBuf) -> Self {
        let current = read(&path).unwrap_or_default();
        Self { path, current }
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = self.path.with_extension("tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(&self.current)?)?;
        fs::rename(temporary, &self.path)?;
        Ok(())
    }
}

fn read(path: &Path) -> Option<Settings> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_choice_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut file = SettingsFile::load(path.clone());
        assert!(file.current.animations);
        file.current.animations = false;
        file.save().unwrap();
        assert!(!SettingsFile::load(path).current.animations);
    }

    #[test]
    fn a_damaged_file_falls_back_to_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"{ esto no es json").unwrap();
        assert_eq!(SettingsFile::load(path).current, Settings::default());
    }
}
