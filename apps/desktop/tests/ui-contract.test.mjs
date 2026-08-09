import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

const read = (path) => readFile(new URL(path, import.meta.url), "utf8");

test("production webview boundary is explicit and least privilege", async () => {
  const config = JSON.parse(await read("../src-tauri/tauri.conf.json"));
  assert.equal(config.app.withGlobalTauri, false);
  assert.equal(config.app.windows[0].minWidth, 800);
  assert.equal(config.app.windows[0].minHeight, 600);
  assert.match(config.app.security.csp, /object-src 'none'/);
  assert.doesNotMatch(config.app.security.csp, /unsafe-inline|unsafe-eval/);

  const capability = JSON.parse(await read("../src-tauri/capabilities/main.json"));
  assert.equal(capability.permissions.includes("core:default"), false);
  const opener = capability.permissions.find((permission) => typeof permission === "object" && permission.identifier === "opener:allow-open-url");
  assert.deepEqual(opener.allow.map((entry) => entry.url).sort(), ["http://*", "https://*"]);
});

test("frontend does not inject user-controlled HTML", async () => {
  const source = await read("../src/main.js");
  const backend = await read("../src-tauri/src/lib.rs");
  assert.doesNotMatch(source, /\.innerHTML\s*=/);
  assert.doesNotMatch(source, /insertAdjacentHTML|document\.write/);
  assert.doesNotMatch(backend, /Command::new|fn open_url/);
});

test("native form controls follow the active color scheme", async () => {
  const css = await read("../src/styles.css");
  assert.match(css, /select option[\s\S]*color-scheme:\s*inherit/);
  assert.match(css, /select option[\s\S]*background-color:\s*var\(--surface-2\)/);
  assert.match(css, /select option[\s\S]*color:\s*var\(--text\)/);
});

test("favicon import is capability scoped and blocks private networks", async () => {
  const backend = await read("../src-tauri/src/lib.rs");
  const capability = JSON.parse(await read("../src-tauri/capabilities/main.json"));
  assert.equal(capability.permissions.includes("allow-fetch-favicon"), true);
  assert.match(backend, /no_proxy\(\)/);
  assert.match(backend, /addresses\.iter\(\)\.any\(\|address\| !is_public_ip/);
  assert.match(backend, /MAX_QI_ICON_BYTES/);
});
