# Motor de audio

## Primer arranque (desarrollo)
    npm install
    npm run sidecar     # copia tu ffmpeg/ffprobe a src-tauri/binaries/
    npm run tauri dev

## Probar el motor sin la app
    cd src-tauri && cargo test --lib engine::audio     # necesita ffmpeg en el PATH

## Qué hace cada caso
| Situación                                   | Acción        | Resultado                         |
|---------------------------------------------|---------------|-----------------------------------|
| Mismo formato y códec (ej. WEBM→WEBM)       | FileCopy      | Copia byte a byte                 |
| Distinto contenedor, mismo códec (MP4→M4A)  | StreamCopy    | Remux, audio idéntico             |
| Lossless → lossless (WAV↔FLAC)              | Encode        | PCM idéntico, mismos bits y Hz    |
| Cualquier otro                              | Encode        | Mejor calidad según el preset     |

"Compacta" sobre un archivo con pérdida solo recodifica si el resultado pesa menos.

## Límites del formato (se avisan con `warnings`)
- Opus (WEBM): solo 8/12/16/24/48 kHz. 44,1 kHz sube a 48 kHz; 96 kHz baja a 48 kHz.
- MP3: máx. 48 kHz y 2 canales.
- FLAC: máx. 24 bits (32 bits/float se ajustan a 24).
- Carátula: solo MP3, M4A y FLAC. Etiquetas: AAC suelto (ADTS) no admite.
- M4A: conserva carátula y etiquetas estándar, pero no las personalizadas.

## Para publicar: FFmpeg propio
No uses un ffmpeg de sistema: puede depender de bibliotecas que no estén instaladas
en los equipos de tus usuarios. El script `scripts/build-ffmpeg-arm64.sh` compila
sidecars ARM64 desde una Mac Intel usando el SDK de macOS y los instala en
`src-tauri/binaries/`. Necesita Xcode Command Line Tools, `pkg-config`, `curl`,
`tar` y `make`.

**Licencia:** la configuración abajo activa `libmp3lame`. LAME es GPL, por lo que
FFmpeg requiere `--enable-gpl` y el binario resultante es GPL, no LGPL-only.
Si distribuyes ese binario, revisa y cumple las obligaciones de GPL, incluida la
forma de ofrecer el código fuente correspondiente. Para mantener FFmpeg bajo
LGPL habría que quitar LAME y `libmp3lame` (y cambiar la funcionalidad de salida
MP3 de la aplicación).

El script se ejecuta desde cualquier directorio:

    bash scripts/build-ffmpeg-arm64.sh

Descarga las fuentes oficiales de Ogg, Opus, LAME, Vorbis y FFmpeg; compila las
bibliotecas estáticas para Apple Silicon y comprueba que los ejecutables
resultantes sean ARM64 y no dependan de rutas de Homebrew.

El script activa `--enable-gpl` porque incluye LAME. Los selectores de codecs
usados son:

    --enable-protocol=file
    --enable-demuxer=wav,flac,mp3,ogg,matroska,mov,aac
    --enable-muxer=wav,flac,mp3,ogg,webm,ipod,adts
    --enable-decoder=pcm_*,flac,mp3*,aac*,alac,opus,vorbis,libopus,libvorbis,mjpeg,png
    --enable-encoder=pcm_*,flac,aac,libopus,libmp3lame,libvorbis,mjpeg,png
    --enable-parser=aac,flac,mpegaudio,opus,vorbis,mjpeg,png
    --enable-libopus --enable-libmp3lame --enable-libvorbis
    --enable-filter=aresample,aformat,anull

Revisa la lista contra `cargo test`: si falta algo, FFmpeg lo dirá como
"Unknown encoder/decoder".
