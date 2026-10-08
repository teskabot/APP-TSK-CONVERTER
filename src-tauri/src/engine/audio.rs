//! Motor de audio. Usa `ffprobe` y `ffmpeg` como sidecar.
//!
//! Flujo: probe → plan (función pura y testeable) → ejecución atómica.
//!
//! Reglas de fidelidad:
//!  1. Si el códec de origen ya es válido en el destino, NO se recodifica:
//!     `FileCopy` (mismo formato, copia byte a byte) o `StreamCopy` (remux).
//!  2. Con recodificación se conserva la profundidad de bits, la tasa de
//!     muestreo, las etiquetas y la carátula siempre que el formato lo permita.
//!     Cuando no se puede (límite del formato), se avisa en `warnings`.
//!  3. La salida se escribe en un `.partial` y se renombra al terminar:
//!     un fallo o cierre de la app nunca deja un archivo a medias.

use super::{ConvertError, Mode, Outcome};
use crate::models::{JobSpec, QualityPreset};
use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;

pub const AUDIO_TARGETS: &[&str] = &["webm", "mp3", "wav", "flac", "aac", "m4a", "ogg"];

// ───────────────────────── Sidecar ─────────────────────────

/// En la app empaquetada, Tauri deja los sidecars junto al ejecutable.
/// En desarrollo o pruebas se cae al PATH.
fn sidecar(name: &str) -> PathBuf {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    let dir = DIR.get_or_init(|| {
        std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf))
    });
    let file = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    if let Some(d) = dir {
        let p = d.join(&file);
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(name)
}

fn run<I, S>(name: &str, args: I) -> Result<Output, ConvertError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new(sidecar(name));
    cmd.args(args).stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.output().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ConvertError::EngineMissing,
        _ => ConvertError::Io(e),
    })
}

fn classify(stderr: &str, reading_input: bool) -> ConvertError {
    let s = stderr.to_lowercase();
    if s.contains("no such file") {
        ConvertError::Missing
    } else if s.contains("unknown encoder") || s.contains("encoder not found") {
        let tail: String = stderr.lines().rev().take(3).collect::<Vec<_>>().join(" | ");
        ConvertError::Failed(tail)
    } else if s.contains("permission denied") {
        if reading_input {
            ConvertError::Protected
        } else {
            ConvertError::Io(std::io::ErrorKind::PermissionDenied.into())
        }
    } else if s.contains("no space left") {
        ConvertError::Io(std::io::Error::new(std::io::ErrorKind::Other, "disco lleno"))
    } else if s.contains("drm") || s.contains("encrypted") {
        ConvertError::Protected
    } else if s.contains("invalid data")
        || s.contains("moov atom not found")
        || s.contains("end of file")
        || s.contains("error while decoding")
        || s.contains("failed to read frame")
        || s.contains("invalid argument")
    {
        ConvertError::Corrupt
    } else {
        let tail: String = stderr.lines().rev().take(3).collect::<Vec<_>>().join(" | ");
        ConvertError::Failed(tail)
    }
}

// ───────────────────────── Probe ─────────────────────────

#[derive(Debug, Deserialize, Default)]
struct Probe {
    #[serde(default)]
    streams: Vec<PStream>,
    #[serde(default)]
    format: PFormat,
}

#[derive(Debug, Deserialize, Default)]
struct PFormat {
    #[serde(default)]
    tags: HashMap<String, String>,
    bit_rate: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PStream {
    index: u32,
    codec_type: Option<String>,
    codec_name: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    sample_fmt: Option<String>,
    bits_per_raw_sample: Option<String>,
    bits_per_sample: Option<u32>,
    bit_rate: Option<String>,
    #[serde(default)]
    disposition: PDisp,
    #[serde(default)]
    tags: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Default)]
struct PDisp {
    #[serde(default)]
    attached_pic: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioInfo {
    pub audio_index: u32,
    pub codec: String,
    pub sample_rate: Option<u32>,
    pub channels: u32,
    pub bit_depth: Option<u32>,
    pub is_float: bool,
    pub bit_rate_kbps: Option<u32>,
    pub cover_index: Option<u32>,
    /// Hay video real (no solo carátula): se está extrayendo audio de un video.
    pub has_video: bool,
    pub has_tags: bool,
}

impl AudioInfo {
    pub fn is_lossless(&self) -> bool {
        matches!(self.codec.as_str(), "flac" | "alac" | "wavpack" | "ape" | "tta")
            || self.codec.starts_with("pcm_")
    }
}

fn num(s: &Option<String>) -> Option<u64> {
    s.as_deref().and_then(|v| v.trim().parse().ok())
}

fn parse_info(p: Probe) -> Option<AudioInfo> {
    let is = |s: &PStream, t: &str| s.codec_type.as_deref() == Some(t);
    let a = p.streams.iter().find(|s| is(s, "audio"))?;
    let cover = p.streams.iter().find(|s| is(s, "video") && s.disposition.attached_pic == 1);
    let has_video = p.streams.iter().any(|s| is(s, "video") && s.disposition.attached_pic != 1);

    let fmt = a.sample_fmt.as_deref().unwrap_or("");
    let base = fmt.trim_end_matches('p'); // fltp → flt, s16p → s16
    let is_float = matches!(base, "flt" | "dbl");
    let bit_depth = num(&a.bits_per_raw_sample)
        .filter(|b| *b > 0)
        .map(|b| b as u32)
        .or(a.bits_per_sample.filter(|b| *b > 0))
        .or(match base {
            "u8" => Some(8),
            "s16" => Some(16),
            "s32" | "flt" => Some(32),
            "s64" | "dbl" => Some(64),
            _ => None,
        });
    let bit_rate = num(&a.bit_rate).or(num(&p.format.bit_rate));

    Some(AudioInfo {
        audio_index: a.index,
        codec: a.codec_name.clone().unwrap_or_default(),
        sample_rate: num(&a.sample_rate).map(|r| r as u32).filter(|r| *r > 0),
        channels: a.channels.unwrap_or(2).max(1),
        bit_depth,
        is_float,
        bit_rate_kbps: bit_rate.map(|b| (b / 1000) as u32),
        cover_index: cover.map(|c| c.index),
        has_video,
        has_tags: !p.format.tags.is_empty() || !a.tags.is_empty(),
    })
}

pub fn probe(path: &Path) -> Result<AudioInfo, ConvertError> {
    let mut args: Vec<OsString> = ["-v", "error", "-print_format", "json", "-show_streams", "-show_format", "-i"]
        .iter()
        .map(OsString::from)
        .collect();
    args.push(path.into());
    let out = run("ffprobe", &args)?;
    if !out.status.success() {
        return Err(classify(&String::from_utf8_lossy(&out.stderr), true));
    }
    let parsed: Probe = serde_json::from_slice(&out.stdout).map_err(|_| ConvertError::Corrupt)?;
    parse_info(parsed).ok_or(ConvertError::NoAudio)
}

// ───────────────────────── Plan (puro, sin E/S) ─────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Mismo formato y códec: copia del archivo tal cual (bit-exact, incluye etiquetas y carátula).
    FileCopy,
    /// Otro contenedor, mismo flujo de audio: remux sin recodificar (bit-exact).
    StreamCopy,
    /// Recodificación con estos argumentos de códec.
    Encode(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub action: Action,
    pub warnings: Vec<String>,
}

const OPUS_RATES: &[u32] = &[8000, 12000, 16000, 24000, 48000];
const MP3_RATES: &[u32] = &[8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000];
const AAC_RATES: &[u32] = &[
    7350, 8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, 64000, 88200, 96000,
];

/// Tasa válida para el formato. Nunca baja la tasa si puede evitarlo (sube a la
/// siguiente compatible); si debe bajar, prefiere la misma familia (44,1k ↔ 48k).
fn pick_rate(src: u32, supported: &[u32]) -> u32 {
    if supported.contains(&src) {
        return src;
    }
    if let Some(r) = supported.iter().copied().filter(|r| *r > src).min() {
        return r;
    }
    let fam = src % 11025 == 0;
    supported
        .iter()
        .copied()
        .filter(|r| (r % 11025 == 0) == fam)
        .max()
        .or_else(|| supported.iter().copied().max())
        .unwrap_or(src)
}

fn kbps(base_stereo: u32, channels: u32) -> u32 {
    (base_stereo * channels.max(1) / 2).max(base_stereo / 2)
}

fn sv(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn pick<T: Copy>(p: QualityPreset, max: T, bal: T, compact: T) -> T {
    match p {
        QualityPreset::Lossless => max,
        QualityPreset::Balanced => bal,
        QualityPreset::Compact => compact,
    }
}

fn compact_kbps(target: &str) -> Option<u32> {
    match target {
        "webm" => Some(64),
        "m4a" | "aac" => Some(112),
        "mp3" => Some(130),
        "ogg" => Some(112),
        _ => None,
    }
}

fn can_copy(info: &AudioInfo, target: &str) -> bool {
    let c = info.codec.as_str();
    match target {
        "webm" | "ogg" => matches!(c, "opus" | "vorbis"),
        "mp3" => c == "mp3",
        "m4a" => matches!(c, "aac" | "alac"),
        "aac" => c == "aac",
        "flac" => c == "flac",
        "wav" => matches!(c, "pcm_s16le" | "pcm_s24le" | "pcm_s32le" | "pcm_f32le" | "pcm_u8"),
        _ => false,
    }
}

fn should_copy(info: &AudioInfo, target: &str, preset: QualityPreset) -> bool {
    if !can_copy(info, target) {
        return false;
    }
    // "Compacta" sobre un origen con pérdida: solo se recodifica si realmente reduce.
    if preset == QualityPreset::Compact && !info.is_lossless() {
        if let (Some(src), Some(c)) = (info.bit_rate_kbps, compact_kbps(target)) {
            return src <= kbps(c, info.channels);
        }
    }
    true
}

fn supports_cover(target: &str) -> bool {
    matches!(target, "mp3" | "m4a" | "flac")
}

fn container_warnings(info: &AudioInfo, target: &str) -> Vec<String> {
    let mut w = Vec::new();
    if info.cover_index.is_some() && !supports_cover(target) {
        w.push(format!("{} no admite carátula: no se incluyó.", target.to_uppercase()));
    }
    if target == "aac" && info.has_tags {
        w.push("AAC sin contenedor no admite etiquetas: no se incluyeron.".into());
    }
    if target == "wav" && info.has_tags {
        w.push("WAV solo guarda etiquetas básicas.".into());
    }
    w
}

fn resample_arg(info: &AudioInfo, supported: &[u32], warnings: &mut Vec<String>) -> Vec<String> {
    let Some(src) = info.sample_rate else { return vec![] };
    let rate = pick_rate(src, supported);
    if rate != src {
        warnings.push(format!(
            "Se ajustó la tasa de muestreo de {src} Hz a {rate} Hz (límite del formato)."
        ));
    }
    sv(&["-ar", &rate.to_string()])
}

fn encode_args(info: &AudioInfo, target: &str, preset: QualityPreset, w: &mut Vec<String>) -> Vec<String> {
    let lossless_src = info.is_lossless();
    let mut a: Vec<String> = Vec::new();
    match target {
        "flac" => {
            let depth = if !lossless_src {
                pick(preset, 24, 16, 16)
            } else if info.is_float {
                24
            } else {
                match info.bit_depth {
                    Some(b) if b <= 16 => 16,
                    _ => 24,
                }
            };
            if lossless_src && (info.is_float || info.bit_depth.map_or(false, |b| b > 24)) {
                w.push("FLAC guarda hasta 24 bits: se ajustó la profundidad.".into());
            }
            if !lossless_src {
                w.push("El original ya tenía pérdida: FLAC no recupera esa calidad.".into());
            }
            a.extend(sv(&["-c:a", "flac", "-compression_level", pick(preset, "5", "5", "8")]));
            if depth <= 16 {
                a.extend(sv(&["-sample_fmt", "s16"]));
            } else {
                a.extend(sv(&["-sample_fmt", "s32", "-bits_per_raw_sample", "24"]));
            }
        }
        "wav" => {
            let codec = if !lossless_src {
                pick(preset, "pcm_s24le", "pcm_s16le", "pcm_s16le")
            } else if info.is_float {
                "pcm_f32le"
            } else {
                match info.bit_depth {
                    Some(8) => "pcm_u8",
                    Some(b) if b <= 16 => "pcm_s16le",
                    Some(b) if b <= 24 => "pcm_s24le",
                    _ => "pcm_s32le",
                }
            };
            if !lossless_src {
                w.push("El original ya tenía pérdida: WAV no recupera esa calidad.".into());
            }
            a.extend(sv(&["-c:a", codec, "-rf64", "auto"]));
        }
        "mp3" => {
            a.extend(sv(&["-c:a", "libmp3lame", "-q:a", pick(preset, "0", "2", "5")]));
            a.extend(resample_arg(info, MP3_RATES, w));
            if info.channels > 2 {
                a.extend(sv(&["-ac", "2"]));
                w.push("MP3 admite 2 canales: se mezcló a estéreo.".into());
            }
        }
        "webm" => {
            let k = kbps(pick(preset, 256, 128, 64), info.channels);
            a.extend(sv(&["-c:a", "libopus", "-b:a", &format!("{k}k"), "-vbr", "on", "-application", "audio"]));
            a.extend(resample_arg(info, OPUS_RATES, w));
        }
        "ogg" => {
            a.extend(sv(&["-c:a", "libvorbis", "-q:a", pick(preset, "10", "6", "3")]));
        }
        "aac" | "m4a" => {
            let k = kbps(pick(preset, 320, 192, 112), info.channels);
            a.extend(sv(&["-c:a", "aac", "-b:a", &format!("{k}k")]));
            a.extend(resample_arg(info, AAC_RATES, w));
        }
        _ => {}
    }
    a
}

pub fn plan(
    info: &AudioInfo,
    src_ext: &str,
    target: &str,
    preset: QualityPreset,
) -> Result<Plan, ConvertError> {
    if !AUDIO_TARGETS.contains(&target) {
        return Err(ConvertError::Unsupported);
    }
    let mut warnings = Vec::new();
    let action = if should_copy(info, target, preset) {
        if src_ext.eq_ignore_ascii_case(target) && !info.has_video {
            Action::FileCopy
        } else {
            Action::StreamCopy
        }
    } else {
        Action::Encode(encode_args(info, target, preset, &mut warnings))
    };
    if action != Action::FileCopy {
        warnings.extend(container_warnings(info, target));
    }
    Ok(Plan { action, warnings })
}

// ───────────────────────── Ejecución ─────────────────────────

fn ffmpeg_args(info: &AudioInfo, plan: &Plan, target: &str, input: &Path, output: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = Vec::new();
    let mut push = |items: &[&str]| a.extend(items.iter().map(OsString::from));
    push(&["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"]);
    a.push(input.into());

    let mut push = |items: &[String]| a.extend(items.iter().map(OsString::from));
    // Solo el audio elegido (así se extrae audio de videos sin arrastrar el video).
    push(&sv(&["-map", &format!("0:{}", info.audio_index)]));
    if let (Some(ci), true) = (info.cover_index, supports_cover(target)) {
        push(&sv(&["-map", &format!("0:{ci}"), "-c:v", "copy", "-disposition:v:0", "attached_pic"]));
    }
    push(&sv(&["-map_metadata", "0"]));
    match &plan.action {
        Action::Encode(args) => push(args),
        _ => push(&sv(&["-c:a", "copy"])),
    }
    match target {
        "mp3" => push(&sv(&["-id3v2_version", "3"])),
        // OJO: NO usar `-movflags +use_metadata_tags`: conserva etiquetas
        // personalizadas pero FFmpeg deja de escribir el átomo `covr` (carátula).
        _ => {}
    }
    a.push(output.into());
    a
}

/// `dir/x.flac` → `dir/.x.partial.flac` (conserva la extensión para que FFmpeg elija el muxer).
pub(crate) fn partial_path(target: &Path) -> PathBuf {
    let stem = target.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = target.extension().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    target.with_file_name(format!(".{stem}.partial.{ext}"))
}

pub fn convert(job: &JobSpec, target: &Path) -> Result<Outcome, ConvertError> {
    let src = &job.item.path;
    let info = probe(src)?;
    let plan = plan(&info, &job.item.ext, &job.target_format, job.preset)?;
    let tmp = partial_path(target);

    let result: Result<(), ConvertError> = match &plan.action {
        Action::FileCopy => std::fs::copy(src, &tmp).map(|_| ()).map_err(Into::into),
        _ => {
            let args = ffmpeg_args(&info, &plan, &job.target_format, src, &tmp);
            run("ffmpeg", &args).and_then(|out| {
                if out.status.success() {
                    Ok(())
                } else {
                    Err(classify(&String::from_utf8_lossy(&out.stderr), false))
                }
            })
        }
    }
    .and_then(|_| std::fs::rename(&tmp, target).map_err(Into::into));

    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }

    let out_len = std::fs::metadata(target)?.len() as i64;
    Ok(Outcome::Converted {
        bytes_saved: job.item.size_bytes as i64 - out_len,
        mode: if matches!(plan.action, Action::Encode(_)) { Mode::Reencoded } else { Mode::Passthrough },
        warnings: plan.warnings,
    })
}

// ───────────────────────── Pruebas ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ConflictPolicy, MediaItem, MediaKind};

    fn info(codec: &str) -> AudioInfo {
        AudioInfo {
            audio_index: 0, codec: codec.into(), sample_rate: Some(44100), channels: 2,
            bit_depth: None, is_float: false, bit_rate_kbps: None, cover_index: None,
            has_video: false, has_tags: false,
        }
    }

    #[test]
    fn mismo_formato_es_copia_directa() {
        let p = plan(&info("opus"), "webm", "webm", QualityPreset::Lossless).unwrap();
        assert_eq!(p.action, Action::FileCopy);
    }

    #[test]
    fn otro_contenedor_mismo_codec_es_remux() {
        let p = plan(&info("aac"), "mp4", "m4a", QualityPreset::Lossless).unwrap();
        assert_eq!(p.action, Action::StreamCopy);
    }

    #[test]
    fn webm_con_video_nunca_se_copia_entero() {
        let mut i = info("opus");
        i.has_video = true;
        let p = plan(&i, "webm", "webm", QualityPreset::Lossless).unwrap();
        assert_eq!(p.action, Action::StreamCopy);
    }

    #[test]
    fn compacta_recodifica_solo_si_reduce() {
        let mut i = info("mp3");
        i.bit_rate_kbps = Some(320);
        assert!(matches!(plan(&i, "mp3", "mp3", QualityPreset::Compact).unwrap().action, Action::Encode(_)));
        i.bit_rate_kbps = Some(96);
        assert_eq!(plan(&i, "mp3", "mp3", QualityPreset::Compact).unwrap().action, Action::FileCopy);
    }

    #[test]
    fn tasas_de_muestreo() {
        assert_eq!(pick_rate(44100, OPUS_RATES), 48000); // sube, no pierde agudos
        assert_eq!(pick_rate(96000, OPUS_RATES), 48000);
        assert_eq!(pick_rate(88200, MP3_RATES), 44100); // misma familia
        assert_eq!(pick_rate(44100, MP3_RATES), 44100);
    }

    #[test]
    fn flac_24_bits_desde_wav_24() {
        let mut i = info("pcm_s24le");
        i.bit_depth = Some(24);
        let Action::Encode(a) = plan(&i, "wav", "flac", QualityPreset::Lossless).unwrap().action else { panic!() };
        assert!(a.windows(2).any(|w| w == ["-bits_per_raw_sample", "24"]));
    }

    #[test]
    fn carátula_se_avisa_si_el_formato_no_la_admite() {
        let mut i = info("mp3");
        i.cover_index = Some(1);
        let p = plan(&i, "mp3", "ogg", QualityPreset::Lossless).unwrap();
        assert!(p.warnings.iter().any(|w| w.contains("carátula")));
        let p = plan(&i, "mp3", "m4a", QualityPreset::Lossless).unwrap();
        assert!(!p.warnings.iter().any(|w| w.contains("carátula")));
    }

    // ── Integración con FFmpeg real (se omite si no está instalado) ──

    fn have_ffmpeg() -> bool {
        run("ffmpeg", ["-version"]).map(|o| o.status.success()).unwrap_or(false)
    }

    fn work_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("conversor-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn ff(args: &[&str]) {
        let o = run("ffmpeg", ["-y", "-hide_banner", "-loglevel", "error"].iter().chain(args.iter())).unwrap();
        assert!(o.status.success(), "ffmpeg: {}", String::from_utf8_lossy(&o.stderr));
    }

    /// MD5 del audio decodificado a PCM 32 bits: prueba de igualdad bit a bit.
    fn audio_md5(p: &Path) -> String {
        let o = run("ffmpeg", [
            OsString::from("-v"), "error".into(), "-i".into(), p.into(),
            "-map".into(), "0:a:0".into(), "-c:a".into(), "pcm_s32le".into(),
            "-f".into(), "md5".into(), "-".into(),
        ]).unwrap();
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    }

    fn job(path: &Path, target: &str, preset: QualityPreset) -> JobSpec {
        JobSpec {
            item: MediaItem {
                id: "t".into(), path: path.into(),
                name: path.file_name().unwrap().to_string_lossy().into(),
                ext: path.extension().unwrap().to_string_lossy().into(),
                kind: MediaKind::Audio, is_video: false,
                size_bytes: std::fs::metadata(path).unwrap().len(),
            },
            target_format: target.into(),
            preset,
        }
    }

    fn convert_to(src: &Path, dir: &Path, target: &str, preset: QualityPreset) -> (PathBuf, Mode, Vec<String>) {
        let j = job(src, target, preset);
        let out = crate::engine::resolve_target(&j, dir, ConflictPolicy::Rename).unwrap();
        match convert(&j, &out).expect("conversión") {
            Outcome::Converted { mode, warnings, .. } => (out, mode, warnings),
            Outcome::Skipped => panic!("omitido"),
        }
    }

    #[test]
    fn wav24_a_flac_a_wav_es_bit_exact() {
        if !have_ffmpeg() { return; }
        let d = work_dir("lossless");
        let wav = d.join("origen.wav");
        ff(&["-f", "lavfi", "-i", "anoisesrc=d=1:r=96000:a=0.3", "-ac", "2", "-c:a", "pcm_s24le",
             "-metadata", "title=Prueba", "-metadata", "artist=Yo", wav.to_str().unwrap()]);
        let md5_src = audio_md5(&wav);

        let out_dir = d.join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        let (flac, mode, _) = convert_to(&wav, &out_dir, "flac", QualityPreset::Lossless);
        assert_eq!(mode, Mode::Reencoded);
        let fi = probe(&flac).unwrap();
        assert_eq!(fi.bit_depth, Some(24));
        assert_eq!(fi.sample_rate, Some(96000));
        assert!(fi.has_tags, "las etiquetas deben conservarse");
        assert_eq!(audio_md5(&flac), md5_src, "FLAC no es idéntico al WAV");

        // Y de vuelta a WAV: sigue 24 bits y sigue idéntico.
        let back_dir = d.join("back");
        std::fs::create_dir_all(&back_dir).unwrap();
        let (wav2, _, _) = convert_to(&flac, &back_dir, "wav", QualityPreset::Lossless);
        assert_eq!(probe(&wav2).unwrap().codec, "pcm_s24le");
        assert_eq!(audio_md5(&wav2), md5_src);
    }

    #[test]
    fn mismo_formato_copia_byte_a_byte() {
        if !have_ffmpeg() { return; }
        let d = work_dir("copia");
        let src = d.join("a.webm");
        ff(&["-f", "lavfi", "-i", "sine=d=1", "-c:a", "libopus", src.to_str().unwrap()]);
        let out_dir = d.join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        let (out, mode, _) = convert_to(&src, &out_dir, "webm", QualityPreset::Lossless);
        assert_eq!(mode, Mode::Passthrough);
        assert_eq!(std::fs::read(&src).unwrap(), std::fs::read(&out).unwrap());
    }

    #[test]
    fn extraer_audio_de_video_sin_recodificar() {
        if !have_ffmpeg() { return; }
        let d = work_dir("video");
        let mp4 = d.join("v.mp4");
        ff(&["-f", "lavfi", "-i", "testsrc=d=1:s=64x64:r=10", "-f", "lavfi", "-i", "sine=d=1",
             "-c:v", "mpeg4", "-c:a", "aac", "-shortest", mp4.to_str().unwrap()]);
        let out_dir = d.join("out");
        std::fs::create_dir_all(&out_dir).unwrap();

        let (m4a, mode, _) = convert_to(&mp4, &out_dir, "m4a", QualityPreset::Lossless);
        assert_eq!(mode, Mode::Passthrough);
        let i = probe(&m4a).unwrap();
        assert!(!i.has_video && i.codec == "aac");
        assert_eq!(audio_md5(&m4a), audio_md5(&mp4), "el audio debe ser idéntico al del video");

        let (flac, _, _) = convert_to(&mp4, &out_dir, "flac", QualityPreset::Lossless);
        assert_eq!(probe(&flac).unwrap().codec, "flac");
    }

    #[test]
    fn caratula_y_etiquetas_sobreviven_mp3_a_m4a() {
        if !have_ffmpeg() { return; }
        let d = work_dir("cover");
        let jpg = d.join("c.jpg");
        ff(&["-f", "lavfi", "-i", "color=c=red:s=64x64", "-frames:v", "1", jpg.to_str().unwrap()]);
        let mp3 = d.join("a.mp3");
        ff(&["-f", "lavfi", "-i", "sine=d=1", "-i", jpg.to_str().unwrap(), "-map", "0:a", "-map", "1:v",
             "-c:a", "libmp3lame", "-c:v", "copy", "-id3v2_version", "3", "-disposition:v", "attached_pic",
             "-metadata", "title=Canción", mp3.to_str().unwrap()]);
        assert!(probe(&mp3).unwrap().cover_index.is_some(), "el MP3 de prueba debe tener carátula");

        let out_dir = d.join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        let (m4a, _, w) = convert_to(&mp3, &out_dir, "m4a", QualityPreset::Balanced);
        let i = probe(&m4a).unwrap();
        assert!(i.cover_index.is_some(), "se perdió la carátula");
        assert!(i.has_tags, "se perdieron las etiquetas");
        assert!(w.is_empty());

        let (ogg, _, w) = convert_to(&mp3, &out_dir, "ogg", QualityPreset::Balanced);
        assert!(probe(&ogg).unwrap().cover_index.is_none());
        assert!(w.iter().any(|m| m.contains("carátula")));
    }

    #[test]
    fn tasa_96k_a_opus_avisa_y_sale_a_48k() {
        if !have_ffmpeg() { return; }
        let d = work_dir("opus");
        let wav = d.join("a.wav");
        ff(&["-f", "lavfi", "-i", "sine=d=1:r=96000", "-c:a", "pcm_s24le", wav.to_str().unwrap()]);
        let out_dir = d.join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        let (webm, _, w) = convert_to(&wav, &out_dir, "webm", QualityPreset::Lossless);
        assert_eq!(probe(&webm).unwrap().sample_rate, Some(48000));
        assert!(w.iter().any(|m| m.contains("tasa de muestreo")));
    }

    #[test]
    fn archivo_dañado_se_clasifica_y_no_deja_basura() {
        if !have_ffmpeg() { return; }
        let d = work_dir("bad");
        let bad = d.join("roto.mp3");
        std::fs::write(&bad, "esto no es audio, ni de lejos. ".repeat(200)).unwrap();
        let out_dir = d.join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        let j = job(&bad, "flac", QualityPreset::Lossless);
        let out = out_dir.join("roto.flac");
        let err = convert(&j, &out).err().expect("debía fallar");
        assert!(matches!(err, ConvertError::Corrupt), "fue {err:?}");
        assert_eq!(std::fs::read_dir(&out_dir).unwrap().count(), 0, "no debe quedar ningún archivo");
    }

    #[test]
    fn dos_origenes_con_el_mismo_nombre_no_se_pisan() {
        let d = work_dir("dup");
        let mk = |sub: &str, ext: &str| {
            let p = d.join(sub);
            std::fs::create_dir_all(&p).unwrap();
            let f = p.join(format!("x.{ext}"));
            std::fs::write(&f, b"x").unwrap();
            job(&f, "flac", QualityPreset::Lossless)
        };
        crate::engine::begin_batch();
        let out = d.join("out");
        let a = crate::engine::resolve_target(&mk("a", "mp3"), &out, ConflictPolicy::Overwrite).unwrap();
        let b = crate::engine::resolve_target(&mk("b", "wav"), &out, ConflictPolicy::Overwrite).unwrap();
        assert_ne!(a, b);
        assert!(b.to_string_lossy().ends_with("x_1.flac"));
    }
}
