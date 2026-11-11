//! Arranque con la sesión, con una entrada en `~/.config/autostart` que la mayoría de
//! escritorios leen, también Hyprland con uwsm.

use std::io;
use std::path::PathBuf;

fn entry_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("autostart").join("puente.desktop"))
}

pub fn is_enabled() -> bool {
    entry_path().is_some_and(|path| path.is_file())
}

pub fn set_enabled(enabled: bool) -> io::Result<()> {
    let path = entry_path().ok_or_else(|| io::Error::other("sin carpeta de configuración"))?;
    if !enabled {
        return match std::fs::remove_file(&path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    let binary = std::env::current_exe()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, entry(&binary.to_string_lossy()))
}

/// Las comillas protegen rutas con espacios; `--hidden` la deja en la bandeja.
fn entry(binary: &str) -> String {
    let quoted = binary.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Puente\n\
         Exec=\"{quoted}\" --hidden\n\
         Icon=puente\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_starts_hidden_and_survives_spaces_in_the_path() {
        let text = entry("/home/ana/mis programas/puente");
        assert!(text.contains("Exec=\"/home/ana/mis programas/puente\" --hidden\n"));
        assert!(text.starts_with("[Desktop Entry]\n"));
    }
}
