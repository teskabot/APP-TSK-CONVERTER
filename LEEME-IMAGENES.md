# Motor de imágenes (`src-tauri/src/engine/raster.rs`)

## Qué hace con cada archivo
Detecta el formato por **contenido** (un PNG renombrado a `.jpg` sigue siendo PNG) y decide:

| Situación                                    | Resultado                                              |
|----------------------------------------------|--------------------------------------------------------|
| Mismo formato (PNG→PNG, TIFF→TIFF)           | Copia byte a byte                                      |
| JPEG→JPEG, WEBP→WEBP                         | Copia byte a byte. Con "Compacta" recomprime y se queda con el resultado solo si pesa menos |
| Cualquier otro                               | Decodifica → gira según EXIF → codifica → añade ICC/EXIF |

## Metadatos
| Destino | Perfil ICC | EXIF |
|---------|-----------|------|
| JPEG, PNG, WEBP | idéntico (bloque crudo) | idéntico (bloque crudo) |
| TIFF | idéntico | solo textos básicos: marca, modelo, software, fecha, autor, copyright, descripción (el aviso lo indica si se pierde exposición/GPS) |

La **orientación EXIF se aplica a los píxeles** (giros exactos, sin pérdida) y el EXIF queda en 1, para que
la imagen se vea igual en visores que ignoran EXIF. Un perfil ICC que no es RGB (p. ej. grises) no se incluye y se avisa.

## Profundidad y transparencia
- PNG y TIFF conservan **16 bits**. JPEG y WEBP guardan 8 y se avisa.
- JPEG no admite transparencia: se rellena de blanco y se avisa (solo si había píxeles realmente transparentes).
- WEBP "Máxima" es **sin pérdida** y conserva el RGB incluso bajo píxeles transparentes.

## Calidad (preset → ajuste)
| | Máxima | Balanceada | Compacta |
|--|--|--|--|
| JPEG (MozJPEG) | q98, color 4:4:4 | q85 | q72 |
| WEBP | sin pérdida | q85 | q70 |
| PNG / TIFF | sin pérdida (siempre) | idem | idem, compresión máxima en PNG |

## Entradas especiales
- **SVG**: se rasteriza con lado mayor entre 2048 y 8192 px (constantes `SVG_MIN_SIDE`/`SVG_MAX_SIDE`) y se avisa del tamaño.
- **RAW** (CR2, NEF, ARW, DNG, RAF, ORF, RW2): revelado básico con `imagepipe`. **No se transfieren los metadatos EXIF** (se avisa).
  Cuerpos de cámara no reconocidos dan "Formato no compatible".
- **HEIC**: solo entrada. 8 y 10 bits, con perfil ICC y EXIF. Para guardar, usa WEBP.

## HEIC: es opcional en la compilación
HEIC usa la biblioteca nativa **libheif ≥ 1.17**. Sin ella el proyecto compila igual y los HEIC fallan con un mensaje claro.
Para activarlo:

    # macOS
    brew install libheif pkg-config
    # Windows (vcpkg)
    vcpkg install libheif:x64-windows-static-md   # y exporta VCPKG_ROOT
    # Linux
    sudo apt install libheif-dev

    npm run dev:heic        # desarrollo
    npm run build:heic      # instalador

Para publicar, enlaza libheif **estática** (así el usuario no necesita instalar nada) y con libde265 solamente:
decodificar no requiere el codificador HEVC (x265), que es GPL y con patentes.

## Pruebas
    cd src-tauri
    cargo test --lib engine::raster                 # sin HEIC
    cargo test --lib engine::raster --features heic # con HEIC

Las pruebas de integración generan imágenes e inspeccionan los resultados con **Python + Pillow + numpy**
(independiente de este código); si no están instalados, se omiten. El de HEIC usa `heif-enc` si existe.

## Pendiente / decisiones abiertas
- **AVIF** no está. `ravif` no puede incluir perfiles ICC: una foto Display P3 cambiaría de color en silencio, lo que
  rompe la promesa central de la app. Hace falta convertir a sRGB con un CMM antes de ofrecerlo.
- RAW sin probar con archivos reales (no tenía muestras). WebP animado y TIFF multipágina no están soportados.
- `Cargo.toml`: quité `panic = "abort"` del perfil release. Una biblioteca de decodificación puede entrar en pánico con un
  archivo roto; con `abort` eso cerraría la app entera en vez de aislar el archivo (`catch_unwind` en `raster::convert`).
