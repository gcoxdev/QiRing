import { cpSync, mkdirSync, readdirSync, rmSync, statSync } from "node:fs";
import { basename, resolve } from "node:path";

const desktopRoot = resolve(import.meta.dirname, "..");
const workspaceRoot = resolve(desktopRoot, "../..");
const bundleRoot = resolve(workspaceRoot, "target/release/bundle");
const outputRoot = resolve(desktopRoot, "release-assets");
const platform = (process.argv[2] || process.platform).toLowerCase().replaceAll(/[^a-z0-9-]/g, "-");
const acceptedSuffixes = [".appimage", ".deb", ".rpm", ".dmg", ".msi", ".exe", ".sig", ".tar.gz"];

rmSync(outputRoot, { recursive: true, force: true });
mkdirSync(outputRoot, { recursive: true });

function walk(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...walk(path));
    else if (entry.isFile()) files.push(path);
  }
  return files;
}

if (!statSync(bundleRoot, { throwIfNoEntry: false })?.isDirectory()) {
  throw new Error(`No Tauri bundle directory found at ${bundleRoot}`);
}

let copied = 0;
const copiedNames = [];
for (const source of walk(bundleRoot)) {
  const lower = source.toLowerCase();
  if (!acceptedSuffixes.some((suffix) => lower.endsWith(suffix))) continue;
  if (statSync(source).size === 0) throw new Error(`Release artifact is empty: ${source}`);
  const destination = resolve(outputRoot, `${platform}-${basename(source)}`);
  cpSync(source, destination);
  copiedNames.push(basename(source).toLowerCase());
  copied += 1;
}

if (copied === 0) throw new Error("Tauri produced no recognized release bundle artifacts.");
const requiredSuffixes = {
  linux: [".appimage", ".deb"],
  macos: [".dmg"],
  windows: [".msi"]
}[platform] || [];
for (const suffix of requiredSuffixes) {
  if (!copiedNames.some((name) => name.endsWith(suffix))) {
    throw new Error(`Missing required ${platform} bundle type: ${suffix}`);
  }
}
console.log(`Collected ${copied} ${platform} release artifacts.`);
