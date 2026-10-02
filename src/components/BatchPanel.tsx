import { useEffect } from "react";
import { FolderOpen, Plus, RotateCcw } from "lucide-react";
import { pickFiles, pickFolder } from "../lib/ipc";
import { useBatch } from "../store/useBatch";
import { GroupCard } from "./GroupCard";
import type { ConflictPolicy } from "../types";

const POLICIES: { value: ConflictPolicy; label: string }[] = [
  { value: "rename", label: "Renombrar" },
  { value: "skip", label: "Omitir" },
  { value: "overwrite", label: "Sobrescribir" },
];

export function BatchPanel() {
  const s = useBatch();
  const images = s.items.filter((i) => i.kind === "image");
  const audio = s.items.filter((i) => i.kind === "audio");

  // Aviso previo: ¿ya existen archivos con el mismo nombre en el destino?
  useEffect(() => {
    s.refreshConflicts();
  }, [s.items, s.settings, s.overrides, s.outputDir]);

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 space-y-5 overflow-y-auto p-6">
        <div className="flex items-center justify-between">
          <h1 className="text-2xl font-semibold tracking-tight">
            {s.items.length} {s.items.length === 1 ? "archivo listo" : "archivos listos"}
          </h1>
          <div className="flex gap-2 text-sm">
            <button
              onClick={async () => s.addPaths(await pickFiles())}
              className="inline-flex items-center gap-1.5 rounded-md border border-line px-3 py-1.5 hover:border-ink"
            >
              <Plus size={14} /> Añadir
            </button>
            <button
              onClick={s.reset}
              className="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-muted hover:text-ink"
            >
              <RotateCcw size={14} /> Empezar de nuevo
            </button>
          </div>
        </div>

        {images.length > 0 && <GroupCard kind="image" items={images} />}
        {audio.length > 0 && <GroupCard kind="audio" items={audio} />}
      </div>

      <footer className="space-y-3 border-t border-line bg-sheet px-6 py-4">
        {s.conflicts.length > 0 && (
          <div className="flex flex-wrap items-center justify-between gap-3 rounded-md bg-accent-soft px-4 py-2.5 text-sm">
            <p>
              {s.conflicts.length === 1
                ? "1 archivo ya existe en el destino."
                : `${s.conflicts.length} archivos ya existen en el destino.`}
            </p>
            <div role="radiogroup" aria-label="Si el archivo ya existe" className="inline-flex overflow-hidden rounded border border-accent/40">
              {POLICIES.map((p) => (
                <button
                  key={p.value}
                  role="radio"
                  aria-checked={s.conflictPolicy === p.value}
                  onClick={() => s.setConflictPolicy(p.value)}
                  className={`px-3 py-1 ${s.conflictPolicy === p.value ? "bg-accent text-sheet" : "hover:bg-sheet"}`}
                >
                  {p.label}
                </button>
              ))}
            </div>
          </div>
        )}

        <div className="flex items-center justify-between gap-4">
          <button
            onClick={async () => {
              const dir = await pickFolder();
              if (dir) s.setOutputDir(dir, false);
            }}
            className="inline-flex min-w-0 items-center gap-2 text-sm text-muted hover:text-ink"
            title="Cambiar carpeta de destino"
          >
            <FolderOpen size={16} className="shrink-0" />
            <span className="truncate">Guardar en {s.outputDir}</span>
          </button>
          <button
            onClick={s.start}
            disabled={!s.outputDir || s.items.length === 0}
            className="rounded-md bg-accent px-6 py-2.5 text-base font-medium text-sheet transition hover:brightness-110 disabled:opacity-40"
          >
            Convertir {s.items.length} {s.items.length === 1 ? "archivo" : "archivos"}
          </button>
        </div>
      </footer>
    </div>
  );
}
