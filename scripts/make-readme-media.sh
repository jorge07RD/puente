#!/usr/bin/env bash
# Regenera las capturas y el GIF del README con datos de ejemplo.
# Necesita una sesión gráfica, ffmpeg e ImageMagick.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
media="$root/assets/readme"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cd "$root"
cargo build -p puente-desktop --release
mkdir -p "$media"

# Español en assets/readme; inglés y portugués en sus subcarpetas, con los mismos nombres.
for language in es en pt; do
    out="$media"
    [ "$language" = es ] || out="$media/$language"
    mkdir -p "$out"
    rm -rf "$work/pc" "$work/phone"
    target/release/puente --lang "$language" --demo "$work/pc"
    target/release/puente --lang "$language" --demo-phone "$work/phone"

    still() { magick "$work/$1/frame-$(printf %04d "$2").ppm" -strip "$out/$3.png"; }
    still pc 33 buscando
    still pc 44 emparejar
    still pc 75 tarjetas
    still pc 128 transmision
    still phone 1 movil-buscando
    still phone 6 movil-emparejar
    still phone 14 movil-compartiendo

    ffmpeg -y -loglevel error -framerate 15 -i "$work/pc/frame-%04d.ppm" \
        -vf "scale=960:-2:flags=lanczos,fps=12,split[a][b];[a]palettegen=max_colors=48:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle" \
        "$out/demo.gif"
done

ls -la "$media" "$media/en" "$media/pt"
