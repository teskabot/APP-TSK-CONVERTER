#!/usr/bin/env bash
# Instalador para compartir: .dmg universal (Intel + Apple Silicon), con HEIC,
# sin depender de nada instalado en el Mac de destino.
#   npm run release:mac
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/src-tauri/binaries"
HEIF="$ROOT/.deps/libheif-universal"
export MACOSX_DEPLOYMENT_TARGET="11.0"

depends_on_homebrew() { otool -L "$1" | tail -n +2 | grep -qE '/(usr/local|opt/homebrew)/'; }

# 1. libheif estática
[[ -f "$HEIF/lib/libheif.a" ]] || "$ROOT/scripts/build-libheif-static.sh"

# 2. ffmpeg/ffprobe autónomos para cada arquitectura
for pair in "arm64 aarch64-apple-darwin" "x86_64 x86_64-apple-darwin"; do
  set -- $pair
  if [[ ! -f "$BIN/ffmpeg-$2" ]] || depends_on_homebrew "$BIN/ffmpeg-$2" || depends_on_homebrew "$BIN/ffprobe-$2"; then
    echo "ffmpeg $1 falta o depende de Homebrew: se compila uno autónomo."
    "$ROOT/scripts/build-ffmpeg-macos.sh" "$1"
  fi
done

# 3. Sidecars universales (Tauri los busca con este sufijo)
for tool in ffmpeg ffprobe; do
  lipo -create "$BIN/$tool-aarch64-apple-darwin" "$BIN/$tool-x86_64-apple-darwin" \
    -output "$BIN/$tool-universal-apple-darwin"
done

# 4. Compilar enlazando libheif estática.
# pkg-config no admite espacios en rutas (la carpeta del proyecto los tiene):
# se accede a libheif por un enlace simbólico sin espacios.
HEIF_LINK="$HOME/Library/Caches/conversor-libheif"
ln -sfn "$HEIF" "$HEIF_LINK"
sed -i '' "s|^prefix=.*|prefix=$HEIF_LINK|" "$HEIF/lib/pkgconfig/libheif.pc"
HEIF="$HEIF_LINK"
export PKG_CONFIG_PATH="$HEIF/lib/pkgconfig"
export PKG_CONFIG_LIBDIR="$HEIF/lib/pkgconfig"
export PKG_CONFIG_ALLOW_CROSS=1
export SYSTEM_DEPS_LIBHEIF_LINK=static
# libde265 (x86_64) detecta SSE/AVX con __builtin_cpu_supports, que vive en el
# runtime de clang; Rust enlaza con -nodefaultlibs y no lo incluye solo.
export CARGO_TARGET_X86_64_APPLE_DARWIN_RUSTFLAGS="-C link-arg=$(xcrun clang --print-runtime-dir)/libclang_rt.osx.a"
cd "$ROOT"
npx tauri build --target universal-apple-darwin --features heic

# 5. Verificar
APP="$ROOT/src-tauri/target/universal-apple-darwin/release/bundle/macos/Conversor.app"
fail=0
for f in "$APP"/Contents/MacOS/*; do
  lipo "$f" -verify_arch x86_64 arm64 || { echo "No es universal: $f" >&2; fail=1; }
  if depends_on_homebrew "$f"; then echo "Depende de Homebrew: $f" >&2; otool -L "$f" >&2; fail=1; fi
done
strings "$APP/Contents/MacOS/conversor" | grep -q "no incluye soporte HEIC" && { echo "Compilado SIN HEIC" >&2; fail=1; }
[[ $fail == 0 ]] || exit 1

echo
echo "Instalador listo:"
ls -1 "$ROOT"/src-tauri/target/universal-apple-darwin/release/bundle/dmg/*.dmg
