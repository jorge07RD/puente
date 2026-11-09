#!/usr/bin/env bash
# Compila la librería Rust para Android y genera el APK.
# Uso: build-android.sh [debug|release]; por defecto, debug.
# Si existe un Rust aislado en ~/.local/share/puente lo usa; si no, el del sistema.
# Se puede cambiar cada ruta con ANDROID_HOME, ANDROID_NDK_HOME y PUENTE_JAVA_HOME.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tools="$HOME/.local/share/puente"

if [ -d "$tools/cargo" ]; then
    export CARGO_HOME="$tools/cargo"
    export RUSTUP_HOME="$tools/rustup"
    export PATH="$CARGO_HOME/bin:$PATH"
fi
export CARGO_TARGET_DIR="$root/target/android"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/29.0.14206865}"
export JAVA_HOME="${PUENTE_JAVA_HOME:-/usr/lib/jvm/java-17-openjdk}"

cd "$root"
cargo ndk -t arm64-v8a -P 29 -o android/app/src/main/jniLibs build -p puente-android --release

cd "$root/android"
variant="${1:-debug}"
./gradlew --console=plain "assemble${variant^}"

echo "APK: $root/android/app/build/outputs/apk/$variant/app-$variant.apk"
