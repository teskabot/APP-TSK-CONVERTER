// Copia el ffmpeg/ffprobe instalados en tu sistema a src-tauri/binaries/
// con el nombre que Tauri exige (<nombre>-<target-triple>[.exe]).
// Solo para DESARROLLO. Para publicar, usa una build LGPL mínima (ver LEEME-AUDIO.md).
import { execSync } from "node:child_process";
import { copyFileSync, mkdirSync, chmodSync } from "node:fs";
import { platform } from "node:os";

const triple = /host: (\S+)/.exec(execSync("rustc -vV").toString())?.[1];
if (!triple) throw new Error("No pude detectar el target triple (¿está instalado Rust?)");
const win = platform() === "win32";
mkdirSync("src-tauri/binaries", { recursive: true });

for (const name of ["ffmpeg", "ffprobe"]) {
  let found;
  try {
    found = execSync(win ? `where ${name}` : `which ${name}`).toString().split(/\r?\n/)[0].trim();
  } catch {
    throw new Error(`No encontré '${name}' en el PATH. Instálalo (brew install ffmpeg / winget install ffmpeg).`);
  }
  const dest = `src-tauri/binaries/${name}-${triple}${win ? ".exe" : ""}`;
  copyFileSync(found, dest);
  if (!win) chmodSync(dest, 0o755);
  console.log(`${found} → ${dest}`);
}
