import assert from "node:assert/strict";
import { chmod, mkdtemp, mkdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";
import { nameBuildOutputs, validateOutputName } from "../scripts/name-build-output.mjs";

const temporaryTarget = await mkdtemp(path.join(os.tmpdir(), "qiring-artifact-name-"));
const previousTarget = process.env.CARGO_TARGET_DIR;
process.env.CARGO_TARGET_DIR = temporaryTarget;

after(async () => {
  if (previousTarget === undefined) delete process.env.CARGO_TARGET_DIR;
  else process.env.CARGO_TARGET_DIR = previousTarget;
  await rm(temporaryTarget, { recursive: true, force: true });
});

test("custom output names are safe basenames without extensions", () => {
  assert.equal(validateOutputName(undefined), null);
  assert.equal(validateOutputName(" QiRing-Manjaro "), "QiRing-Manjaro");
  for (const invalid of ["", "../QiRing", "QiRing/App", "QiRing.AppImage", "QiRing!"]) {
    assert.throws(() => validateOutputName(invalid));
  }
});

test("bundle artifacts receive the requested basename and retain their extensions", async () => {
  const appImageDirectory = path.join(temporaryTarget, "release", "bundle", "appimage");
  const debDirectory = path.join(temporaryTarget, "release", "bundle", "deb");
  await mkdir(appImageDirectory, { recursive: true });
  await mkdir(debDirectory, { recursive: true });
  const appImage = path.join(appImageDirectory, "QiRing_0.1.0_amd64.AppImage");
  const deb = path.join(debDirectory, "QiRing_0.1.0_amd64.deb");
  await writeFile(appImage, "appimage");
  await chmod(appImage, 0o755);
  await writeFile(deb, "deb");

  const outputs = await nameBuildOutputs({
    outputName: "QiRing-Manjaro",
    formats: ["appimage", "deb"],
    platform: "linux"
  });

  assert.deepEqual(outputs, [
    path.join(appImageDirectory, "QiRing-Manjaro.AppImage"),
    path.join(debDirectory, "QiRing-Manjaro.deb")
  ]);
  assert.equal(await readFile(outputs[0], "utf8"), "appimage");
  assert.equal(await readFile(outputs[1], "utf8"), "deb");
  assert.notEqual((await stat(outputs[0])).mode & 0o111, 0);
  await assert.rejects(stat(appImage));
  await assert.rejects(stat(deb));
});

test("standalone Windows executable naming uses the requested basename", async () => {
  const releaseDirectory = path.join(temporaryTarget, "release");
  await mkdir(releaseDirectory, { recursive: true });
  const executable = path.join(releaseDirectory, "qiring-desktop.exe");
  await writeFile(executable, "windows executable");

  const [output] = await nameBuildOutputs({
    outputName: "QiRing-Portable",
    formats: ["binary"],
    platform: "win32"
  });

  assert.equal(output, path.join(releaseDirectory, "QiRing-Portable.exe"));
  assert.equal(await readFile(output, "utf8"), "windows executable");
});
