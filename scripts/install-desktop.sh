#!/usr/bin/env bash
# Compila Puente y lo añade al menú de aplicaciones del usuario.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
binary="$HOME/.local/bin/puente"

cd "$root"
cargo build -p puente-desktop --release

install -Dm755 target/release/puente "$binary"
install -Dm644 assets/puente.svg "$data/icons/hicolor/scalable/apps/puente.svg"
install -d "$data/applications"
cat > "$data/applications/puente.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Puente
Comment=Ve y controla tu móvil Android desde la PC
Exec=$binary
Icon=puente
Terminal=false
Categories=Utility;Network;
StartupWMClass=puente
DESKTOP

echo "Puente instalado en $binary"
