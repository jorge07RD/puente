//! Preferencias del usuario, guardadas en disco.

use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Result;

/// Idioma de la interfaz. `Auto` sigue al del sistema.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguageChoice {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "es")]
    Spanish,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "pt")]
    Portuguese,
}

impl LanguageChoice {
    /// El siguiente al pulsar el selector.
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Spanish,
            Self::Spanish => Self::English,
            Self::English => Self::Portuguese,
            Self::Portuguese => Self::Auto,
        }
    }
}

/// Resolución, nitidez y fluidez del vídeo que envía el móvil.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoQuality {
    /// 1920 px a 60 fps, con los datos ajustados a lo que aguanta la red.
    #[default]
    Auto,
    /// Menos red y batería: 1280 px a 30 fps.
    Saver,
    /// 1920 px a 60 fps. Es la que usa el móvil si nadie le pide otra.
    Balanced,
    /// 2560 px a 60 fps, con el doble de datos.
    High,
}

impl VideoQuality {
    /// La siguiente al pulsar el selector.
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Saver,
            Self::Saver => Self::Balanced,
            Self::Balanced => Self::High,
            Self::High => Self::Auto,
        }
    }
}

/// Qué hace la PC con los avisos que llegan del móvil.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeMode {
    /// En la tarjeta y como aviso del escritorio.
    #[default]
    On,
    /// Solo en la tarjeta, sin aviso del escritorio.
    Silent,
    /// Ni en la tarjeta ni en el escritorio.
    Off,
}

impl NoticeMode {
    /// El siguiente al pulsar el selector.
    pub fn next(self) -> Self {
        match self {
            Self::On => Self::Silent,
            Self::Silent => Self::Off,
            Self::Off => Self::On,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub animations: bool,
    /// Reproducir en la PC el audio del móvil.
    pub audio: bool,
    /// Mientras se transmite, el audio del móvil se oye solo en la PC.
    pub phone_muted: bool,
    pub language: LanguageChoice,
    pub quality: VideoQuality,
    pub notices: NoticeMode,
    /// Móvil: dirección de la PC escrita a mano, para redes donde no se encuentra sola.
    pub manual_ip: Option<IpAddr>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            animations: true,
            audio: true,
            phone_muted: false,
            language: LanguageChoice::Auto,
            quality: VideoQuality::Auto,
            notices: NoticeMode::On,
            manual_ip: None,
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
    fn the_language_is_saved_with_a_short_code() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut file = SettingsFile::load(path.clone());
        assert_eq!(file.current.language, LanguageChoice::Auto);
        file.current.language = LanguageChoice::Portuguese;
        file.save().unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("\"pt\""));
        assert_eq!(SettingsFile::load(path).current.language, LanguageChoice::Portuguese);
    }

    #[test]
    fn the_quality_is_saved_and_cycles_through_the_four_levels() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut file = SettingsFile::load(path.clone());
        assert_eq!(file.current.quality, VideoQuality::Auto);
        file.current.quality = VideoQuality::High;
        file.save().unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("\"high\""));
        assert_eq!(SettingsFile::load(path).current.quality, VideoQuality::High);
        let start = VideoQuality::Auto;
        assert_eq!(start.next().next().next().next(), start);
    }

    #[test]
    fn the_notice_mode_is_saved_and_cycles_through_the_three_modes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut file = SettingsFile::load(path.clone());
        assert_eq!(file.current.notices, NoticeMode::On);
        file.current.notices = NoticeMode::Silent;
        file.save().unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("\"silent\""));
        assert_eq!(SettingsFile::load(path).current.notices, NoticeMode::Silent);
        let start = NoticeMode::On;
        assert_eq!(start.next().next().next(), start);
    }

    #[test]
    fn the_manual_address_is_saved_and_can_be_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut file = SettingsFile::load(path.clone());
        assert_eq!(file.current.manual_ip, None);
        file.current.manual_ip = Some("192.168.1.20".parse().unwrap());
        file.save().unwrap();
        let mut file = SettingsFile::load(path.clone());
        assert_eq!(file.current.manual_ip, Some("192.168.1.20".parse().unwrap()));
        file.current.manual_ip = None;
        file.save().unwrap();
        assert_eq!(SettingsFile::load(path).current.manual_ip, None);
    }

    #[test]
    fn a_damaged_file_falls_back_to_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"{ esto no es json").unwrap();
        assert_eq!(SettingsFile::load(path).current, Settings::default());
    }
}
