# Avisos de terceros

Conversor usa software de terceros. Este archivo lista los componentes que se
distribuyen dentro de la aplicación, sus licencias y dónde obtener su código
fuente. Los textos completos de las licencias GNU están en la carpeta
[`licencias/`](licencias/).

El código fuente de Conversor y los scripts usados para compilar estos
componentes están en https://github.com/teskabot/APP-TSK-CONVERTER

---

## FFmpeg (programas `ffmpeg` y `ffprobe`)

Conversor incluye los programas `ffmpeg` y `ffprobe` como ejecutables
separados para convertir audio. La aplicación los ejecuta como procesos
independientes; no están enlazados con el código de Conversor.

- **Versión:** FFmpeg 7.1.1
- **Licencia:** GNU General Public License, versión 2 o posterior
  ([`licencias/GPL-2.0.txt`](licencias/GPL-2.0.txt)). La compilación usa
  `--enable-gpl`, por eso el binario resultante se distribuye bajo GPL.
- **Copyright:** © 2000-2025 the FFmpeg developers — https://ffmpeg.org
- **Código fuente:** https://ffmpeg.org/releases/ffmpeg-7.1.1.tar.xz
- **Cómo se compiló:** `scripts/build-ffmpeg-macos.sh` en este repositorio.
  La configuración exacta se puede ver con `ffmpeg -version`.

Bibliotecas incluidas dentro de esos ejecutables:

| Biblioteca | Versión | Licencia | Código fuente |
|------------|---------|----------|---------------|
| LAME (libmp3lame) | 3.100 | LGPL 2 o posterior | https://downloads.sourceforge.net/project/lame/lame/3.100/lame-3.100.tar.gz |
| Opus (libopus) | 1.5.2 | BSD de 3 cláusulas | https://downloads.xiph.org/releases/opus/opus-1.5.2.tar.gz |
| Vorbis (libvorbis) | 1.3.7 | BSD de 3 cláusulas | https://downloads.xiph.org/releases/vorbis/libvorbis-1.3.7.tar.xz |
| Ogg (libogg) | 1.3.5 | BSD de 3 cláusulas | https://downloads.xiph.org/releases/ogg/libogg-1.3.5.tar.xz |

Copyright de Opus, Vorbis y Ogg: © Xiph.Org Foundation y colaboradores.
Copyright de LAME: © los desarrolladores de LAME — https://lame.sourceforge.io

---

## Bibliotecas LGPL enlazadas en la aplicación

Estas bibliotecas están enlazadas dentro del ejecutable de Conversor. Como el
código fuente completo de Conversor es público (ver arriba), podés modificar
cualquiera de ellas y volver a compilar la aplicación con tu versión.

| Biblioteca | Versión | Uso | Licencia | Código fuente |
|------------|---------|-----|----------|---------------|
| libheif | 1.23.7 | Lectura de HEIC/HEIF (macOS) | [LGPL 3](licencias/LGPL-3.0.txt) | https://github.com/strukturag/libheif |
| libde265 | 1.1.3 | Decodificación HEVC para HEIC (macOS) | [LGPL 3](licencias/LGPL-3.0.txt) | https://github.com/strukturag/libde265 |
| rawloader | 0.37.2 | Lectura de archivos RAW | [LGPL 2.1](licencias/LGPL-2.1.txt) | https://crates.io/crates/rawloader |
| imagepipe | 0.5.1 | Revelado de archivos RAW | [LGPL 3](licencias/LGPL-3.0.txt) | https://crates.io/crates/imagepipe |
| multicache | 0.6.1 | Dependencia de imagepipe | [LGPL 3](licencias/LGPL-3.0.txt) | https://crates.io/crates/multicache |

libheif y libde265: © struktur AG, Dirk Farin y colaboradores.
rawloader, imagepipe y multicache: © Pedro Côrte-Real.

La LGPL 3 se apoya en la GPL 3, cuyo texto está en
[`licencias/GPL-3.0.txt`](licencias/GPL-3.0.txt).

---

## Bibliotecas MPL 2.0

Usadas para leer SVG. Su código fuente está disponible sin cambios en
crates.io. Licencia: Mozilla Public License 2.0 —
https://www.mozilla.org/MPL/2.0/

- cssparser 0.37.0, cssparser-macros 0.7.1, selectors 0.38.0 — © The Servo Project Developers
- dtoa-short 0.3.5
- option-ext 0.2.0

---

## MozJPEG / libjpeg

Este software se basa en parte en el trabajo del Independent JPEG Group.
MozJPEG (crates `mozjpeg` 0.10 y `mozjpeg-sys` 2.2) se distribuye bajo las
licencias IJG, BSD de 3 cláusulas y zlib —
https://github.com/mozilla/mozjpeg/blob/main/LICENSE.md

---

## Otras bibliotecas

El resto de las dependencias usan licencias permisivas (MIT, Apache 2.0,
BSD, ISC, zlib, Unicode o CC0). Entre las principales:

| Componente | Licencia |
|------------|----------|
| Tauri y sus plugins (dialog, opener) | MIT o Apache 2.0 |
| image, tiff, png, resvg, walkdir, rayon, serde | MIT o Apache 2.0 |
| libwebp (crate `webp`) | BSD de 3 cláusulas (libwebp) / MIT o Apache 2.0 (crate) |
| React, React DOM, Zustand | MIT |
| Lucide (íconos) | ISC |
| Fuente Geist | SIL Open Font License 1.1 |

La lista completa de dependencias de Rust y sus versiones está en
`src-tauri/Cargo.lock`, y la de JavaScript en `package-lock.json`. Cada
paquete incluye su propio aviso de licencia en crates.io o npm.
