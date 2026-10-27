#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARCH="${1:-arm64}"
case "$ARCH" in
  arm64) HOST="aarch64-apple-darwin"; FF_ARCH="aarch64"; SUFFIX="aarch64-apple-darwin" ;;
  x86_64) HOST="x86_64-apple-darwin"; FF_ARCH="x86_64"; SUFFIX="x86_64-apple-darwin" ;;
  *) echo "Uso: $0 [arm64|x86_64]" >&2; exit 1 ;;
esac
BUILD_ROOT="/tmp/conversor-ffmpeg-$ARCH-build"
mkdir -p "$BUILD_ROOT"
WORK_DIR="$(mktemp -d "$BUILD_ROOT/work.XXXXXX")"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Este script debe ejecutarse en macOS." >&2
  exit 1
fi

for tool in xcrun curl tar make pkg-config; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Falta '$tool'. Instala Xcode Command Line Tools y pkg-config." >&2
    exit 1
  fi
done

SDK="$(xcrun --sdk macosx --show-sdk-path)"
CC="$(xcrun --sdk macosx --find clang)"
AR="$(xcrun --sdk macosx --find ar)"
RANLIB="$(xcrun --sdk macosx --find ranlib)"
MIN_MACOS="11.0"
PREFIX="$WORK_DIR/$ARCH"
JOBS="$(sysctl -n hw.ncpu)"

mkdir -p "$PREFIX" "$WORK_DIR/src" "$WORK_DIR/downloads"
export CC AR RANLIB
export CFLAGS="-arch $ARCH -isysroot $SDK -mmacosx-version-min=$MIN_MACOS"
export CPPFLAGS="-arch $ARCH -isysroot $SDK -mmacosx-version-min=$MIN_MACOS -I$PREFIX/include"
export LDFLAGS="-arch $ARCH -isysroot $SDK -mmacosx-version-min=$MIN_MACOS -L$PREFIX/lib"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig"
export PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig"
export MACOSX_DEPLOYMENT_TARGET="$MIN_MACOS"

download_and_unpack() {
  local url="$1"
  local archive="$2"
  local directory="$3"
  curl --fail --location --retry 3 "$url" --output "$WORK_DIR/downloads/$archive"
  tar -xf "$WORK_DIR/downloads/$archive" -C "$WORK_DIR/src"
  printf '%s\n' "$WORK_DIR/src/$directory"
}

build_autotools() {
  local directory="$1"
  shift
  (
    cd "$directory"
    ./configure --build=x86_64-apple-darwin --host="$HOST" \
      --prefix="$PREFIX" --disable-shared --enable-static "$@"
    make -j"$JOBS"
    make install
  )
}

echo "Building $ARCH Ogg..."
OGG="$(download_and_unpack \
  "https://downloads.xiph.org/releases/ogg/libogg-1.3.5.tar.xz" \
  "libogg-1.3.5.tar.xz" "libogg-1.3.5")"
build_autotools "$OGG"

echo "Building $ARCH Opus..."
OPUS="$(download_and_unpack \
  "https://downloads.xiph.org/releases/opus/opus-1.5.2.tar.gz" \
  "opus-1.5.2.tar.gz" "opus-1.5.2")"
build_autotools "$OPUS" --disable-extra-programs --disable-doc

echo "Building $ARCH LAME..."
LAME="$(download_and_unpack \
  "https://downloads.sourceforge.net/project/lame/lame/3.100/lame-3.100.tar.gz" \
  "lame-3.100.tar.gz" "lame-3.100")"
build_autotools "$LAME" --disable-frontend

echo "Building $ARCH Vorbis..."
VORBIS="$(download_and_unpack \
  "https://downloads.xiph.org/releases/vorbis/libvorbis-1.3.7.tar.xz" \
  "libvorbis-1.3.7.tar.xz" "libvorbis-1.3.7")"
(
  cd "$VORBIS"
  ./configure --build=x86_64-apple-darwin --host="$HOST" \
    --prefix="$PREFIX" --disable-shared --enable-static \
    --with-ogg="$PREFIX" \
    --with-ogg-includes="$PREFIX/include" \
    --with-ogg-libraries="$PREFIX/lib" \
    --disable-oggtest
  make -C lib -j"$JOBS" libvorbis.la libvorbisenc.la
  make -C include install
  make -C lib install-libLTLIBRARIES
  make install-pkgconfigDATA
)

cat > "$PREFIX/lib/pkgconfig/lame.pc" <<EOF
prefix=$PREFIX
exec_prefix=\${prefix}
libdir=\${exec_prefix}/lib
includedir=\${prefix}/include

Name: LAME
Description: LAME MP3 encoder library
Version: 3.100
Libs: -L\${libdir} -lmp3lame
Cflags: -I\${includedir}
EOF

echo "Building $ARCH FFmpeg..."
FFMPEG="$(download_and_unpack \
  "https://ffmpeg.org/releases/ffmpeg-7.1.1.tar.xz" \
  "ffmpeg-7.1.1.tar.xz" "ffmpeg-7.1.1")"
(
  cd "$FFMPEG"
  ./configure \
    --prefix="$PREFIX" \
    --target-os=darwin \
    --arch=$FF_ARCH \
    --enable-cross-compile \
    --sysroot="$SDK" \
    --cc="$CC" \
    --ar="$AR" \
    --ranlib="$RANLIB" \
    --pkg-config=pkg-config \
    --pkg-config-flags=--static \
    --extra-cflags="$CFLAGS -I$PREFIX/include" \
    --extra-ldflags="$LDFLAGS" \
    --enable-gpl \
    --disable-nonfree \
    --disable-everything \
    --disable-autodetect \
    --disable-shared \
    --enable-static \
    --enable-ffmpeg \
    --enable-ffprobe \
    --enable-protocol=file \
    --enable-demuxer=wav,flac,mp3,ogg,matroska,mov,aac \
    --enable-muxer=wav,flac,mp3,ogg,webm,ipod,adts \
    --enable-decoder=pcm_*,flac,mp3*,aac*,alac,opus,vorbis,libopus,libvorbis,mjpeg,png \
    --enable-encoder=pcm_*,flac,aac,libopus,libmp3lame,libvorbis,mjpeg,png \
    --enable-parser=aac,flac,mpegaudio,opus,vorbis,mjpeg,png \
    --enable-libopus \
    --enable-libmp3lame \
    --enable-libvorbis \
    --enable-filter=aresample,aformat,anull
  make -j"$JOBS"
)

OUT="$ROOT/src-tauri/binaries"
mkdir -p "$OUT"
install -m 755 "$FFMPEG/ffmpeg" "$OUT/ffmpeg-$SUFFIX"
install -m 755 "$FFMPEG/ffprobe" "$OUT/ffprobe-$SUFFIX"

for binary in "$OUT/ffmpeg-$SUFFIX" "$OUT/ffprobe-$SUFFIX"; do
  if ! lipo "$binary" -verify_arch "$ARCH"; then
    echo "El binario no es $ARCH: $binary" >&2
    exit 1
  fi
  if otool -L "$binary" | grep -E '/(usr/local|opt/homebrew)/'; then
    echo "El binario depende de una ruta local de Homebrew: $binary" >&2
    exit 1
  fi
done

echo
echo "Sidecars $ARCH instalados:"
file "$OUT/ffmpeg-$SUFFIX"
file "$OUT/ffprobe-$SUFFIX"
echo
echo "Aviso: libmp3lame es GPL; FFmpeg se compiló con --enable-gpl."
echo "El directorio de trabajo temporal se conserva en: $WORK_DIR"
