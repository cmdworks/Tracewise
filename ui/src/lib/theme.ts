export type Theme = "system" | "light" | "dark";

export const THEMES: Theme[] = ["system", "light", "dark"];
const KEY = "trail-theme";

/** The theme the page shows for a choice; `system` follows the operating system. */
export function resolve(theme: Theme, prefersDark: boolean): "light" | "dark" {
  return theme === "system" ? (prefersDark ? "dark" : "light") : theme;
}

export function next(theme: Theme): Theme {
  return THEMES[(THEMES.indexOf(theme) + 1) % THEMES.length];
}

export function load(): Theme {
  try {
    const t = localStorage.getItem(KEY);
    return t === "light" || t === "dark" || t === "system" ? t : "system";
  } catch {
    return "system";
  }
}

export function save(theme: Theme) {
  try {
    localStorage.setItem(KEY, theme);
  } catch {
    // a private window: the choice lasts until the page closes
  }
}

/** Puts the theme on the page (`data-theme` and `class` on html and body). */
export function apply(theme: Theme) {
  const dark = typeof matchMedia === "function" && matchMedia("(prefers-color-scheme: dark)").matches;
  const t = resolve(theme, dark);
  for (const el of [document.documentElement, document.body]) {
    el.setAttribute("data-theme", t);
    el.classList.toggle("dark", t === "dark");
  }
}
