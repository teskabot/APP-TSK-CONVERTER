#!/usr/bin/env bash
# Compila libheif ESTÁTICA (solo decodificación HEVC con libde265) para macOS
# universal (arm64 + x86_64). Así la app no depende de Homebrew en otros Macs
# y no incluye x265 (GPL, con patentes) ni aom.
#
# Resultado: .deps/libheif-universal/{lib,include}  (lo usa scripts/release-macos.sh)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEPS="$ROOT/.deps"
OUT="$DEPS/libheif-universal"
MIN_MACOS="${MIN_MACOS:-11.0}"
DE265_VER="1.1.3"
HEIF_VER="1.23.7"
JOBS="$(sysctl -n hw.ncpu)"

for tool in cmake curl tar lipo xcrun; do
  command -v "$tool" >/dev/null || { echo "Falta '$tool'." >&2; exit 1; }
done

mkdir -p "$DEPS/src" "$DEPS/downloads"
fetch() {  # url archivo carpeta
  [[ -f "$DEPS/downloads/$2" ]] || curl --fail --location --retry 3 "$1" -o "$DEPS/downloads/$2"
  rm -rf "$DEPS/src/$3"
  tar -xf "$DEPS/downloads/$2" -C "$DEPS/src"
}
fetch "https://github.com/strukturag/libde265/releases/download/v$DE265_VER/libde265-$DE265_VER.tar.gz" \
  "libde265-$DE265_VER.tar.gz" "libde265-$DE265_VER"
fetch "https://github.com/strukturag/libheif/releases/download/v$HEIF_VER/libheif-$HEIF_VER.tar.gz" \
  "libheif-$HEIF_VER.tar.gz" "libheif-$HEIF_VER"

COMMON=(
  -G Ninja -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF
  -DCMAKE_OSX_DEPLOYMENT_TARGET="$MIN_MACOS"
  -DCMAKE_INSTALL_LIBDIR=lib
  # Nunca tomar bibliotecas de Homebrew: deben quedar dentro de la app.
  -DCMAKE_IGNORE_PREFIX_PATH="/usr/local;/opt/homebrew"
  -DCMAKE_FIND_FRAMEWORK=NEVER
)

build_arch() {
  local arch="$1"
  local prefix="$DEPS/libheif-$arch"
  local sse=ON; [[ "$arch" == arm64 ]] && sse=OFF
  rm -rf "$prefix" "$DEPS/build-$arch"
  echo "== libde265 $arch"
  cmake -S "$DEPS/src/libde265-$DE265_VER" -B "$DEPS/build-$arch/de265" "${COMMON[@]}" \
    -DCMAKE_OSX_ARCHITECTURES="$arch" -DCMAKE_INSTALL_PREFIX="$prefix" \
    -DENABLE_SDL=OFF -DENABLE_DECODER=OFF -DENABLE_ENCODER=OFF -DENABLE_SSE="$sse"
  cmake --build "$DEPS/build-$arch/de265" -j"$JOBS"
  cmake --install "$DEPS/build-$arch/de265"

  echo "== libheif $arch"
  local off=()
  for k in AOM_DECODER AOM_ENCODER DAV1D RAV1E SvtEnc X264 X265 KVAZAAR JPEG_DECODER JPEG_ENCODER \
           OpenJPEG_DECODER OpenJPEG_ENCODER OPENJPH_ENCODER FFMPEG_DECODER OpenH264_DECODER \
           UVG266 VVDEC VVENC; do
    off+=("-DWITH_$k=OFF" "-DWITH_${k}_PLUGIN=OFF")
  done
  cmake -S "$DEPS/src/libheif-$HEIF_VER" -B "$DEPS/build-$arch/heif" "${COMMON[@]}" \
    -DCMAKE_OSX_ARCHITECTURES="$arch" -DCMAKE_INSTALL_PREFIX="$prefix" \
    -DCMAKE_PREFIX_PATH="$prefix" \
    "${off[@]}" -DWITH_LIBDE265=ON -DWITH_LIBDE265_PLUGIN=OFF \
    -DENABLE_PLUGIN_LOADING=OFF -DWITH_LIBSHARPYUV=OFF -DWITH_UNCOMPRESSED_CODEC=OFF \
    -DWITH_HEADER_COMPRESSION=OFF -DWITH_GDK_PIXBUF=OFF -DWITH_EXAMPLES=OFF \
    -DBUILD_TESTING=OFF -DBUILD_DOCUMENTATION=OFF -DWITH_REDUCED_VISIBILITY=ON
  cmake --build "$DEPS/build-$arch/heif" -j"$JOBS"
  cmake --install "$DEPS/build-$arch/heif"
}

build_arch arm64
build_arch x86_64

echo "== Uniendo en biblioteca universal"
rm -rf "$OUT"; mkdir -p "$OUT/lib/pkgconfig"
cp -R "$DEPS/libheif-arm64/include" "$OUT/include"
for lib in libheif.a libde265.a; do
  lipo -create "$DEPS/libheif-arm64/lib/$lib" "$DEPS/libheif-x86_64/lib/$lib" -output "$OUT/lib/$lib"
done
cat > "$OUT/lib/pkgconfig/libheif.pc" <<EOF
prefix=$OUT
libdir=\${prefix}/lib
includedir=\${prefix}/include

Name: libheif
Description: HEIF decoder (static, libde265 only)
Version: $HEIF_VER
Libs: -L\${libdir} -lheif
Libs.private: -lde265 -lc++
Cflags: -I\${includedir}
EOF

lipo -info "$OUT/lib/libheif.a" "$OUT/lib/libde265.a"
echo "Listo: $OUT"
