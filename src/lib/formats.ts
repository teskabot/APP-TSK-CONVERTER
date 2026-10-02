import type { MediaKind, QualityPreset } from "../types";

export interface FormatOption {
  value: string;
  label: string;
  /** true si el formato siempre comprime con pérdida (MP3, JPEG...). */
  lossy?: boolean;
}

export const OUTPUT_FORMATS: Record<MediaKind, FormatOption[]> = {
  image: [
    { value: "png", label: "PNG" },
    { value: "webp", label: "WEBP" },
    { value: "jpeg", label: "JPEG", lossy: true },
    { value: "tiff", label: "TIFF" },
    { value: "heic", label: "HEIC", lossy: true },
  ],
  audio: [
    { value: "webm", label: "WEBM (Opus)", lossy: true },
    { value: "flac", label: "FLAC" },
    { value: "wav", label: "WAV" },
    { value: "mp3", label: "MP3", lossy: true },
    { value: "aac", label: "AAC", lossy: true },
    { value: "m4a", label: "M4A", lossy: true },
    { value: "ogg", label: "OGG", lossy: true },
  ],
};

export const DEFAULT_FORMAT: Record<MediaKind, string> = { image: "png", audio: "flac" };

export const PRESETS: { value: QualityPreset; label: string }[] = [
  { value: "lossless", label: "Máxima" },
  { value: "balanced", label: "Balanceada" },
  { value: "compact", label: "Compacta" },
];

export const SUPPORTED_SUMMARY = {
  image: "PNG, WEBP, JPEG, HEIC, TIFF, SVG, RAW",
  audio: "WEBM, MP3, WAV, FLAC, AAC, M4A, OGG y audio de video",
};

export function isLossy(kind: MediaKind, format: string) {
  return !!OUTPUT_FORMATS[kind].find((f) => f.value === format)?.lossy;
}

export function formatBytes(n: number) {
  const abs = Math.abs(n);
  if (abs < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = abs / 1024, i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return `${n < 0 ? "-" : ""}${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
}

export function formatDuration(totalSeconds: number) {
  const s = Math.max(0, Math.round(totalSeconds));
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  return `${m} min ${String(s % 60).padStart(2, "0")} s`;
}
