const APPLE_PLATFORM = /mac|iphone|ipad|ipod/i;

export function detectedPlatform(navigatorLike = globalThis.navigator) {
  return navigatorLike?.userAgentData?.platform
    || navigatorLike?.platform
    || navigatorLike?.userAgent
    || "";
}

export function usesCommandKey(platform = detectedPlatform()) {
  return APPLE_PLATFORM.test(platform);
}

export function shortcutModifier(platform = detectedPlatform()) {
  return usesCommandKey(platform) ? "⌘" : "Ctrl";
}

export function formatShortcut(keys, platform = detectedPlatform()) {
  const parts = String(keys).split("+").map((part) => part.trim()).filter(Boolean);
  if (!usesCommandKey(platform)) return ["Ctrl", ...parts].join("+");
  return `⌘${parts.map((part) => part === "Shift" ? "⇧" : part).join("")}`;
}

export function shortcutAriaLabel(keys, platform = detectedPlatform()) {
  const parts = String(keys).split("+").map((part) => part.trim()).filter(Boolean);
  return [usesCommandKey(platform) ? "Command" : "Control", ...parts].join(" plus ");
}

export function renderShortcutLabels(root = document, platform = detectedPlatform()) {
  for (const node of root.querySelectorAll("[data-shortcut]")) {
    node.textContent = formatShortcut(node.dataset.shortcut, platform);
    node.setAttribute("aria-label", shortcutAriaLabel(node.dataset.shortcut, platform));
  }
  for (const node of root.querySelectorAll("[data-shortcut-modifier]")) {
    node.textContent = shortcutModifier(platform);
    node.setAttribute("aria-label", usesCommandKey(platform) ? "Command" : "Control");
  }
}
