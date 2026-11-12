//! Identidad de Puente en Windows. Sin un identificador registrado, los avisos salen
//! a nombre de PowerShell.

use std::path::Path;

use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

pub const APP_ID: &str = "dev.puente.app";

/// Registra el nombre y el icono para el usuario actual; no necesita administrador.
/// Si falla, los avisos siguen saliendo, solo que sin nombre propio.
pub fn register_app(data_dir: &Path, icon_png: &[u8]) {
    let icon = data_dir.join("puente.png");
    let _ = std::fs::create_dir_all(data_dir);
    let _ = std::fs::write(&icon, icon_png);

    let path = format!(r"Software\Classes\AppUserModelId\{APP_ID}");
    if let Ok((key, _)) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(path) {
        let _ = key.set_value("DisplayName", &"Puente");
        let _ = key.set_value("IconUri", &icon.display().to_string());
    }
}
