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

## Para publicar: FFmpeg propio, LGPL y mínimo
No uses un ffmpeg de sistema ni uno "GPL" (arrastra la licencia a tu app).
Compílalo solo con audio y sin GPL; además pesa pocos MB:

    ./configure --disable-gpl --disable-nonfree --disable-everything \
      --enable-protocol=file --enable-ffprobe \
      --enable-demuxer=wav,flac,mp3,ogg,matroska,mov,aac \
      --enable-muxer=wav,flac,mp3,ogg,webm,ipod,adts \
      --enable-decoder=pcm_*,flac,mp3*,aac*,alac,opus,vorbis,libopus,mjpeg,png \
      --enable-encoder=pcm_*,flac,aac,libopus,libmp3lame,libvorbis,mjpeg,png \
      --enable-parser=aac,flac,mpegaudio,opus,vorbis,mjpeg,png \
      --enable-libopus --enable-libmp3lame --enable-libvorbis \
      --enable-filter=aresample,aformat,anull

(Revisa la lista contra `cargo test`: si falta algo, FFmpeg lo dirá como "Unknown encoder/decoder".)
