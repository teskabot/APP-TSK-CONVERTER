import { useState } from "react";
import { AudioLines, ChevronDown, Image as ImageIcon, X } from "lucide-react";
import { OUTPUT_FORMATS, PRESETS, formatBytes, isLossy } from "../lib/formats";
import { useBatch } from "../store/useBatch";
import type { MediaItem, MediaKind } from "../types";

const COPY = {
  image: { title: "Imágenes", icon: ImageIcon, hint: "Se conservan el perfil de color, los metadatos y la transparencia." },
  audio: { title: "Audio", icon: AudioLines, hint: "Se conservan la tasa de muestreo, las etiquetas y la carátula." },
} as const;

export function GroupCard({ kind, items }: { kind: MediaKind; items: MediaItem[] }) {
  const { settings, setGroup, overrides, setOverride, removeItem } = useBatch();
  const [open, setOpen] = useState(false);
  const s = settings[kind];
  const { title, icon: Icon, hint } = COPY[kind];
  const total = items.reduce((n, i) => n + i.sizeBytes, 0);
  const lossyTarget = isLossy(kind, s.format);

  return (
    <section className="rounded-lg border border-line bg-sheet">
      <header className="flex items-baseline justify-between gap-4 px-5 pt-5">
        <h2 className="flex items-center gap-2 text-lg font-semibold">
          <Icon size={18} className="text-accent" aria-hidden /> {title}
        </h2>
        <p className="text-sm text-muted">
          {items.length} {items.length === 1 ? "archivo" : "archivos"}, {formatBytes(total)}
        </p>
      </header>

      <div className="grid gap-5 px-5 py-5 sm:grid-cols-[minmax(0,12rem)_1fr]">
        <label className="flex flex-col gap-1.5 text-sm">
          <span className="font-medium">Convertir a</span>
          <select
            value={s.format}
            onChange={(e) => setGroup(kind, { format: e.target.value })}
            className="rounded-md border border-line bg-paper px-3 py-2 text-base"
          >
            {OUTPUT_FORMATS[kind].map((f) => (
              <option key={f.value} value={f.value}>{f.label}</option>
            ))}
          </select>
        </label>

        <fieldset className="flex flex-col gap-1.5 text-sm">
          <legend className="mb-1.5 font-medium">Calidad</legend>
          <div className="inline-flex w-full overflow-hidden rounded-md border border-line bg-paper">
            {PRESETS.map((p) => (
              <label
                key={p.value}
                className="flex-1 cursor-pointer px-3 py-2 text-center transition has-[:checked]:bg-accent has-[:checked]:text-sheet has-[:focus-visible]:outline has-[:focus-visible]:outline-2 has-[:focus-visible]:-outline-offset-2"
              >
                <input
                  type="radio"
                  name={`preset-${kind}`}
                  className="sr-only"
                  checked={s.preset === p.value}
                  onChange={() => setGroup(kind, { preset: p.value })}
                />
                {p.label}
              </label>
            ))}
          </div>
        </fieldset>

        <p className="text-sm text-muted sm:col-span-2">
          {lossyTarget
            ? `${s.format.toUpperCase()} siempre comprime con pérdida. “Máxima” usa la mejor calidad posible.`
            : hint}
        </p>
      </div>

      <button
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
        className="flex w-full items-center justify-between border-t border-line px-5 py-3 text-sm font-medium hover:bg-paper"
      >
        Ver y ajustar archivos
        <ChevronDown size={16} className={`transition-transform ${open ? "rotate-180" : ""}`} />
      </button>

      {open && (
        <ul className="max-h-64 divide-y divide-line overflow-y-auto border-t border-line">
          {items.map((it) => {
            const o = overrides[it.id];
            return (
              <li key={it.id} className="flex items-center gap-3 px-5 py-2 text-sm">
                <span className="min-w-0 flex-1 truncate" title={it.path}>
                  {it.name}
                  {it.isVideo && <span className="ml-2 text-muted">(extraer audio)</span>}
                </span>
                <span className="w-16 text-right text-muted">{formatBytes(it.sizeBytes)}</span>
                <select
                  aria-label={`Formato de ${it.name}`}
                  value={o?.format ?? s.format}
                  onChange={(e) =>
                    setOverride(it.id, e.target.value === s.format ? null : { format: e.target.value })
                  }
                  className={`rounded border bg-paper px-2 py-1 ${o?.format ? "border-accent" : "border-line"}`}
                >
                  {OUTPUT_FORMATS[kind].map((f) => (
                    <option key={f.value} value={f.value}>{f.label}</option>
                  ))}
                </select>
                <button
                  onClick={() => removeItem(it.id)}
                  aria-label={`Quitar ${it.name}`}
                  className="rounded p-1 text-muted hover:bg-line hover:text-ink"
                >
                  <X size={14} />
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
