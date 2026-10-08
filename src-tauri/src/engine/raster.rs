//! Motor de imágenes.
//!
//! Flujo: detectar (por contenido) → plan → decodificar → (orientación) → codificar → metadatos.
//!
//! Reglas de fidelidad:
//!  1. Mismo formato de origen y destino → copia byte a byte (PNG y TIFF siempre; JPEG y
//!     WEBP salvo "Compacta", que recomprime y se queda con el resultado solo si pesa menos).
//!  2. El perfil ICC y el EXIF viajan sin reinterpretarse (blobs crudos) a JPEG, PNG y WebP.
//!     TIFF conserva el ICC y los datos básicos de cámara (ver `encode_tiff`).
//!  3. La orientación EXIF se aplica a los píxeles (giro sin pérdida) y el EXIF queda en 1:
//!     así la imagen se ve igual en cualquier visor, incluso en los que ignoran EXIF.
//!  4. Profundidad: PNG y TIFF conservan 16 bits; JPEG y WebP guardan 8 y se avisa.
//!  5. HEIC es solo de entrada; la salida se ofrece en WEBP (ver LEEME-IMAGENES.md).
//!  6. La salida se escribe en un `.partial` y se renombra: nunca queda un archivo a medias.

use super::audio::partial_path;
use super::{ConvertError, Mode, Outcome};
use crate::models::{JobSpec, QualityPreset};
use img_parts::{Bytes, ImageEXIF, ImageICC};
use std::borrow::Cow;
use std::io::Read;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

pub const IMAGE_TARGETS: &[&str] = &["png", "webp", "jpeg", "tiff"];

const SVG_MIN_SIDE: f32 = 2048.0; // un SVG se rasteriza con al menos este lado mayor...
const SVG_MAX_SIDE: f32 = 8192.0; // ...y como mucho este
const WEBP_MAX_SIDE: u32 = 16383;
const JPEG_MAX_SIDE: u32 = 65535;
const JPEG_EXIF_MAX: usize = 65_000; // un segmento APP1 no pasa de ~64 KB
const RAW_EXT: &[&str] = &["cr2", "nef", "arw", "dng", "raf", "orf", "rw2"];

// ───────────────────────── Detección ─────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Src {
    Png,
    Jpeg,
    Webp,
    Tiff,
    Heic,
    Svg,
    Raw,
}

impl Src {
    /// Origen con pérdida: convertirlo a un formato sin pérdida no recupera calidad.
    fn is_lossy(self) -> bool {
        matches!(self, Src::Jpeg | Src::Heic)
    }
    fn label(self) -> &'static str {
        match self {
            Src::Png => "PNG",
            Src::Jpeg => "JPEG",
            Src::Webp => "WEBP",
            Src::Tiff => "TIFF",
            Src::Heic => "HEIC",
            Src::Svg => "SVG",
            Src::Raw => "RAW",
        }
    }
}

fn is_heif_brand(b: &[u8]) -> bool {
    matches!(
        b,
        b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" | b"hevm" | b"hevs" | b"mif1" | b"msf1"
    )
}

/// Se detecta por los primeros bytes: un PNG renombrado a `.jpg` sigue siendo un PNG.
/// RAW y SVG se deciden por extensión (los RAW son TIFF por dentro; el SVG es texto).
pub fn detect(path: &Path, ext: &str) -> Option<Src> {
    let ext = ext.to_ascii_lowercase();
    if RAW_EXT.contains(&ext.as_str()) {
        return Some(Src::Raw);
    }
    let mut head = [0u8; 16];
    let n = std::fs::File::open(path).and_then(|mut f| f.read(&mut head)).ok()?;
    let h = &head[..n];
    if h.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Src::Png)
    } else if h.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Src::Jpeg)
    } else if n >= 12 && &h[0..4] == b"RIFF" && &h[8..12] == b"WEBP" {
        Some(Src::Webp)
    } else if h.starts_with(b"II*\0") || h.starts_with(b"MM\0*") {
        Some(Src::Tiff)
    } else if n >= 12 && &h[4..8] == b"ftyp" && is_heif_brand(&h[8..12]) {
        Some(Src::Heic)
    } else if ext == "svg" {
        Some(Src::Svg)
    } else {
        None
    }
}

// ───────────────────────── Plan ─────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Mismo formato: copia del archivo tal cual (bit-exact, con todos sus metadatos).
    FileCopy,
    /// Decodificar y volver a codificar. `keep_smaller`: si no reduce el tamaño, se copia el original.
    Transcode { keep_smaller: bool },
}

pub fn plan(src: Src, target: &str, preset: QualityPreset) -> Action {
    let same = matches!(
        (src, target),
        (Src::Png, "png") | (Src::Jpeg, "jpeg") | (Src::Webp, "webp") | (Src::Tiff, "tiff")
    );
    if !same {
        return Action::Transcode { keep_smaller: false };
    }
    let lossy_format = matches!(target, "jpeg" | "webp");
    if lossy_format && preset == QualityPreset::Compact {
        Action::Transcode { keep_smaller: true }
    } else {
        Action::FileCopy
    }
}

// ───────────────────────── Píxeles ─────────────────────────

pub enum Pix {
    Rgb8(Vec<u8>),
    Rgba8(Vec<u8>),
    Rgb16(Vec<u16>),
    Rgba16(Vec<u16>),
}

pub struct Raster {
    pub w: u32,
    pub h: u32,
    pub pix: Pix,
}

fn u16_to_u8(v: u16) -> u8 {
    ((v as u32 * 255 + 32767) / 65535) as u8
}

/// Reubica los píxeles según la orientación EXIF (1..=8). Son giros/espejos exactos.
fn orient<T: Copy>(src: &[T], w: usize, h: usize, ch: usize, o: u16) -> (Vec<T>, usize, usize) {
    let (nw, nh) = if o >= 5 { (h, w) } else { (w, h) };
    let mut out = Vec::with_capacity(src.len());
    for y in 0..nh {
        for x in 0..nw {
            let (sx, sy) = match o {
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (y, h - 1 - x),
                7 => (w - 1 - y, h - 1 - x),
                8 => (w - 1 - y, x),
                _ => (x, y),
            };
            let i = (sy * w + sx) * ch;
            out.extend_from_slice(&src[i..i + ch]);
        }
    }
    (out, nw, nh)
}

impl Raster {
    fn empty() -> Raster {
        Raster { w: 0, h: 0, pix: Pix::Rgb8(Vec::new()) }
    }
    fn is_16(&self) -> bool {
        matches!(self.pix, Pix::Rgb16(_) | Pix::Rgba16(_))
    }
    /// ¿Hay píxeles realmente transparentes? (un canal alfa 100 % opaco no cuenta)
    fn has_real_transparency(&self) -> bool {
        match &self.pix {
            Pix::Rgba8(v) => v.chunks_exact(4).any(|p| p[3] != 255),
            Pix::Rgba16(v) => v.chunks_exact(4).any(|p| p[3] != 65535),
            _ => false,
        }
    }
    fn to_8(self) -> Raster {
        let pix = match self.pix {
            Pix::Rgb16(v) => Pix::Rgb8(v.into_iter().map(u16_to_u8).collect()),
            Pix::Rgba16(v) => Pix::Rgba8(v.into_iter().map(u16_to_u8).collect()),
            p => p,
        };
        Raster { pix, ..self }
    }
    /// Compone sobre fondo blanco y devuelve RGB de 8 bits.
    fn flatten_white(self) -> Raster {
        let r = self.to_8();
        let pix = match r.pix {
            Pix::Rgba8(v) => {
                let mut out = Vec::with_capacity(v.len() / 4 * 3);
                for p in v.chunks_exact(4) {
                    let a = p[3] as u32;
                    for c in &p[..3] {
                        out.push(((*c as u32 * a + 255 * (255 - a) + 127) / 255) as u8);
                    }
                }
                Pix::Rgb8(out)
            }
            p => p,
        };
        Raster { pix, ..r }
    }
    fn apply_orientation(self, o: u16) -> Raster {
        if !(2..=8).contains(&o) {
            return self;
        }
        let (w, h) = (self.w as usize, self.h as usize);
        macro_rules! go {
            ($v:expr, $ch:expr, $variant:ident) => {{
                let (d, nw, nh) = orient(&$v, w, h, $ch, o);
                Raster { w: nw as u32, h: nh as u32, pix: Pix::$variant(d) }
            }};
        }
        match &self.pix {
            Pix::Rgb8(v) => go!(v, 3, Rgb8),
            Pix::Rgba8(v) => go!(v, 4, Rgba8),
            Pix::Rgb16(v) => go!(v, 3, Rgb16),
            Pix::Rgba16(v) => go!(v, 4, Rgba16),
        }
    }
}

// ───────────────────────── Metadatos (ICC y EXIF) ─────────────────────────

/// `exif` es siempre el bloque TIFF crudo (empieza por "II*\0" o "MM\0*"), sin el prefijo "Exif\0\0".
#[derive(Default, Clone)]
pub struct Meta {
    pub icc: Option<Vec<u8>>,
    pub exif: Option<Vec<u8>>,
}

fn strip_exif_prefix(b: &[u8]) -> &[u8] {
    b.strip_prefix(b"Exif\0\0").unwrap_or(b)
}

fn valid_icc(b: &[u8]) -> bool {
    b.len() >= 128 && &b[36..40] == b"acsp"
}

fn icc_is_rgb(b: &[u8]) -> bool {
    &b[16..20] == b"RGB "
}

fn rd16(b: &[u8], le: bool, o: usize) -> Option<u16> {
    let s: [u8; 2] = b.get(o..o + 2)?.try_into().ok()?;
    Some(if le { u16::from_le_bytes(s) } else { u16::from_be_bytes(s) })
}

fn rd32(b: &[u8], le: bool, o: usize) -> Option<u32> {
    let s: [u8; 4] = b.get(o..o + 4)?.try_into().ok()?;
    Some(if le { u32::from_le_bytes(s) } else { u32::from_be_bytes(s) })
}

fn type_size(t: u16) -> usize {
    match t {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 => 4,
        5 | 10 | 12 => 8,
        _ => 0,
    }
}

struct IfdEntry {
    tag: u16,
    typ: u16,
    /// posición del campo de valor de 4 bytes dentro del bloque
    value_pos: usize,
    data: Vec<u8>,
}

fn parse_ifd0(blob: &[u8]) -> Option<(bool, Vec<IfdEntry>)> {
    let le = match blob.get(0..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    if rd16(blob, le, 2)? != 42 {
        return None;
    }
    let off = rd32(blob, le, 4)? as usize;
    let n = rd16(blob, le, off)? as usize;
    let mut v = Vec::new();
    for i in 0..n {
        let e = off + 2 + i * 12;
        let tag = rd16(blob, le, e)?;
        let typ = rd16(blob, le, e + 2)?;
        let count = rd32(blob, le, e + 4)? as usize;
        let sz = type_size(typ).checked_mul(count)?;
        if type_size(typ) == 0 {
            continue;
        }
        let data = if sz <= 4 {
            blob.get(e + 8..e + 8 + sz)?.to_vec()
        } else {
            let p = rd32(blob, le, e + 8)? as usize;
            blob.get(p..p.checked_add(sz)?)?.to_vec()
        };
        v.push(IfdEntry { tag, typ, value_pos: e + 8, data });
    }
    Some((le, v))
}

fn exif_orientation(blob: &[u8]) -> u16 {
    let Some((le, entries)) = parse_ifd0(blob) else { return 1 };
    entries
        .iter()
        .find(|e| e.tag == 0x0112 && e.typ == 3 && e.data.len() >= 2)
        .and_then(|e| rd16(&e.data, le, 0))
        .unwrap_or(1)
}

/// Deja la orientación EXIF en 1 (los píxeles ya están girados).
fn reset_orientation(blob: &mut [u8]) {
    let Some((le, entries)) = parse_ifd0(blob) else { return };
    if let Some(e) = entries.iter().find(|e| e.tag == 0x0112 && e.typ == 3) {
        let bytes = if le { 1u16.to_le_bytes() } else { 1u16.to_be_bytes() };
        blob[e.value_pos..e.value_pos + 2].copy_from_slice(&bytes);
    }
}

/// Etiquetas de texto básicas de IFD0 (las únicas que TIFF puede llevar como etiquetas propias).
const BASIC_ASCII: &[u16] = &[0x010E, 0x010F, 0x0110, 0x0131, 0x0132, 0x013B, 0x8298];

fn basic_fields(blob: &[u8]) -> Vec<(u16, String)> {
    let Some((_, entries)) = parse_ifd0(blob) else { return Vec::new() };
    entries
        .into_iter()
        .filter(|e| e.typ == 2 && BASIC_ASCII.contains(&e.tag))
        .filter_map(|e| {
            let s = String::from_utf8_lossy(&e.data);
            let s = s.trim_end_matches('\0').to_string();
            (!s.is_empty()).then_some((e.tag, s))
        })
        .collect()
}

/// ¿El EXIF trae datos de exposición/GPS que TIFF no puede transportar?
fn has_detailed_exif(blob: &[u8]) -> bool {
    parse_ifd0(blob).map_or(false, |(_, es)| es.iter().any(|e| e.tag == 0x8769 || e.tag == 0x8825))
}

/// Construye un bloque EXIF mínimo (little endian) con textos básicos y orientación.
fn build_exif(ascii: &[(u16, String)], orientation: Option<u16>) -> Vec<u8> {
    let mut items: Vec<(u16, u16, Vec<u8>)> = ascii
        .iter()
        .map(|(t, s)| {
            let mut d = s.as_bytes().to_vec();
            d.push(0);
            (*t, 2u16, d)
        })
        .collect();
    if let Some(o) = orientation {
        items.push((0x0112, 3, o.to_le_bytes().to_vec()));
    }
    items.sort_by_key(|i| i.0);
    let mut out = vec![b'I', b'I', 42, 0, 8, 0, 0, 0];
    out.extend_from_slice(&(items.len() as u16).to_le_bytes());
    let mut extra: Vec<u8> = Vec::new();
    let extra_base = 8 + 2 + items.len() * 12 + 4;
    for (tag, typ, data) in &items {
        let count = if *typ == 2 { data.len() } else { data.len() / 2 } as u32;
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&typ.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        if data.len() <= 4 {
            let mut v = data.clone();
            v.resize(4, 0);
            out.extend_from_slice(&v);
        } else {
            out.extend_from_slice(&((extra_base + extra.len()) as u32).to_le_bytes());
            extra.extend_from_slice(data);
            if extra.len() % 2 == 1 {
                extra.push(0);
            }
        }
    }
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&extra);
    out
}

fn read_meta<I: ImageICC + ImageEXIF>(img: &I) -> Meta {
    Meta {
        icc: img.icc_profile().map(|b| b.to_vec()),
        exif: img.exif().map(|b| strip_exif_prefix(&b).to_vec()),
    }
}

fn meta_err<E: std::fmt::Display>(e: E) -> ConvertError {
    ConvertError::Failed(format!("metadatos: {e}"))
}

fn container_meta(src: Src, data: &[u8]) -> Meta {
    let bytes = Bytes::copy_from_slice(data);
    match src {
        Src::Jpeg => img_parts::jpeg::Jpeg::from_bytes(bytes).map(|i| read_meta(&i)).unwrap_or_default(),
        Src::Png => img_parts::png::Png::from_bytes(bytes).map(|i| read_meta(&i)).unwrap_or_default(),
        Src::Webp => img_parts::webp::WebP::from_bytes(bytes)
            .map(|w| {
                // El estándar guarda el EXIF de WebP sin prefijo; img-parts solo lo ve con prefijo.
                let exif = w
                    .chunk_by_id(img_parts::webp::CHUNK_EXIF)
                    .and_then(|c| c.content().data().cloned())
                    .map(|b| strip_exif_prefix(&b).to_vec());
                Meta { icc: w.icc_profile().map(|b| b.to_vec()), exif }
            })
            .unwrap_or_default(),
        _ => Meta::default(),
    }
}

fn tiff_meta(path: &Path) -> Meta {
    use tiff::tags::Tag;
    let Ok(f) = std::fs::File::open(path) else { return Meta::default() };
    let Ok(mut d) = tiff::decoder::Decoder::new(std::io::BufReader::new(f)) else { return Meta::default() };
    let icc = d.get_tag_u8_vec(Tag::Unknown(34675)).ok().filter(|v| !v.is_empty());
    let mut ascii = Vec::new();
    for (code, tag) in tiff_ascii_tags() {
        if let Ok(s) = d.get_tag_ascii_string(tag) {
            if !s.is_empty() {
                ascii.push((code, s));
            }
        }
    }
    let orientation = d.get_tag_u32(Tag::Orientation).ok().map(|o| o as u16);
    let exif = (!ascii.is_empty() || orientation.is_some()).then(|| build_exif(&ascii, orientation));
    Meta { icc, exif }
}

fn tiff_ascii_tags() -> [(u16, tiff::tags::Tag); 7] {
    use tiff::tags::Tag;
    [
        (0x010E, Tag::ImageDescription),
        (0x010F, Tag::Make),
        (0x0110, Tag::Model),
        (0x0131, Tag::Software),
        (0x0132, Tag::DateTime),
        (0x013B, Tag::Artist),
        (0x8298, Tag::Copyright),
    ]
}

// ───────────────────────── Decodificadores ─────────────────────────

pub struct Decoded {
    pub r: Raster,
    pub meta: Meta,
    pub notes: Vec<String>,
}

fn map_img_err(e: image::ImageError) -> ConvertError {
    use image::ImageError as E;
    match e {
        E::Unsupported(_) => ConvertError::Unsupported,
        E::Decoding(_) => ConvertError::Corrupt,
        E::IoError(_) => ConvertError::Corrupt, // decodificamos desde memoria: fin de datos = archivo roto
        E::Limits(_) => ConvertError::Failed("la imagen es demasiado grande".into()),
        other => ConvertError::Failed(other.to_string()),
    }
}

fn raster_from_dynamic(img: image::DynamicImage) -> Raster {
    use image::DynamicImage as D;
    let (w, h) = (img.width(), img.height());
    let (alpha, deep) = match &img {
        D::ImageLuma8(_) | D::ImageRgb8(_) => (false, false),
        D::ImageLumaA8(_) | D::ImageRgba8(_) => (true, false),
        D::ImageLuma16(_) | D::ImageRgb16(_) => (false, true),
        D::ImageLumaA16(_) | D::ImageRgba16(_) => (true, true),
        other => (other.color().has_alpha(), true),
    };
    let pix = match (alpha, deep) {
        (false, false) => Pix::Rgb8(img.into_rgb8().into_raw()),
        (true, false) => Pix::Rgba8(img.into_rgba8().into_raw()),
        (false, true) => Pix::Rgb16(img.into_rgb16().into_raw()),
        (true, true) => Pix::Rgba16(img.into_rgba16().into_raw()),
    };
    Raster { w, h, pix }
}

fn decode_webp(data: &[u8]) -> Result<Raster, ConvertError> {
    let feat = webp::BitstreamFeatures::new(data).ok_or(ConvertError::Corrupt)?;
    if feat.has_animation() {
        return Err(ConvertError::Unsupported); // WebP animado: fuera de alcance
    }
    let img = webp::Decoder::new(data).decode().ok_or(ConvertError::Corrupt)?;
    let (w, h) = (img.width(), img.height());
    let bytes: &[u8] = &img;
    let ch = if feat.has_alpha() { 4 } else { 3 };
    if bytes.len() != (w as usize) * (h as usize) * ch {
        return Err(ConvertError::Corrupt);
    }
    let pix = if ch == 4 { Pix::Rgba8(bytes.to_vec()) } else { Pix::Rgb8(bytes.to_vec()) };
    Ok(Raster { w, h, pix })
}

fn decode_common(path: &Path, src: Src) -> Result<Decoded, ConvertError> {
    let data = std::fs::read(path)?;
    let meta = if src == Src::Tiff { tiff_meta(path) } else { container_meta(src, &data) };
    let r = if src == Src::Webp {
        decode_webp(&data)?
    } else {
        let fmt = match src {
            Src::Png => image::ImageFormat::Png,
            Src::Jpeg => image::ImageFormat::Jpeg,
            _ => image::ImageFormat::Tiff,
        };
        let mut rd = image::io::Reader::with_format(std::io::Cursor::new(&data), fmt);
        let mut lim = image::io::Limits::default();
        lim.max_alloc = Some(4 << 30); // fotos grandes en 16 bits superan el límite por defecto
        rd.limits(lim);
        raster_from_dynamic(rd.decode().map_err(map_img_err)?)
    };
    Ok(Decoded { r, meta, notes: Vec::new() })
}

#[cfg(feature = "heic")]
fn decode_heic(path: &Path) -> Result<Decoded, ConvertError> {
    use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};
    let data = std::fs::read(path)?;
    let ctx = HeifContext::read_from_bytes(&data).map_err(|_| ConvertError::Corrupt)?;
    let handle = ctx.primary_image_handle().map_err(|_| ConvertError::Corrupt)?;
    let (w, h) = (handle.width() as usize, handle.height() as usize);
    let bits = handle.luma_bits_per_pixel().max(8) as u32;
    let alpha = handle.has_alpha_channel();
    let ch = if alpha { 4 } else { 3 };
    let lib = LibHeif::new();

    let r = if bits > 8 {
        let chroma = if alpha { RgbChroma::HdrRgbaLe } else { RgbChroma::HdrRgbLe };
        let img = lib.decode(&handle, ColorSpace::Rgb(chroma), None).map_err(|_| ConvertError::Corrupt)?;
        let plane = img.planes().interleaved.ok_or(ConvertError::Corrupt)?;
        let max = (1u32 << bits) - 1;
        let mut v: Vec<u16> = Vec::with_capacity(w * h * ch);
        for y in 0..h {
            let row = plane.data.get(y * plane.stride..y * plane.stride + w * ch * 2).ok_or(ConvertError::Corrupt)?;
            for px in row.chunks_exact(2) {
                let s = u16::from_le_bytes([px[0], px[1]]) as u32;
                v.push(((s.min(max) * 65535 + max / 2) / max) as u16);
            }
        }
        let pix = if alpha { Pix::Rgba16(v) } else { Pix::Rgb16(v) };
        Raster { w: w as u32, h: h as u32, pix }
    } else {
        let chroma = if alpha { RgbChroma::Rgba } else { RgbChroma::Rgb };
        let img = lib.decode(&handle, ColorSpace::Rgb(chroma), None).map_err(|_| ConvertError::Corrupt)?;
        let plane = img.planes().interleaved.ok_or(ConvertError::Corrupt)?;
        let mut v: Vec<u8> = Vec::with_capacity(w * h * ch);
        for y in 0..h {
            let row = plane.data.get(y * plane.stride..y * plane.stride + w * ch).ok_or(ConvertError::Corrupt)?;
            v.extend_from_slice(row);
        }
        let pix = if alpha { Pix::Rgba8(v) } else { Pix::Rgb8(v) };
        Raster { w: w as u32, h: h as u32, pix }
    };

    // libheif no aplica el perfil ICC al decodificar: hay que llevarlo.
    let icc = handle.color_profile_raw().map(|p| p.data);
    let mut exif = None;
    let mut ids = [0u32; 8];
    let n = handle.metadata_block_ids(&mut ids, b"Exif");
    for id in ids.iter().take(n) {
        if let Ok(block) = handle.metadata(*id) {
            // Bloque HEIF: 4 bytes con el desplazamiento hasta la cabecera TIFF.
            if block.len() > 4 {
                let off = u32::from_be_bytes([block[0], block[1], block[2], block[3]]) as usize;
                if let Some(t) = block.get(4 + off..) {
                    exif = Some(t.to_vec());
                    break;
                }
            }
        }
    }
    Ok(Decoded { r, meta: Meta { icc, exif }, notes: Vec::new() })
}

#[cfg(not(feature = "heic"))]
fn decode_heic(_path: &Path) -> Result<Decoded, ConvertError> {
    Err(ConvertError::Failed(
        "esta compilación no incluye soporte HEIC (ver LEEME-IMAGENES.md)".into(),
    ))
}

fn svg_fonts() -> std::sync::Arc<resvg::usvg::fontdb::Database> {
    use std::sync::{Arc, OnceLock};
    static FONTS: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = resvg::usvg::fontdb::Database::new();
            db.load_system_fonts(); // cargar fuentes cuesta: una sola vez para todo el lote
            Arc::new(db)
        })
        .clone()
}

fn decode_svg(path: &Path) -> Result<Decoded, ConvertError> {
    use resvg::{tiny_skia, usvg};
    let data = std::fs::read(path)?;
    let mut opt = usvg::Options::default();
    opt.fontdb = svg_fonts();
    let tree = usvg::Tree::from_data(&data, &opt).map_err(|_| ConvertError::Corrupt)?;
    let (sw, sh) = (tree.size().width(), tree.size().height());
    if !(sw > 0.0 && sh > 0.0) {
        return Err(ConvertError::Corrupt);
    }
    let longest = sw.max(sh);
    let mut scale = 1.0f32;
    if longest < SVG_MIN_SIDE {
        scale = SVG_MIN_SIDE / longest;
    }
    if longest * scale > SVG_MAX_SIDE {
        scale = SVG_MAX_SIDE / longest;
    }
    let pw = (sw * scale).round().max(1.0) as u32;
    let ph = (sh * scale).round().max(1.0) as u32;
    let mut pm = tiny_skia::Pixmap::new(pw, ph).ok_or_else(|| ConvertError::Failed("el SVG es demasiado grande".into()))?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(pw as f32 / sw, ph as f32 / sh), &mut pm.as_mut());
    let mut rgba = Vec::with_capacity(pw as usize * ph as usize * 4);
    for p in pm.pixels() {
        let c = p.demultiply(); // tiny-skia guarda alfa premultiplicado
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Ok(Decoded {
        r: Raster { w: pw, h: ph, pix: Pix::Rgba8(rgba) },
        meta: Meta::default(),
        notes: vec![format!("El SVG es vectorial: se convirtió en una imagen de {pw}×{ph} px.")],
    })
}

fn decode_raw(path: &Path, want16: bool) -> Result<Decoded, ConvertError> {
    let raw_err = |e: String| {
        let l = e.to_lowercase();
        if l.contains("camera") || l.contains("upported") {
            ConvertError::Unsupported
        } else {
            ConvertError::Corrupt
        }
    };
    let mut p = imagepipe::Pipeline::new_from_file(path).map_err(raw_err)?;
    let (w, h, pix) = if want16 {
        let i = p.output_16bit(None).map_err(raw_err)?;
        (i.width, i.height, Pix::Rgb16(i.data))
    } else {
        let i = p.output_8bit(None).map_err(raw_err)?;
        (i.width, i.height, Pix::Rgb8(i.data))
    };
    let len = match &pix {
        Pix::Rgb8(v) => v.len(),
        Pix::Rgb16(v) => v.len(),
        _ => 0,
    };
    if len != w * h * 3 {
        return Err(ConvertError::Corrupt);
    }
    Ok(Decoded {
        r: Raster { w: w as u32, h: h as u32, pix },
        meta: Meta::default(),
        notes: vec!["RAW revelado con ajustes básicos; no se transfirieron los metadatos de la cámara.".into()],
    })
}

fn decode(path: &Path, src: Src, want16: bool) -> Result<Decoded, ConvertError> {
    let mut d = match src {
        Src::Heic => decode_heic(path)?,
        Src::Svg => decode_svg(path)?,
        Src::Raw => decode_raw(path, want16)?,
        _ => decode_common(path, src)?,
    };
    if let Some(exif) = d.meta.exif.as_mut() {
        let o = exif_orientation(exif);
        // HEIC: libheif ya aplicó el giro; en el resto lo aplicamos nosotros.
        if src != Src::Heic && (2..=8).contains(&o) {
            let r = std::mem::replace(&mut d.r, Raster::empty());
            d.r = r.apply_orientation(o);
        }
        reset_orientation(exif);
    }
    Ok(d)
}

// ───────────────────────── Codificadores ─────────────────────────

fn ioerr(e: std::io::Error) -> ConvertError {
    ConvertError::Io(e)
}

fn encode_png(r: &Raster, preset: QualityPreset) -> Result<Vec<u8>, ConvertError> {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    use image::{ColorType, ImageEncoder};
    let (bytes, ct): (Cow<[u8]>, ColorType) = match &r.pix {
        Pix::Rgb8(v) => (Cow::Borrowed(v), ColorType::Rgb8),
        Pix::Rgba8(v) => (Cow::Borrowed(v), ColorType::Rgba8),
        Pix::Rgb16(v) => (Cow::Owned(v.iter().flat_map(|x| x.to_ne_bytes()).collect()), ColorType::Rgb16),
        Pix::Rgba16(v) => (Cow::Owned(v.iter().flat_map(|x| x.to_ne_bytes()).collect()), ColorType::Rgba16),
    };
    let level = if preset == QualityPreset::Compact { CompressionType::Best } else { CompressionType::Default };
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, level, FilterType::Adaptive)
        .write_image(&bytes, r.w, r.h, ct)
        .map_err(map_img_err)?;
    Ok(out)
}

fn encode_jpeg(r: &Raster, preset: QualityPreset) -> Result<Vec<u8>, ConvertError> {
    let Pix::Rgb8(rgb) = &r.pix else { return Err(ConvertError::Unsupported) };
    if r.w > JPEG_MAX_SIDE || r.h > JPEG_MAX_SIDE {
        return Err(ConvertError::Failed(format!("JPEG admite hasta {JPEG_MAX_SIDE} px por lado")));
    }
    // "Máxima": calidad 98 sin submuestreo de color (4:4:4). El resto usa el 4:2:0 habitual.
    let (q, full_chroma) = match preset {
        QualityPreset::Lossless => (98.0, true),
        QualityPreset::Balanced => (85.0, false),
        QualityPreset::Compact => (72.0, false),
    };
    let mut c = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
    c.set_size(r.w as usize, r.h as usize);
    c.set_quality(q);
    if full_chroma {
        c.set_chroma_sampling_pixel_sizes((1, 1), (1, 1));
    }
    c.set_progressive_mode();
    let mut c = c.start_compress(Vec::new()).map_err(ioerr)?;
    c.write_scanlines(rgb).map_err(ioerr)?;
    c.finish().map_err(ioerr)
}

fn encode_webp(r: &Raster, preset: QualityPreset) -> Result<Vec<u8>, ConvertError> {
    if r.w > WEBP_MAX_SIDE || r.h > WEBP_MAX_SIDE {
        return Err(ConvertError::Failed(format!("WEBP admite hasta {WEBP_MAX_SIDE} px por lado")));
    }
    let (data, layout): (&[u8], webp::PixelLayout) = match &r.pix {
        Pix::Rgb8(v) => (v, webp::PixelLayout::Rgb),
        Pix::Rgba8(v) => (v, webp::PixelLayout::Rgba),
        _ => return Err(ConvertError::Unsupported),
    };
    let mut cfg = webp::WebPConfig::new().map_err(|_| ConvertError::Failed("configuración de WEBP".into()))?;
    match preset {
        QualityPreset::Lossless => {
            cfg.lossless = 1;
            cfg.quality = 80.0; // en modo sin pérdida es esfuerzo de compresión, no calidad
            cfg.method = 5;
            cfg.exact = 1; // conserva también el RGB de los píxeles totalmente transparentes
        }
        QualityPreset::Balanced => {
            cfg.quality = 85.0;
            cfg.method = 4;
            cfg.use_sharp_yuv = 1;
        }
        QualityPreset::Compact => {
            cfg.quality = 70.0;
            cfg.method = 5;
            cfg.use_sharp_yuv = 1;
        }
    }
    cfg.alpha_quality = 100;
    let mem = webp::Encoder::new(data, layout, r.w, r.h)
        .encode_advanced(&cfg)
        .map_err(|e| ConvertError::Failed(format!("WEBP: {e:?}")))?;
    Ok(mem.to_vec())
}

fn terr(e: tiff::TiffError) -> ConvertError {
    ConvertError::Failed(format!("TIFF: {e}"))
}

/// TIFF conserva el perfil ICC y los textos básicos de cámara (marca, modelo, fecha, autor,
/// copyright...) como etiquetas propias. El resto del EXIF (exposición, GPS) no cabe aquí.
fn encode_tiff(r: &Raster, meta: &Meta) -> Result<Vec<u8>, ConvertError> {
    use tiff::encoder::{colortype, compression::Deflate, TiffEncoder};
    use tiff::tags::Tag;
    let basic = meta.exif.as_deref().map(basic_fields).unwrap_or_default();
    let tags = tiff_ascii_tags();
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut enc = TiffEncoder::new(&mut buf).map_err(terr)?;
        macro_rules! image {
            ($ct:ty, $data:expr) => {{
                let mut img = enc
                    .new_image_with_compression::<$ct, _>(r.w, r.h, Deflate::default())
                    .map_err(terr)?;
                if let Some(icc) = &meta.icc {
                    img.encoder().write_tag(Tag::Unknown(34675), &icc[..]).map_err(terr)?;
                }
                for (code, text) in &basic {
                    if let Some((_, tag)) = tags.iter().find(|(c, _)| c == code) {
                        img.encoder().write_tag(*tag, text.as_str()).map_err(terr)?;
                    }
                }
                img.write_data($data).map_err(terr)?;
            }};
        }
        match &r.pix {
            Pix::Rgb8(v) => image!(colortype::RGB8, &v[..]),
            Pix::Rgba8(v) => image!(colortype::RGBA8, &v[..]),
            Pix::Rgb16(v) => image!(colortype::RGB16, &v[..]),
            Pix::Rgba16(v) => image!(colortype::RGBA16, &v[..]),
        }
    }
    Ok(buf.into_inner())
}

/// Añade ICC y EXIF a un PNG o JPEG ya codificado, sin recodificar los píxeles.
macro_rules! inject {
    ($ty:ty, $bytes:expr, $meta:expr) => {{
        if $meta.icc.is_none() && $meta.exif.is_none() {
            Ok($bytes)
        } else {
            let mut img = <$ty>::from_bytes(Bytes::from($bytes)).map_err(meta_err)?;
            if let Some(i) = &$meta.icc {
                img.set_icc_profile(Some(Bytes::from(i.clone())));
            }
            if let Some(e) = &$meta.exif {
                img.set_exif(Some(Bytes::from(e.clone())));
            }
            let mut out = Vec::new();
            img.encoder().write_to(&mut out).map_err(ioerr)?;
            Ok::<Vec<u8>, ConvertError>(out)
        }
    }};
}

/// img-parts inserta los segmentos en una posición fija (3) que, tras mozjpeg, cae DETRÁS del
/// encabezado de imagen (SOF): algunos lectores no los ven ahí. Se colocan tras JFIF: EXIF, luego ICC.
fn inject_jpeg(data: Vec<u8>, meta: &Meta) -> Result<Vec<u8>, ConvertError> {
    use img_parts::jpeg::{markers, Jpeg, JpegSegment};
    if meta.icc.is_none() && meta.exif.is_none() {
        return Ok(data);
    }
    let mut j = Jpeg::from_bytes(Bytes::from(data)).map_err(meta_err)?;
    if let Some(i) = &meta.icc {
        j.set_icc_profile(Some(Bytes::from(i.clone())));
    }
    if let Some(e) = &meta.exif {
        j.set_exif(Some(Bytes::from(e.clone())));
    }
    fn is_exif(s: &JpegSegment) -> bool {
        s.marker() == markers::APP1 && s.contents().starts_with(b"Exif\0\0")
    }
    fn is_icc(s: &JpegSegment) -> bool {
        s.marker() == markers::APP2 && s.contents().starts_with(b"ICC_PROFILE\0")
    }
    let segs = j.segments_mut();
    let (mut exif, mut icc, mut rest) = (Vec::new(), Vec::new(), Vec::new());
    for s in segs.drain(..) {
        if is_exif(&s) {
            exif.push(s);
        } else if is_icc(&s) {
            icc.push(s);
        } else {
            rest.push(s);
        }
    }
    icc.sort_by_key(|s| s.contents().get(12).copied().unwrap_or(0)); // número de secuencia
    let at = rest.iter().position(|s| s.marker() != markers::APP0).unwrap_or(rest.len());
    let tail = rest.split_off(at);
    segs.extend(rest);
    segs.extend(exif);
    segs.extend(icc);
    segs.extend(tail);
    let mut out = Vec::new();
    j.encoder().write_to(&mut out).map_err(ioerr)?;
    Ok(out)
}

fn inject_webp(data: Vec<u8>, meta: &Meta) -> Result<Vec<u8>, ConvertError> {
    use img_parts::riff::{RiffChunk, RiffContent};
    use img_parts::webp::{WebP, CHUNK_EXIF, CHUNK_VP8X};
    if meta.icc.is_none() && meta.exif.is_none() {
        return Ok(data);
    }
    let mut w = WebP::from_bytes(Bytes::from(data)).map_err(meta_err)?;
    if let Some(i) = &meta.icc {
        w.set_icc_profile(Some(Bytes::from(i.clone())));
    }
    if let Some(e) = &meta.exif {
        w.set_exif(Some(Bytes::from(e.clone()))); // crea el contenedor VP8X si hace falta
        // img-parts antepone "Exif\0\0"; el estándar de WebP pide el bloque TIFF crudo.
        if let Some(c) = w.chunks_mut().iter_mut().find(|c| c.id() == CHUNK_EXIF) {
            *c = RiffChunk::new(CHUNK_EXIF, RiffContent::Data(Bytes::from(e.clone())));
        }
    }
    // img-parts coloca ICCP justo antes de VP8 (detrás de ALPH en WebP con transparencia y pérdida);
    // el estándar exige que vaya inmediatamente después de VP8X, y libwebp rechaza el archivo si no.
    {
        let chunks = w.chunks_mut();
        if let Some(pos) = chunks.iter().position(|c| c.id() == img_parts::webp::CHUNK_ICCP) {
            let c = chunks.remove(pos);
            let after_vp8x = chunks.iter().position(|c| c.id() == CHUNK_VP8X).map_or(0, |i| i + 1);
            chunks.insert(after_vp8x, c);
        }
    }
    // img-parts no actualiza los flags de VP8X si ya existía (p. ej. WebP con transparencia):
    // sin esto, un visor ignoraría el perfil de color aunque esté en el archivo.
    if let Some(c) = w.chunks_mut().iter_mut().find(|c| c.id() == CHUNK_VP8X) {
        if let Some(d) = c.content().data() {
            let mut v = d.to_vec();
            if !v.is_empty() {
                if meta.icc.is_some() {
                    v[0] |= 0x20;
                }
                if meta.exif.is_some() {
                    v[0] |= 0x08;
                }
            }
            *c = RiffChunk::new(CHUNK_VP8X, RiffContent::Data(Bytes::from(v)));
        }
    }
    let mut out = Vec::new();
    w.encoder().write_to(&mut out).map_err(ioerr)?;
    Ok(out)
}

// ───────────────────────── Conversión ─────────────────────────

/// ICC y EXIF utilizables en el destino; lo que no se puede llevar se avisa.
fn prepare_meta(meta: &Meta, target: &str, notes: &mut Vec<String>) -> Meta {
    let mut m = meta.clone();
    if let Some(icc) = &m.icc {
        if !valid_icc(icc) {
            m.icc = None;
        } else if !icc_is_rgb(icc) {
            notes.push("El perfil de color no es RGB (p. ej. escala de grises): no se incluyó.".into());
            m.icc = None;
        }
    }
    if target == "jpeg" && m.exif.as_ref().map_or(false, |e| e.len() > JPEG_EXIF_MAX) {
        notes.push("Los metadatos EXIF no caben en un JPEG: no se incluyeron.".into());
        m.exif = None;
    }
    if target == "tiff" && m.exif.as_deref().map_or(false, has_detailed_exif) {
        notes.push(
            "TIFF solo conserva los datos básicos de cámara (marca, modelo, fecha, autor); \
             el resto del EXIF no cabe."
                .into(),
        );
    }
    m
}

fn transcode(path: &Path, src: Src, target: &str, preset: QualityPreset) -> Result<(Vec<u8>, Vec<String>), ConvertError> {
    let want16 = matches!(target, "png" | "tiff");
    let d = decode(path, src, want16)?;
    let mut notes = d.notes;
    let mut r = d.r;

    if src.is_lossy() && (want16 || (target == "webp" && preset == QualityPreset::Lossless)) {
        notes.push(format!(
            "El original ({}) ya tenía pérdida: se conservan sus píxeles tal cual, pero el archivo pesará más.",
            src.label()
        ));
    }
    match target {
        "jpeg" => {
            if r.has_real_transparency() {
                notes.push("JPEG no admite transparencia: se rellenó con fondo blanco.".into());
            }
            if r.is_16() {
                notes.push("JPEG guarda 8 bits por canal: se redujo desde 16 bits.".into());
            }
            r = r.flatten_white();
        }
        "webp" if r.is_16() => {
            notes.push("WEBP guarda 8 bits por canal: se redujo desde 16 bits.".into());
            r = r.to_8();
        }
        _ => {}
    }
    let meta = prepare_meta(&d.meta, target, &mut notes);

    let bytes = match target {
        "png" => inject!(img_parts::png::Png, encode_png(&r, preset)?, meta)?,
        "jpeg" => inject_jpeg(encode_jpeg(&r, preset)?, &meta)?,
        "webp" => inject_webp(encode_webp(&r, preset)?, &meta)?,
        "tiff" => encode_tiff(&r, &meta)?,
        _ => return Err(ConvertError::Unsupported),
    };
    Ok((bytes, notes))
}

pub fn convert(job: &JobSpec, target: &Path) -> Result<Outcome, ConvertError> {
    let src_path = &job.item.path;
    let fmt = job.target_format.as_str();
    if !IMAGE_TARGETS.contains(&fmt) {
        return Err(ConvertError::Unsupported);
    }
    // El escaneo solo entrega extensiones de imagen: si el contenido no encaja, está dañado.
    let src = detect(src_path, &job.item.ext).ok_or(ConvertError::Corrupt)?;
    let src_len = std::fs::metadata(src_path)?.len();
    let tmp = partial_path(target);

    let result: Result<(Mode, Vec<String>), ConvertError> = (|| match plan(src, fmt, job.preset) {
        Action::FileCopy => {
            std::fs::copy(src_path, &tmp)?;
            Ok((Mode::Passthrough, Vec::new()))
        }
        Action::Transcode { keep_smaller } => {
            // Un decodificador que entra en pánico con un archivo roto no debe tumbar el lote.
            let (bytes, mut notes) = catch_unwind(AssertUnwindSafe(|| transcode(src_path, src, fmt, job.preset)))
                .map_err(|_| ConvertError::Corrupt)??;
            if keep_smaller && bytes.len() as u64 >= src_len {
                std::fs::copy(src_path, &tmp)?;
                notes.push("No se recomprimió: el resultado no era más pequeño que el original.".into());
                Ok((Mode::Passthrough, notes))
            } else {
                std::fs::write(&tmp, &bytes)?;
                Ok((Mode::Reencoded, notes))
            }
        }
    })()
    .and_then(|ok| std::fs::rename(&tmp, target).map(|_| ok).map_err(Into::into));

    match result {
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
        Ok((mode, warnings)) => {
            let out_len = std::fs::metadata(target)?.len() as i64;
            Ok(Outcome::Converted { bytes_saved: src_len as i64 - out_len, mode, warnings })
        }
    }
}

// ───────────────────────── Pruebas ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ConflictPolicy, MediaItem, MediaKind};
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::OnceLock;

    // ── unitarias (sin dependencias externas) ──

    fn rgb(w: u32, h: u32, v: Vec<u8>) -> Raster {
        Raster { w, h, pix: Pix::Rgb8(v) }
    }
    fn bytes(r: &Raster) -> Vec<u8> {
        match &r.pix {
            Pix::Rgb8(v) => v.clone(),
            _ => panic!(),
        }
    }

    #[test]
    fn orientaciones_exif() {
        let src = || rgb(2, 1, vec![1, 1, 1, 2, 2, 2]); // [A B]
        let r = src().apply_orientation(6); // 90° horario: A arriba, B abajo
        assert_eq!((r.w, r.h), (1, 2));
        assert_eq!(bytes(&r), vec![1, 1, 1, 2, 2, 2]);
        let r = src().apply_orientation(8); // 90° antihorario: B arriba, A abajo
        assert_eq!((r.w, r.h), (1, 2));
        assert_eq!(bytes(&r), vec![2, 2, 2, 1, 1, 1]);
        let r = src().apply_orientation(3); // 180°
        assert_eq!((r.w, r.h), (2, 1));
        assert_eq!(bytes(&r), vec![2, 2, 2, 1, 1, 1]);
        let r = src().apply_orientation(2); // espejo horizontal
        assert_eq!(bytes(&r), vec![2, 2, 2, 1, 1, 1]);
        let r = src().apply_orientation(1);
        assert_eq!(bytes(&r), vec![1, 1, 1, 2, 2, 2]);
    }

    #[test]
    fn exif_construir_leer_y_reiniciar_orientacion() {
        let mut blob = build_exif(&[(0x010F, "Canon".into()), (0x0110, "EOS R5 Mark II".into())], Some(6));
        assert_eq!(exif_orientation(&blob), 6);
        let f = basic_fields(&blob);
        assert!(f.contains(&(0x010F, "Canon".to_string())));
        assert!(f.contains(&(0x0110, "EOS R5 Mark II".to_string())));
        reset_orientation(&mut blob);
        assert_eq!(exif_orientation(&blob), 1);
        assert_eq!(basic_fields(&blob).len(), 2, "reiniciar la orientación no debe tocar lo demás");
        assert!(!has_detailed_exif(&blob));
    }

    #[test]
    fn exif_basura_no_rompe() {
        assert_eq!(exif_orientation(b"basura"), 1);
        assert!(basic_fields(&[0u8; 3]).is_empty());
        let mut b = b"II*\0\xff\xff\xff\xff".to_vec();
        reset_orientation(&mut b); // desplazamiento fuera de rango: no debe entrar en pánico
    }

    #[test]
    fn validacion_icc() {
        let mut p = vec![0u8; 200];
        p[36..40].copy_from_slice(b"acsp");
        p[16..20].copy_from_slice(b"RGB ");
        assert!(valid_icc(&p) && icc_is_rgb(&p));
        p[16..20].copy_from_slice(b"GRAY");
        assert!(!icc_is_rgb(&p));
        assert!(!valid_icc(&[1, 2, 3]));
    }

    #[test]
    fn plan_de_copia_y_recompresion() {
        use QualityPreset::*;
        assert_eq!(plan(Src::Png, "png", Lossless), Action::FileCopy);
        assert_eq!(plan(Src::Png, "png", Compact), Action::FileCopy);
        assert_eq!(plan(Src::Tiff, "tiff", Compact), Action::FileCopy);
        assert_eq!(plan(Src::Jpeg, "jpeg", Balanced), Action::FileCopy);
        assert_eq!(plan(Src::Jpeg, "jpeg", Compact), Action::Transcode { keep_smaller: true });
        assert_eq!(plan(Src::Heic, "jpeg", Lossless), Action::Transcode { keep_smaller: false });
        assert_eq!(plan(Src::Svg, "png", Lossless), Action::Transcode { keep_smaller: false });
    }

    #[test]
    fn transparencia_real_vs_alfa_opaco() {
        let opaco = Raster { w: 1, h: 1, pix: Pix::Rgba8(vec![9, 9, 9, 255]) };
        let claro = Raster { w: 1, h: 1, pix: Pix::Rgba8(vec![9, 9, 9, 0]) };
        assert!(!opaco.has_real_transparency());
        assert!(claro.has_real_transparency());
        let Pix::Rgb8(v) = claro.flatten_white().pix else { panic!() };
        assert_eq!(v, vec![255, 255, 255]);
    }

    // ── integración: fixtures creados e inspeccionados con PIL (independiente de este código) ──

    const FIXTURES_PY: &str = r#"
import sys, numpy as np
from PIL import Image, ImageCms
d = sys.argv[1]
icc = ImageCms.ImageCmsProfile(ImageCms.createProfile("sRGB")).tobytes()
y, x = np.mgrid[0:48, 0:64]
smooth = np.stack([(x*4) % 256, (y*5) % 256, ((x+y)*2) % 256], -1).astype(np.uint8)
noise = np.random.default_rng(7).integers(0, 256, (48, 64, 3), dtype=np.uint8)
ex = Image.Exif(); ex[0x010F] = "TestMake"; ex[0x0110] = "TestModel"; ex[0x0112] = 6
Image.fromarray(noise).save(f"{d}/noise.png", icc_profile=icc, exif=ex)
Image.fromarray(noise).save(f"{d}/noise.webp", lossless=True, icc_profile=icc, exif=ex)
Image.fromarray(noise).save(f"{d}/noise.tif", icc_profile=icc, tiffinfo={271: "TestMake", 272: "TestModel", 274: 6})
Image.fromarray(smooth).save(f"{d}/smooth.jpg", quality=95, icc_profile=icc, exif=ex)
Image.fromarray(smooth).save(f"{d}/smooth.png")
alpha = np.tile(np.linspace(0, 255, 64).astype(np.uint8), (48, 1))
rgba = np.dstack([noise, alpha]); rgba[0, 0] = [10, 20, 30, 0]
Image.fromarray(rgba).save(f"{d}/alpha.png")
Image.fromarray(rgba).save(f"{d}/alpha_icc.png", icc_profile=icc)
open(f"{d}/icc.bin", "wb").write(icc)
"#;

    const INSPECT_PY: &str = r#"
import sys, json, hashlib, numpy as np
from PIL import Image
im = Image.open(sys.argv[1]); im.load()
ex = im.getexif(); icc = im.info.get("icc_profile")
a = np.array(im.convert("RGBA" if "A" in im.getbands() else "RGB"))
print(json.dumps({"format": im.format, "mode": im.mode, "size": list(im.size),
  "icc": hashlib.sha256(icc).hexdigest() if icc else None,
  "make": ex.get(0x010F), "model": ex.get(0x0110), "orient": ex.get(0x0112),
  "px00": a[0, 0].tolist(), "pxc": a[a.shape[0]//2, a.shape[1]//2].tolist()}))
"#;

    const DIFF_PY: &str = r#"
import sys, numpy as np
from PIL import Image
mode = sys.argv[4]
a = np.array(Image.open(sys.argv[1]).convert(mode)).astype(int)
b = np.array(Image.open(sys.argv[2]).convert(mode)).astype(int)
if sys.argv[3] == "6": a = np.rot90(a, k=-1)
if a.shape != b.shape: print("999 999"); sys.exit()
d = np.abs(a - b); print(d.max(), d.mean())
"#;

    fn py(script: &str, args: &[&str]) -> Option<String> {
        let o = Command::new("python3").arg("-c").arg(script).args(args).output().ok()?;
        o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
    }

    /// Carpeta de fixtures (se generan una sola vez). `None` si no hay python3 + PIL + numpy.
    fn fixtures() -> Option<&'static PathBuf> {
        static F: OnceLock<Option<PathBuf>> = OnceLock::new();
        F.get_or_init(|| {
            let d = std::env::temp_dir().join(format!("conversor-img-fix-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).ok()?;
            py(FIXTURES_PY, &[d.to_str()?])?;
            Some(d)
        })
        .as_ref()
    }

    fn work(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("conversor-img-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn inspect(p: &Path) -> serde_json::Value {
        serde_json::from_str(&py(INSPECT_PY, &[p.to_str().unwrap()]).expect("PIL no pudo abrir la salida")).unwrap()
    }

    fn diff(a: &Path, b: &Path, rot6: bool, mode: &str) -> (f64, f64) {
        let o = py(DIFF_PY, &[a.to_str().unwrap(), b.to_str().unwrap(), if rot6 { "6" } else { "0" }, mode]).unwrap();
        let v: Vec<f64> = o.split_whitespace().map(|x| x.parse().unwrap()).collect();
        (v[0], v[1])
    }

    fn job(path: &Path, target: &str, preset: QualityPreset) -> JobSpec {
        JobSpec {
            item: MediaItem {
                id: "t".into(),
                path: path.into(),
                name: path.file_name().unwrap().to_string_lossy().into(),
                ext: path.extension().unwrap().to_string_lossy().into(),
                kind: MediaKind::Image,
                is_video: false,
                size_bytes: std::fs::metadata(path).unwrap().len(),
            },
            target_format: target.into(),
            preset,
        }
    }

    fn run(src: &Path, dir: &Path, target: &str, preset: QualityPreset) -> Result<(PathBuf, Mode, Vec<String>), ConvertError> {
        let j = job(src, target, preset);
        let out = crate::engine::resolve_target(&j, dir, ConflictPolicy::Rename).unwrap();
        match convert(&j, &out)? {
            Outcome::Converted { mode, warnings, .. } => Ok((out, mode, warnings)),
            Outcome::Skipped => panic!("omitido"),
        }
    }

    fn icc_sha(fx: &Path) -> String {
        py("import sys,hashlib;print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())", &[fx.join("icc.bin").to_str().unwrap()]).unwrap()
    }

    macro_rules! skip_sin_fixtures {
        () => {
            match fixtures() {
                Some(f) => f,
                None => return,
            }
        };
    }

    #[test]
    fn png_a_tiff_conserva_icc_exif_orientacion_y_pixeles() {
        let fx = skip_sin_fixtures!();
        let d = work("png-tiff");
        let (out, mode, w) = run(&fx.join("noise.png"), &d, "tiff", QualityPreset::Lossless).unwrap();
        assert_eq!(mode, Mode::Reencoded);
        let i = inspect(&out);
        assert_eq!(i["format"], "TIFF");
        assert_eq!(i["size"], serde_json::json!([48, 64]), "la orientación 6 debe aplicarse a los píxeles");
        assert_eq!(i["icc"], icc_sha(fx), "el perfil ICC debe ser idéntico");
        assert_eq!(i["make"], "TestMake");
        assert_eq!(i["model"], "TestModel");
        assert!(i["orient"].is_null() || i["orient"] == 1, "orientación: {}", i["orient"]);
        assert_eq!(diff(&fx.join("noise.png"), &out, true, "RGB"), (0.0, 0.0), "los píxeles deben ser idénticos");
        assert!(w.is_empty(), "no debería avisar nada: {w:?}");
    }

    #[test]
    fn png_a_webp_sin_perdida_es_identico() {
        let fx = skip_sin_fixtures!();
        let d = work("png-webp");
        let (out, _, w) = run(&fx.join("noise.png"), &d, "webp", QualityPreset::Lossless).unwrap();
        let i = inspect(&out);
        assert_eq!(i["format"], "WEBP");
        assert_eq!(i["size"], serde_json::json!([48, 64]));
        assert_eq!(i["icc"], icc_sha(fx));
        assert_eq!(i["make"], "TestMake");
        assert!(i["orient"].is_null() || i["orient"] == 1);
        assert_eq!(diff(&fx.join("noise.png"), &out, true, "RGB"), (0.0, 0.0));
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn webp_con_alfa_y_icc_mantiene_el_perfil_y_el_alfa_exacto() {
        let fx = skip_sin_fixtures!();
        let d = work("alpha");
        // Sin pérdida: RGBA exacto, incluido el RGB oculto bajo alfa 0 (exact=1)
        let (out, _, _) = run(&fx.join("alpha_icc.png"), &d, "webp", QualityPreset::Lossless).unwrap();
        assert_eq!(inspect(&out)["icc"], icc_sha(fx));
        assert_eq!(diff(&fx.join("alpha_icc.png"), &out, false, "RGBA"), (0.0, 0.0));
        // Con pérdida + alfa: libwebp crea VP8X y hay que marcar el flag ICC (si no, el visor lo ignora)
        let (out, _, _) = run(&fx.join("alpha_icc.png"), &d, "webp", QualityPreset::Balanced).unwrap();
        let i = inspect(&out);
        assert_eq!(i["mode"], "RGBA");
        assert_eq!(i["icc"], icc_sha(fx), "perfil perdido en WebP con transparencia");
    }

    #[test]
    fn png_a_jpeg_maxima_conserva_metadatos_y_calidad() {
        let fx = skip_sin_fixtures!();
        let d = work("png-jpeg");
        // noise.png trae ICC + EXIF + orientación 6; el degradado permite medir la calidad
        let (out, _, w) = run(&fx.join("noise.png"), &d, "jpeg", QualityPreset::Lossless).unwrap();
        let i = inspect(&out);
        assert_eq!(i["format"], "JPEG");
        assert_eq!(i["size"], serde_json::json!([48, 64]));
        assert_eq!(i["icc"], icc_sha(fx));
        assert_eq!(i["make"], "TestMake");
        assert!(i["orient"].is_null() || i["orient"] == 1);
        assert!(w.is_empty(), "{w:?}");
        let (out, _, _) = run(&fx.join("smooth.png"), &d, "jpeg", QualityPreset::Lossless).unwrap();
        let (max, mean) = diff(&fx.join("smooth.png"), &out, false, "RGB");
        assert!(mean < 1.5 && max <= 12.0, "calidad 'Máxima' demasiado baja: max {max} media {mean}");
    }

    #[test]
    fn jpeg_a_png_lleva_icc_exif_y_gira_el_pixel() {
        let fx = skip_sin_fixtures!();
        let d = work("jpeg-png");
        let (out, _, w) = run(&fx.join("smooth.jpg"), &d, "png", QualityPreset::Lossless).unwrap();
        let i = inspect(&out);
        assert_eq!(i["size"], serde_json::json!([48, 64]));
        assert_eq!(i["icc"], icc_sha(fx));
        assert_eq!(i["make"], "TestMake");
        assert!(i["orient"].is_null() || i["orient"] == 1);
        let (max, _) = diff(&fx.join("smooth.jpg"), &out, true, "RGB");
        assert!(max <= 3.0, "los píxeles difieren más de lo esperable entre decodificadores: {max}");
        assert!(w.iter().any(|m| m.contains("ya tenía pérdida")), "falta el aviso de origen con pérdida: {w:?}");
    }

    #[test]
    fn mismo_formato_copia_byte_a_byte() {
        let fx = skip_sin_fixtures!();
        let d = work("copia");
        for (f, t) in [("noise.png", "png"), ("smooth.jpg", "jpeg"), ("noise.tif", "tiff"), ("noise.webp", "webp")] {
            let (out, mode, _) = run(&fx.join(f), &d, t, QualityPreset::Lossless).unwrap();
            assert_eq!(mode, Mode::Passthrough, "{f}");
            assert_eq!(std::fs::read(fx.join(f)).unwrap(), std::fs::read(out).unwrap(), "{f}");
        }
    }

    #[test]
    fn compacta_en_jpeg_nunca_devuelve_algo_mas_grande() {
        let fx = skip_sin_fixtures!();
        let d = work("compacta");
        let src = fx.join("smooth.jpg");
        let (out, _, _) = run(&src, &d, "jpeg", QualityPreset::Compact).unwrap();
        assert!(std::fs::metadata(&out).unwrap().len() <= std::fs::metadata(&src).unwrap().len());
    }

    #[test]
    fn rgba_a_jpeg_rellena_de_blanco_y_avisa() {
        let fx = skip_sin_fixtures!();
        let d = work("rgba-jpeg");
        let (out, _, w) = run(&fx.join("alpha.png"), &d, "jpeg", QualityPreset::Lossless).unwrap();
        let i = inspect(&out);
        assert_eq!(i["mode"], "RGB");
        let px: Vec<i64> = i["px00"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
        assert!(px.iter().all(|c| *c >= 248), "píxel transparente debería quedar blanco: {px:?}");
        assert!(w.iter().any(|m| m.contains("transparencia")), "{w:?}");
    }

    #[test]
    fn png_de_16_bits_se_conserva_en_tiff_y_se_avisa_en_webp() {
        let d = work("16bit");
        let mut img = image::ImageBuffer::<image::Rgb<u16>, Vec<u16>>::new(40, 30);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgb([(x * 1601 + y * 7) as u16, (y * 2113 + x) as u16, ((x * y * 37) % 65536) as u16]);
        }
        let src = d.join("deep.png");
        img.save(&src).unwrap();
        let (tif, _, w) = run(&src, &d, "tiff", QualityPreset::Lossless).unwrap();
        assert!(w.is_empty(), "{w:?}");
        let back = image::open(&tif).unwrap().into_rgb16();
        assert_eq!(back.as_raw(), img.as_raw(), "TIFF de 16 bits no es idéntico");
        let depth = Command::new("identify").args(["-format", "%z", tif.to_str().unwrap()]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&depth.stdout).trim(), "16", "ImageMagick no ve 16 bits");
        let (_, _, w) = run(&src, &d, "webp", QualityPreset::Lossless).unwrap();
        assert!(w.iter().any(|m| m.contains("8 bits")), "{w:?}");
        let (png, _, _) = run(&src, &d, "png", QualityPreset::Lossless).unwrap();
        assert_eq!(std::fs::read(&src).unwrap(), std::fs::read(png).unwrap(), "PNG→PNG debe ser copia");
    }

    #[test]
    fn svg_se_rasteriza_con_transparencia() {
        let fx = skip_sin_fixtures!();
        let _ = fx;
        let d = work("svg");
        let svg = d.join("logo.svg");
        std::fs::write(&svg, r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50"><circle cx="50" cy="25" r="20" fill="#ff0000"/></svg>"##).unwrap();
        let (out, _, w) = run(&svg, &d, "png", QualityPreset::Lossless).unwrap();
        let i = inspect(&out);
        assert_eq!(i["size"], serde_json::json!([2048, 1024]));
        assert_eq!(i["mode"], "RGBA");
        assert_eq!(i["pxc"], serde_json::json!([255, 0, 0, 255]));
        assert_eq!(i["px00"][3], 0, "las esquinas deben ser transparentes");
        assert!(w.iter().any(|m| m.contains("vectorial")), "{w:?}");
    }

    #[test]
    fn tiff_y_webp_de_origen_aportan_icc_exif_y_orientacion() {
        let fx = skip_sin_fixtures!();
        let d = work("origenes");
        for f in ["noise.tif", "noise.webp"] {
            let (out, _, _) = run(&fx.join(f), &d, "png", QualityPreset::Lossless).unwrap();
            let i = inspect(&out);
            assert_eq!(i["size"], serde_json::json!([48, 64]), "{f}: orientación sin aplicar");
            assert_eq!(i["icc"], icc_sha(fx), "{f}: ICC");
            assert_eq!(i["make"], "TestMake", "{f}: EXIF");
            assert_eq!(diff(&fx.join("noise.png"), &out, true, "RGB"), (0.0, 0.0), "{f}: píxeles");
        }
    }

    #[cfg(feature = "heic")]
    #[test]
    fn heic_se_convierte_con_icc_y_orientacion_correcta() {
        let fx = skip_sin_fixtures!();
        let d = work("heic");
        let heic = d.join("foto.heic");
        let ok = Command::new("heif-enc")
            .args(["-q", "90", fx.join("smooth.jpg").to_str().unwrap(), "-o", heic.to_str().unwrap()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !ok {
            return; // sin heif-enc no hay HEIC de prueba
        }
        let (out, _, w) = run(&heic, &d, "png", QualityPreset::Lossless).unwrap();
        let i = inspect(&out);
        assert_eq!(i["size"], serde_json::json!([48, 64]), "el giro del HEIC debe quedar aplicado una sola vez");
        assert!(i["orient"].is_null() || i["orient"] == 1, "orientación doblemente aplicada: {}", i["orient"]);
        eprintln!("HEIC → PNG: icc={} make={} avisos={w:?}", i["icc"], i["make"]);
        assert_eq!(i["icc"], icc_sha(fx), "el perfil ICC del HEIC no llegó al PNG");
        assert_eq!(i["make"], "TestMake");
        assert!(w.iter().any(|m| m.contains("HEIC")), "falta el aviso de origen con pérdida: {w:?}");
        let (jpg, _, _) = run(&heic, &d, "jpeg", QualityPreset::Balanced).unwrap();
        assert_eq!(inspect(&jpg)["size"], serde_json::json!([48, 64]));
    }

    #[test]
    fn archivos_dañados_se_clasifican_y_no_dejan_basura() {
        let fx = skip_sin_fixtures!();
        let d = work("dañados");
        let out_dir = d.join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        // 1) contenido que no es una imagen
        let basura = d.join("basura.png");
        std::fs::write(&basura, "esto no es una imagen ".repeat(100)).unwrap();
        // 2) PNG real truncado por la mitad
        let png = std::fs::read(fx.join("noise.png")).unwrap();
        let truncado = d.join("truncado.png");
        std::fs::write(&truncado, &png[..png.len() / 2]).unwrap();
        // 3) JPEG real truncado
        let jpg = std::fs::read(fx.join("smooth.jpg")).unwrap();
        let jtrunc = d.join("truncado.jpg");
        std::fs::write(&jtrunc, &jpg[..jpg.len() / 3]).unwrap();
        for (f, t) in [(&basura, "jpeg"), (&truncado, "webp"), (&jtrunc, "png")] {
            let e = run(f, &out_dir, t, QualityPreset::Lossless).err().unwrap_or_else(|| panic!("{f:?} debía fallar"));
            assert!(matches!(e, ConvertError::Corrupt), "{f:?}: fue {e:?}");
        }
        assert_eq!(std::fs::read_dir(&out_dir).unwrap().count(), 0, "no debe quedar ningún archivo");
    }

    #[test]
    fn extension_mentirosa_se_trata_por_su_contenido() {
        let fx = skip_sin_fixtures!();
        let d = work("mentirosa");
        let falso = d.join("falso.jpg");
        std::fs::copy(fx.join("noise.png"), &falso).unwrap(); // PNG con extensión .jpg
        let (out, mode, _) = run(&falso, &d, "jpeg", QualityPreset::Balanced).unwrap();
        assert_eq!(mode, Mode::Reencoded);
        assert_eq!(&std::fs::read(&out).unwrap()[..3], &[0xFF, 0xD8, 0xFF], "la salida debe ser un JPEG de verdad");
        let (out, mode, _) = run(&falso, &d, "png", QualityPreset::Balanced).unwrap();
        assert_eq!(mode, Mode::Passthrough, "es un PNG: se copia");
        assert_eq!(std::fs::read(&out).unwrap(), std::fs::read(fx.join("noise.png")).unwrap());
    }
}
