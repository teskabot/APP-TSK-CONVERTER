conversor/
├─ package.json · vite.config.ts · tsconfig.json · index.html
├─ src/                        (Frontend: React + TS + Tailwind v4)
│  ├─ main.tsx · App.tsx       (orquestación por fase: idle → ready → running → done)
│  ├─ index.css                (tokens de diseño + marcas de registro del lienzo)
│  ├─ types.ts                 (espejo de models.rs)
│  ├─ lib/ipc.ts               (única capa que llama a Tauri: invoke, Channel, drag&drop)
│  ├─ lib/formats.ts           (formatos de salida, presets, helpers)
│  ├─ store/useBatch.ts        (estado global con Zustand)
│  └─ components/
│     ├─ DropZone.tsx          (estado inicial)
│     ├─ BatchPanel.tsx        (lote + destino + conflictos)
│     ├─ GroupCard.tsx         (Imágenes / Audio + excepciones por archivo)
│     └─ ProgressPanel.tsx     (progreso + resumen + fallidos)
└─ src-tauri/                  (Backend: Rust)
   ├─ Cargo.toml · build.rs · tauri.conf.json
   ├─ capabilities/default.json
   └─ src/
      ├─ main.rs · lib.rs      (registro de plugins y comandos IPC)
      ├─ models.rs             (structs serde compartidos con el frontend)
      ├─ commands/             (capa IPC, delgada)
      │  ├─ system.rs          (núcleos CPU, carpeta por defecto)
      │  ├─ scan.rs            (carpetas → archivos clasificados)
      │  ├─ conflicts.rs       (aviso de duplicados antes de procesar)
      │  └─ convert.rs         (pool Rayon + eventos de progreso + cancelar)
      └─ engine/
         └─ mod.rs             (convert_one: punto de extensión para audio.rs / image.rs)
