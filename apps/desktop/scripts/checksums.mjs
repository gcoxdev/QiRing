import { createHash } from "node:crypto";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const outputRoot = resolve(import.meta.dirname, "../release-assets");
const names = readdirSync(outputRoot)
  .filter((name) => name !== "SHA256SUMS")
  .sort((left, right) => left.localeCompare(right));

if (names.length === 0) throw new Error("No release assets are available for checksum generation.");
const lines = names.map((name) => {
  const digest = createHash("sha256").update(readFileSync(resolve(outputRoot, name))).digest("hex");
  return `${digest}  ${name}`;
});
writeFileSync(resolve(outputRoot, "SHA256SUMS"), `${lines.join("\n")}\n`, { mode: 0o600 });
console.log(`Wrote SHA-256 checksums for ${names.length} release assets.`);
