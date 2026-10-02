import { Channel, invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import type {
  Conflict, ConvertRequest, JobSpec, MediaItem, ProgressEvent, SystemInfo,
} from "../types";

export const getSystemInfo = () => invoke<SystemInfo>("get_system_info");
export const scanPaths = (paths: string[]) => invoke<MediaItem[]>("scan_paths", { paths });
export const checkConflicts = (jobs: JobSpec[], outputDir: string) =>
  invoke<Conflict[]>("check_conflicts", { jobs, outputDir });
export const cancelConversion = () => invoke<void>("cancel_conversion");

export function startConversion(request: ConvertRequest, onEvent: (e: ProgressEvent) => void) {
  const channel = new Channel<ProgressEvent>();
  channel.onmessage = onEvent;
  return invoke<void>("start_conversion", { request, onEvent: channel });
}

export const openFolder = (path: string) => openPath(path);

export async function pickFiles(): Promise<string[]> {
  const r = await open({ multiple: true, directory: false });
  return Array.isArray(r) ? r : r ? [r] : [];
}
export async function pickFolder(): Promise<string | null> {
  const r = await open({ directory: true, multiple: false });
  return typeof r === "string" ? r : null;
}

/** Suscripción al drag & drop nativo de la ventana (entrega rutas reales). */
export async function listenFileDrop(
  onDrop: (paths: string[]) => void,
  onHover: (hovering: boolean) => void
) {
  return getCurrentWebview().onDragDropEvent((e) => {
    const t = e.payload.type;
    if (t === "enter" || t === "over") onHover(true);
    else if (t === "leave") onHover(false);
    else if (t === "drop") {
      onHover(false);
      onDrop(e.payload.paths);
    }
  });
}
