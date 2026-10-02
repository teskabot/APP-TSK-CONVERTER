import { FolderOpen, FilePlus2 } from "lucide-react";
import { pickFiles, pickFolder } from "../lib/ipc";
import { SUPPORTED_SUMMARY } from "../lib/formats";
import { useBatch } from "../store/useBatch";

export function DropZone({ hovering }: { hovering: boolean }) {
  const addPaths = useBatch((s) => s.addPaths);

  return (
    <div className="flex h-full items-center justify-center p-8">
      <div
        data-hover={hovering}
        className="frame flex w-full max-w-2xl flex-col items-center gap-7 px-10 py-20 text-center"
        role="region"
        aria-label="Zona para soltar archivos"
      >
        <span className="mark tl" /><span className="mark tr" />
        <span className="mark bl" /><span className="mark br" />

        <div className="space-y-3">
          <h1 className="text-4xl font-semibold tracking-tight">
            {hovering ? "Suéltalos aquí" : "Suelta tus archivos o carpetas"}
          </h1>
          <p className="mx-auto max-w-md text-base text-muted">
            Convierte imágenes y audio por lotes, sin perder calidad. Todo ocurre en tu equipo.
          </p>
        </div>

        <div className="flex flex-wrap justify-center gap-3">
          <button
            onClick={async () => addPaths(await pickFiles())}
            className="inline-flex items-center gap-2 rounded-md bg-ink px-4 py-2.5 text-sm font-medium text-sheet transition hover:bg-accent"
          >
            <FilePlus2 size={16} /> Elegir archivos
          </button>
          <button
            onClick={async () => {
              const dir = await pickFolder();
              if (dir) addPaths([dir]);
            }}
            className="inline-flex items-center gap-2 rounded-md border border-line px-4 py-2.5 text-sm font-medium transition hover:border-ink"
          >
            <FolderOpen size={16} /> Elegir carpeta
          </button>
        </div>

        <dl className="space-y-1 text-sm text-muted">
          <div><dt className="inline font-medium text-ink">Imágenes: </dt><dd className="inline">{SUPPORTED_SUMMARY.image}</dd></div>
          <div><dt className="inline font-medium text-ink">Audio: </dt><dd className="inline">{SUPPORTED_SUMMARY.audio}</dd></div>
        </dl>
      </div>
    </div>
  );
}
