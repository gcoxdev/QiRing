const THEME_STORAGE_KEY = "qiring.ui.theme";
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
  if (!VALID_THEMES.has(theme)) return;
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // The encrypted setting remains authoritative if web storage is unavailable.
  }
}

export function storedThemePreference() {
  try {
    const theme = window.localStorage.getItem(THEME_STORAGE_KEY);
    return VALID_THEMES.has(theme) ? theme : "system";
  } catch {
    return "system";
  }
}

applyThemePreference(storedThemePreference());
