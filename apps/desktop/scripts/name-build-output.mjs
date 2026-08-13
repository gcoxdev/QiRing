import { chmod, copyFile, readdir, stat, unlink } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));

const bundleFormats = Object.freeze({
  appimage: { directory: ["bundle", "appimage"], extension: ".AppImage" },
  deb: { directory: ["bundle", "deb"], extension: ".deb" },
  dmg: { directory: ["bundle", "dmg"], extension: ".dmg" },
  msi: { directory: ["bundle", "msi"], extension: ".msi" }
});

export function validateOutputName(value) {
  if (value === undefined) return null;
  const name = value.trim();
  if (!name) throw new Error("QIRING_OUTPUT_NAME cannot be empty.");
  if (name.length > 120) throw new Error("QIRING_OUTPUT_NAME cannot exceed 120 characters.");
  if (!/^[A-Za-z0-9][A-Za-z0-9._ -]*[A-Za-z0-9_-]$|^[A-Za-z0-9]$/.test(name)) {
    throw new Error(
      "QIRING_OUTPUT_NAME must begin with a letter or number and contain only letters, numbers, spaces, dots, underscores, or hyphens."
    );
  }
  const lower = name.toLowerCase();
  if ([".appimage", ".deb", ".dmg", ".msi", ".exe"].some((extension) => lower.endsWith(extension))) {
    throw new Error("Set QIRING_OUTPUT_NAME without a file extension; the build adds it automatically.");
  }
  return name;
}

function releaseDirectory({ profile = "release", targetTriple = null } = {}) {
  const configuredTarget = process.env.CARGO_TARGET_DIR;
  const targetRoot = configuredTarget
    ? path.resolve(repositoryRoot, configuredTarget)
    : path.join(repositoryRoot, "target");
  return targetTriple
    ? path.join(targetRoot, targetTriple, profile)
    : path.join(targetRoot, profile);
}

async function newestArtifact(directory, extension, destination) {
  const entries = await readdir(directory, { withFileTypes: true });
  const candidates = await Promise.all(entries
    .filter((entry) => entry.isFile() && entry.name.endsWith(extension))
    .map(async (entry) => {
      const file = path.join(directory, entry.name);
      return { file, modified: (await stat(file)).mtimeMs };
    }));
  const sources = candidates
    .filter(({ file }) => path.resolve(file) !== path.resolve(destination))
    .sort((left, right) => right.modified - left.modified);
  return sources[0]?.file ?? (candidates.some(({ file }) => path.resolve(file) === path.resolve(destination))
    ? destination
    : null);
}

async function replaceArtifact(source, destination) {
  if (path.resolve(source) === path.resolve(destination)) return;
  const sourceMode = (await stat(source)).mode;
  await copyFile(source, destination);
  await chmod(destination, sourceMode);
  await unlink(source);
}

export async function nameBuildOutputs({
  outputName,
  formats,
  profile = "release",
  targetTriple = null,
  platform = process.platform
}) {
  const name = validateOutputName(outputName);
  if (!name) return [];

  const releaseRoot = releaseDirectory({ profile, targetTriple });
  const expandedFormats = formats.flatMap((format) => format === "native"
    ? platform === "linux"
      ? ["appimage", "deb"]
      : platform === "darwin"
        ? ["dmg"]
        : platform === "win32"
          ? ["msi"]
          : []
    : [format]);
  const outputs = [];

  for (const format of [...new Set(expandedFormats)]) {
    if (format === "binary") {
      const extension = platform === "win32" ? ".exe" : "";
      const source = path.join(releaseRoot, `qiring-desktop${extension}`);
      const destination = path.join(releaseRoot, `${name}${extension}`);
      try {
        await stat(source);
      } catch {
        try {
          await stat(destination);
          outputs.push(destination);
          continue;
        } catch {
          throw new Error(`Expected executable was not found: ${source}`);
        }
      }
      await replaceArtifact(source, destination);
      outputs.push(destination);
      continue;
    }

    const definition = bundleFormats[format];
    if (!definition) throw new Error(`Unsupported artifact format: ${format}`);
    const directory = path.join(releaseRoot, ...definition.directory);
    const destination = path.join(directory, `${name}${definition.extension}`);
    const source = await newestArtifact(directory, definition.extension, destination);
    if (!source) throw new Error(`No ${definition.extension} artifact was found in ${directory}`);
    await replaceArtifact(source, destination);
    outputs.push(destination);
  }
  return outputs;
}

async function main() {
  const args = process.argv.slice(2);
  const debugIndex = args.indexOf("--debug");
  const profile = debugIndex >= 0 ? "debug" : "release";
  if (debugIndex >= 0) args.splice(debugIndex, 1);
  const targetArgument = args.find((argument) => argument.startsWith("--target="));
  const targetTriple = targetArgument?.slice("--target=".length) || null;
  if (targetArgument) args.splice(args.indexOf(targetArgument), 1);
  if (args.length === 0) throw new Error("Provide at least one artifact format to name.");

  const outputs = await nameBuildOutputs({
    outputName: process.env.QIRING_OUTPUT_NAME,
    formats: args,
    profile,
    targetTriple
  });
  for (const output of outputs) console.log(`Named build output: ${output}`);
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;
if (invokedPath === import.meta.url) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
