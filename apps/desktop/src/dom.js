export function byId(id) {
  const element = document.getElementById(id);
  if (!element) throw new Error(`Missing required UI element: ${id}`);
  return element;
}

const ICON_PATHS = Object.freeze({
  add: ["M12 5v14M5 12h14"],
  save: ["M5 4h12l2 2v14H5z", "M8 4v6h8V4M8 20v-6h8v6"],
  trash: ["M4 7h16M9 7V4h6v3M7 7l1 13h8l1-13M10 11v5M14 11v5"],
  menu: ["M4 7h16M4 12h16M4 17h16"],
  lock: ["M6 10h12v10H6z", "M8 10V7a4 4 0 0 1 8 0v3"],
  vault: ["M4 5h16v15H4z", "M8 5V3h8v2M9 12h6M12 9v6"],
  ring: ["M11 3a7 7 0 1 0 0 14 7 7 0 0 0 0-14", "m16 15 5 5M18 17l-2 2M20 19l-2 2"],
  key: ["M14 8a4 4 0 1 1-1.2 2.85L4 20v-3l2-2h3l1.85-1.8", "M16.5 7.5h.01"],
  shield: ["M12 3 5 6v5c0 4.5 2.8 7.8 7 10 4.2-2.2 7-5.5 7-10V6z", "m9 12 2 2 4-5"],
  backup: ["M5 5h14v15H5z", "M8 5V3h8v2M8 16h8M9 9h6"],
  settings: ["M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.38a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z", "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6"],
  clear: ["m6 6 12 12M18 6 6 18"],
  external: ["M14 5h5v5M19 5l-9 9", "M17 13v6H5V7h6"],
  copy: ["M9 9h11v11H9z", "M4 15V4h11"],
  eye: ["M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6S2 12 2 12", "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6"],
  eye_off: ["M3 3l18 18", "M10.6 6.2A11 11 0 0 1 12 6c6.5 0 10 6 10 6a15 15 0 0 1-3.1 3.8M6.2 6.2C3.5 8.1 2 12 2 12s3.5 6 10 6c1.2 0 2.3-.2 3.3-.6", "M10 9.2A3 3 0 0 1 14.8 14"],
  sparkle: ["M12 3l1.3 4.2L17 9l-3.7 1.8L12 15l-1.3-4.2L7 9l3.7-1.8z", "M18 15l.7 2.3L21 18l-2.3.7L18 21l-.7-2.3L15 18l2.3-.7z"],
  clock: ["M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18", "M12 7v5l3 2"],
  question: ["M12 18h.01M9.5 9a2.5 2.5 0 1 1 3.5 2.3c-.8.5-1 1-1 2.2", "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18"],
  play: ["M8 5v14l11-7z"],
  folder: ["M3 6h7l2 2h9v11H3z"],
  rotate: ["M20 7v5h-5M4 17v-5h5", "M6.1 8a7 7 0 0 1 11.7-1L20 12M4 12l2.2 5a7 7 0 0 0 11.7-1"],
  refresh: ["M20 7v5h-5M4 17v-5h5", "M6.1 8a7 7 0 0 1 11.7-1M17.9 16a7 7 0 0 1-11.7 1"],
  upload: ["M12 16V4m-4 4 4-4 4 4", "M5 14v6h14v-6"],
  globe: ["M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18", "M3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18"],
  undo: ["M9 7 4 12l5 5M5 12h9a5 5 0 0 1 5 5"],
  print: ["M7 9V3h10v6M7 18H4V9h16v9h-3", "M7 14h10v7H7z"],
  qr: ["M4 4h6v6H4zM6 6h2v2H6z", "M14 4h6v6h-6zM16 6h2v2h-2z", "M4 14h6v6H4zM6 16h2v2H6z", "M14 14h2v2h-2zM18 14h2v6h-2zM14 18h2v2h-2z"],
  check: ["m5 12 4 4L19 6"],
  download: ["M12 4v12m-4-4 4 4 4-4", "M5 20h14"],
  file: ["M6 3h8l4 4v14H6z", "M14 3v5h5"],
  image: ["M4 5h16v14H4z", "m5 16 4-4 3 3 2-2 5 4", "M15 9h.01"],
  remove: ["M5 12h14"],
  grip: ["M9 5h.01M15 5h.01M9 12h.01M15 12h.01M9 19h.01M15 19h.01"],
  sort_ascending: ["M4 6h11M4 12h8M4 18h5", "m15 15 3 3 3-3M18 18V6"],
  sort_descending: ["M4 6h5M4 12h8M4 18h11", "m15 9 3-3 3 3M18 6v12"]
});

function syncButtonTitle(button) {
  const iconOnly = document.documentElement.dataset.buttonDisplay === "icons" && !button.closest(".app-menu");
  if (iconOnly) {
    if (!button.hasAttribute("title") || button.dataset.autoTitle === "true") {
      button.title = button.dataset.label || button.getAttribute("aria-label") || "";
      button.dataset.autoTitle = "true";
    }
  } else if (button.dataset.autoTitle === "true") {
    button.removeAttribute("title");
    delete button.dataset.autoTitle;
  }
}

export function createIcon(name) {
  const paths = ICON_PATHS[name];
  if (!paths) throw new Error(`Unknown icon: ${name}`);
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.classList.add("button-icon");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("focusable", "false");
  svg.setAttribute("viewBox", "0 0 24 24");
  for (const data of paths) {
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", data);
    svg.append(path);
  }
  return svg;
}

export function decorateButton(button, icon = button.dataset.icon) {
  if (!icon) return button;
  const preserved = [...button.children].filter((child) => child.matches("kbd"));
  const label = button.dataset.label || [...button.childNodes]
    .filter((node) => node.nodeType === Node.TEXT_NODE)
    .map((node) => node.textContent)
    .join(" ")
    .trim() || button.textContent.trim();
  const labelNode = document.createElement("span");
  labelNode.className = "button-label";
  labelNode.textContent = label;
  button.replaceChildren(createIcon(icon), labelNode, ...preserved);
  button.dataset.icon = icon;
  button.dataset.hasIcon = "true";
  button.dataset.label = label;
  if (!button.hasAttribute("aria-label")) button.setAttribute("aria-label", label);
  syncButtonTitle(button);
  return button;
}

export function decorateButtons(root = document) {
  root.querySelectorAll("button[data-icon]:not([data-has-icon])").forEach((button) => decorateButton(button));
}

export function refreshButtonTitles(root = document) {
  root.querySelectorAll("button[data-has-icon]").forEach(syncButtonTitle);
}

export function setButtonLabel(button, label) {
  button.dataset.label = label;
  const labelNode = button.querySelector(":scope > .button-label");
  if (labelNode) labelNode.textContent = label;
  else button.textContent = label;
  if (!button.hasAttribute("data-fixed-aria-label")) button.setAttribute("aria-label", label);
  syncButtonTitle(button);
}

export function setButtonIcon(button, icon) {
  const current = button.querySelector(":scope > .button-icon");
  if (current) current.replaceWith(createIcon(icon));
  button.dataset.icon = icon;
}

export function createElement(tag, options = {}) {
  const element = document.createElement(tag);
  if (options.className) element.className = options.className;
  if (options.text !== undefined) element.textContent = options.text;
  if (options.type) element.type = options.type;
  if (options.attributes) {
    for (const [name, value] of Object.entries(options.attributes)) {
      element.setAttribute(name, value);
    }
  }
  if (options.icon && tag === "button") decorateButton(element, options.icon);
  return element;
}

export function setHidden(element, hidden) {
  element.hidden = hidden;
}

export function formatDate(value) {
  if (!value) return "Unknown";
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short"
  }).format(new Date(value));
}

export function formatBytes(value) {
  if (!Number.isFinite(value)) return "Unknown size";
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
}
