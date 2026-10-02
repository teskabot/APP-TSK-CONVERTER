import { create } from "zustand";
import * as ipc from "../lib/ipc";
import { DEFAULT_FORMAT } from "../lib/formats";
import type {
  Conflict, ConflictPolicy, Failure, GroupSettings, JobSpec, MediaItem,
  MediaKind, Notice, ThreadProfile,
} from "../types";

type Phase = "idle" | "ready" | "running" | "done";

interface Progress { done: number; total: number; activeThreads: number; etaSeconds: number | null }
interface Result {
  succeeded: number; skipped: number; failures: Failure[];
  notices: Notice[]; passthrough: number;
  elapsedMs: number; bytesSaved: number; outputDir: string; logPath: string;
}

interface BatchState {
  phase: Phase;
  items: MediaItem[];
  settings: Record<MediaKind, GroupSettings>;
  overrides: Record<string, Partial<GroupSettings>>;
  outputDir: string | null;
  conflicts: Conflict[];
  conflictPolicy: ConflictPolicy;
  threadProfile: ThreadProfile;
  progress: Progress | null;
  result: Result | null;

  init: () => Promise<void>;
  addPaths: (paths: string[]) => Promise<void>;
  removeItem: (id: string) => void;
  setGroup: (kind: MediaKind, patch: Partial<GroupSettings>) => void;
  setOverride: (id: string, patch: Partial<GroupSettings> | null) => void;
  setOutputDir: (dir: string, remember: boolean) => void;
  setConflictPolicy: (p: ConflictPolicy) => void;
  refreshConflicts: () => Promise<void>;
  jobs: () => JobSpec[];
  start: () => Promise<void>;
  retryFailed: () => Promise<void>;
  reset: () => void;
}

const DIR_KEY = "conversor.outputDir";
let conflictCheckId = 0;

export const useBatch = create<BatchState>((set, get) => ({
  phase: "idle",
  items: [],
  settings: {
    image: { format: DEFAULT_FORMAT.image, preset: "lossless" },
    audio: { format: DEFAULT_FORMAT.audio, preset: "lossless" },
  },
  overrides: {},
  outputDir: null,
  conflicts: [],
  conflictPolicy: "rename",
  threadProfile: "balanced",
  progress: null,
  result: null,

  init: async () => {
    const info = await ipc.getSystemInfo();
    const saved = localStorage.getItem(DIR_KEY);
    set({ outputDir: saved ?? info.defaultOutputDir });
  },

  addPaths: async (paths) => {
    if (get().phase === "running") return;
    const found = await ipc.scanPaths(paths);
    if (found.length === 0) return;
    const known = new Set(get().items.map((i) => i.path));
    const fresh = found.filter((i) => !known.has(i.path));
    set((s) => ({ items: [...s.items, ...fresh], phase: "ready" }));
  },

  removeItem: (id) =>
    set((s) => {
      const items = s.items.filter((i) => i.id !== id);
      return { items, phase: items.length ? s.phase : "idle" };
    }),

  setGroup: (kind, patch) =>
    set((s) => ({ settings: { ...s.settings, [kind]: { ...s.settings[kind], ...patch } } })),

  setOverride: (id, patch) =>
    set((s) => {
      const next = { ...s.overrides };
      if (patch === null) delete next[id];
      else next[id] = { ...next[id], ...patch };
      return { overrides: next };
    }),

  setOutputDir: (dir, remember) => {
    if (remember) localStorage.setItem(DIR_KEY, dir);
    set({ outputDir: dir });
  },

  setConflictPolicy: (conflictPolicy) => set({ conflictPolicy }),

  jobs: () => {
    const { items, settings, overrides } = get();
    return items.map((item) => {
      const base = settings[item.kind];
      const o = overrides[item.id] ?? {};
      return {
        item,
        targetFormat: o.format ?? base.format,
        preset: o.preset ?? base.preset,
      };
    });
  },

  refreshConflicts: async () => {
    const { outputDir } = get();
    if (!outputDir) return;
    const jobs = get().jobs();
    const signature = JSON.stringify(jobs.map(({ item, targetFormat }) => [item.id, item.path, targetFormat]));
    const requestId = ++conflictCheckId;
    const conflicts = await ipc.checkConflicts(jobs, outputDir);
    const current = get();
    const currentSignature = JSON.stringify(
      current.jobs().map(({ item, targetFormat }) => [item.id, item.path, targetFormat])
    );
    if (
      requestId === conflictCheckId &&
      current.outputDir === outputDir &&
      currentSignature === signature
    ) {
      set({ conflicts });
    }
  },

  start: async () => {
    const { outputDir, conflictPolicy, threadProfile } = get();
    if (!outputDir) return;
    set({ phase: "running", progress: null, result: null });
    await ipc.startConversion(
      { jobs: get().jobs(), outputDir, conflictPolicy, threadProfile },
      (e) => {
        if (e.event === "progress") set({ progress: e.data });
        if (e.event === "finished") set({ phase: "done", result: e.data });
      }
    );
  },

  retryFailed: async () => {
    const failed = new Set(get().result?.failures.map((f) => f.itemId));
    set((s) => ({ items: s.items.filter((i) => failed.has(i.id)) }));
    await get().start();
  },

  reset: () =>
    set({ phase: "idle", items: [], overrides: {}, conflicts: [], progress: null, result: null }),
}));
