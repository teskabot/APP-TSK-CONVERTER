import { useState } from "react";
import { AlertTriangle, FolderOpen, Info, RefreshCw } from "lucide-react";
import { cancelConversion, openFolder } from "../lib/ipc";
import { formatBytes, formatDuration } from "../lib/formats";
import { useBatch } from "../store/useBatch";
import type { Notice } from "../types";

export function ProgressPanel() {
  const { phase, progress, result, retryFailed, reset } = useBatch();
  const [actionError, setActionError] = useState<string | null>(null);

  const openPath = async (path: string) => {
    setActionError(null);
    try {
      await openFolder(path);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : String(error));
    }
  };

  if (phase === "running") {
    const pct = progress ? Math.round((progress.done / progress.total) * 100) : 0;
    return (
      <div className="flex h-full items-center justify-center p-8">
        <div className="w-full max-w-xl space-y-5">
          <div className="flex items-baseline justify-between">
            <h1 className="text-3xl font-semibold tracking-tight">Convirtiendo</h1>
            <span className="text-3xl font-semibold tabular-nums">{pct}%</span>
          </div>
          <div className="h-2 overflow-hidden rounded-full bg-line" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
            <div className="h-full bg-accent transition-[width] duration-200" style={{ width: `${pct}%` }} />
          </div>
          <p className="text-sm text-muted">
            {progress ? `${progress.done} de ${progress.total} archivos` : "Preparando"}
            {progress?.etaSeconds != null && ` · quedan ${formatDuration(progress.etaSeconds)}`}
            {progress && ` · ${progress.activeThreads} en paralelo`}
          </p>
          <button onClick={cancelConversion} className="text-sm text-muted underline hover:text-ink">
            Cancelar
          </button>
        </div>
      </div>
    );
  }

  if (!result) return null;
  const problems = result.failures.length;

  return (
    <div className="flex h-full overflow-y-auto p-8">
      <div className="m-auto w-full max-w-xl space-y-6">
        <h1 className="text-3xl font-semibold tracking-tight">
          {problems === 0 ? "Listo" : "Terminó con avisos"}
        </h1>

        <dl className="grid grid-cols-3 gap-4 text-sm">
          <Stat label="Convertidos" value={String(result.succeeded)} />
          <Stat label="Tiempo" value={formatDuration(result.elapsedMs / 1000)} />
          <Stat label={result.bytesSaved >= 0 ? "Espacio ahorrado" : "Espacio extra"} value={formatBytes(Math.abs(result.bytesSaved))} />
        </dl>
        <p className="break-all text-sm text-muted">Destino: {result.outputDir}</p>

        {result.passthrough > 0 && (
          <p className="text-sm text-muted">
            {result.passthrough === 1
              ? "1 archivo se copió sin recodificar"
              : `${result.passthrough} archivos se copiaron sin recodificar`}
            : el contenido quedó idéntico al original.
          </p>
        )}

        {problems > 0 && (
          <section className="rounded-lg border border-warn/40 bg-sheet">
            <h2 className="flex items-center gap-2 px-4 pt-4 font-semibold text-warn">
              <AlertTriangle size={16} /> Archivos con problemas ({problems})
            </h2>
            <ul className="divide-y divide-line px-4 py-2 text-sm">
              {result.failures.map((f) => (
                <li key={f.itemId} className="flex justify-between gap-4 py-2">
                  <span className="truncate">{f.name}</span>
                  <span className="max-w-[60%] break-all text-right text-muted">{f.reason}</span>
                </li>
              ))}
            </ul>
            <div className="border-t border-line p-3">
              <button onClick={retryFailed} className="inline-flex items-center gap-2 rounded-md border border-line px-3 py-1.5 text-sm hover:border-ink">
                <RefreshCw size={14} /> Reintentar fallidos
              </button>
              <button onClick={() => openPath(result.logPath)} className="ml-3 inline-flex items-center gap-2 rounded-md border border-line px-3 py-1.5 text-sm hover:border-ink">
                Abrir log técnico
              </button>
            </div>
          </section>
        )}

        {result.notices.length > 0 && <NoticeList notices={result.notices} />}

        <div className="flex items-center gap-4">
          <button
            onClick={() => openPath(result.outputDir)}
            className="inline-flex items-center gap-2 rounded-md bg-accent px-6 py-3 text-base font-medium text-sheet hover:brightness-110"
          >
            <FolderOpen size={18} /> Abrir carpeta de destino
          </button>
          <button onClick={reset} className="text-sm text-muted underline hover:text-ink">
            Convertir más archivos
          </button>
        </div>
        {actionError && <p role="alert" className="text-sm text-warn">No se pudo abrir la ruta: {actionError}</p>}
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dd className="text-2xl font-semibold tabular-nums">{value}</dd>
      <dt className="text-muted">{label}</dt>
    </div>
  );
}

/** Agrupa por mensaje: 200 archivos con el mismo ajuste son 1 aviso, no 200. */
function groupNotices(notices: Notice[]) {
  const map = new Map<string, string[]>();
  for (const n of notices) {
    const names = map.get(n.message) ?? [];
    names.push(n.name);
    map.set(n.message, names);
  }
  return [...map]
    .map(([message, names]) => ({ message, names }))
    .sort((a, b) => b.names.length - a.names.length);
}

function NoticeList({ notices }: { notices: Notice[] }) {
  const groups = groupNotices(notices);
  return (
    <section className="rounded-lg border border-line bg-sheet" aria-label="Avisos">
      <h2 className="flex items-center gap-2 px-4 pt-4 font-semibold">
        <Info size={16} className="text-accent" aria-hidden /> Ajustes aplicados
      </h2>
      <p className="px-4 pt-1 text-sm text-muted">
        Estos archivos se convirtieron bien, pero el formato de destino exigió un cambio.
      </p>
      <ul className="divide-y divide-line px-4 py-2">
        {groups.map((g) => (
          <li key={g.message} className="py-2 text-sm">
            <details>
              <summary className="cursor-pointer list-none">
                <span>{g.message}</span>{" "}
                <span className="text-muted">
                  ({g.names.length} {g.names.length === 1 ? "archivo" : "archivos"})
                </span>
              </summary>
              <ul className="mt-2 max-h-40 space-y-0.5 overflow-y-auto pl-3 text-muted">
                {g.names.map((n, i) => (
                  <li key={i} className="truncate">{n}</li>
                ))}
              </ul>
            </details>
          </li>
        ))}
      </ul>
    </section>
  );
}
