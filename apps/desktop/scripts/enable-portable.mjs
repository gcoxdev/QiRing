import { access, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { validateOutputName } from "./name-build-output.mjs";

const mode = process.argv[2];
const outputName = validateOutputName(process.env.QIRING_OUTPUT_NAME);
const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const targetDirectory = process.env.CARGO_TARGET_DIR
  ? path.resolve(repositoryRoot, process.env.CARGO_TARGET_DIR)
  : path.join(repositoryRoot, "target");
const releaseDirectory = path.join(targetDirectory, "release");

async function requireFile(file) {
  try {
    await access(file);
  } catch {
    throw new Error(`Expected build output was not found: ${file}`);
  }
}

let markerDirectory;
let launcherDescription;

if (mode === "appimage") {
  markerDirectory = path.join(releaseDirectory, "bundle", "appimage");
  const appImages = outputName
    ? [`${outputName}.AppImage`]
    : (await readdir(markerDirectory)).filter((file) => file.endsWith(".AppImage"));
  if (appImages.length === 0) {
    throw new Error(`No AppImage was found in ${markerDirectory}`);
  }
  for (const appImage of appImages) await requireFile(path.join(markerDirectory, appImage));
  launcherDescription = appImages.join(", ");
} else if (mode === "windows") {
  markerDirectory = releaseDirectory;
  const executable = path.join(markerDirectory, `${outputName || "qiring-desktop"}.exe`);
  await requireFile(executable);
  launcherDescription = path.basename(executable);
} else {
  throw new Error("Usage: node scripts/enable-portable.mjs appimage|windows");
}

const marker = path.join(markerDirectory, "qiring-portable");
await writeFile(
  marker,
  "QiRing portable mode marker. Keep this file beside the launcher and distribute the pair together.\n",
  { encoding: "utf8", mode: 0o600 }
);

console.log(`Portable mode enabled for ${launcherDescription}`);
console.log(`Marker: ${marker}`);
console.log(`At first launch, QiRing will create QiRingData in this directory.`);
