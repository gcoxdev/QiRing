import { invoke } from "@tauri-apps/api/core";

const VALID_THEMES = new Set(["system", "dark", "light"]);

export function applyThemePreference(theme) {
  const preference = VALID_THEMES.has(theme) ? theme : "system";
  const systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const resolved = preference === "light" || (preference === "system" && !systemDark)
    ? "light"
    : "dark";
  document.documentElement.dataset.theme = resolved;
  document.querySelector('meta[name="theme-color"]')?.setAttribute(
    "content",
    resolved === "light" ? "#e9efec" : "#0a0e0d"
  );
}

export function persistThemePreference(theme) {
  if (!VALID_THEMES.has(theme)) return Promise.resolve();
  return invoke("set_bootstrap_theme", { theme }).catch(() => {
    // The encrypted setting remains authoritative if the sidecar preference
    // cannot be written. The current page still keeps the selected theme.
  });
}

export async function storedThemePreference() {
  try {
    const theme = await invoke("get_bootstrap_theme");
    return VALID_THEMES.has(theme) ? theme : "system";
  } catch {
    return "system";
  }
}

applyThemePreference("system");
storedThemePreference().then(applyThemePreference);
