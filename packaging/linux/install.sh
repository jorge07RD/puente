#!/usr/bin/env bash
# Instala Puente para el usuario actual, sin necesidad de permisos de administrador.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
binary="$HOME/.local/bin/puente"

install -Dm755 "$here/puente" "$binary"
install -Dm644 "$here/puente.svg" "$data/icons/hicolor/scalable/apps/puente.svg"
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

echo "Puente instalado en $binary. Búscalo en el menú de aplicaciones."
