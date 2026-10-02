// Espejo de src-tauri/src/models.rs (serde camelCase).
export type MediaKind = "image" | "audio";
export type QualityPreset = "lossless" | "balanced" | "compact";
export type ConflictPolicy = "rename" | "overwrite" | "skip";
export type ThreadProfile = "efficient" | "balanced" | "turbo";

export interface MediaItem {
  id: string;
  path: string;
  name: string;
  ext: string;
  kind: MediaKind;
  isVideo: boolean;
  sizeBytes: number;
}

export interface GroupSettings {
  format: string;
  preset: QualityPreset;
}

export interface JobSpec {
  item: MediaItem;
  targetFormat: string;
  preset: QualityPreset;
}

export interface ConvertRequest {
  jobs: JobSpec[];
  outputDir: string;
  conflictPolicy: ConflictPolicy;
  threadProfile: ThreadProfile;
}

export interface SystemInfo {
  cpuCores: number;
  defaultThreads: number;
  defaultOutputDir: string;
}

export interface Conflict {
  itemId: string;
  existingPath: string;
}

export interface Failure {
  itemId: string;
  name: string;
  reason: string;
}

export interface Notice {
  itemId: string;
  name: string;
  message: string;
}

export type ProgressEvent =
  | { event: "started"; data: { total: number; threads: number } }
  | {
      event: "progress";
      data: { done: number; total: number; activeThreads: number; etaSeconds: number | null };
    }
  | {
      event: "finished";
      data: {
        succeeded: number;
        skipped: number;
        failures: Failure[];
        notices: Notice[];
        passthrough: number;
        elapsedMs: number;
        bytesSaved: number;
        outputDir: string;
        logPath: string;
      };
    };
