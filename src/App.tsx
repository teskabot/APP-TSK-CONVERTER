import { useEffect, useState } from "react";
import { BatchPanel } from "./components/BatchPanel";
import { DropZone } from "./components/DropZone";
import { ProgressPanel } from "./components/ProgressPanel";
import { listenFileDrop } from "./lib/ipc";
import { useBatch } from "./store/useBatch";

export default function App() {
  const phase = useBatch((s) => s.phase);
  const init = useBatch((s) => s.init);
  const addPaths = useBatch((s) => s.addPaths);
  const [hovering, setHovering] = useState(false);

  useEffect(() => {
    init();
    let unlisten: (() => void) | undefined;
    listenFileDrop(addPaths, setHovering).then((u) => (unlisten = u));
    return () => unlisten?.();
  }, []);

  return (
    <main className="h-full">
      {phase === "idle" && <DropZone hovering={hovering} />}
      {phase === "ready" && <BatchPanel />}
      {(phase === "running" || phase === "done") && <ProgressPanel />}
    </main>
  );
}
