import type { Tab } from "./types";

/** What the page remembers between launches and reloads (hot reload included): the project, the tab and the open file. */
export interface SavedUi {
  repo: string;
  tab: Tab;
  file: string;
}

export const SESSION_KEY = "trail-ui-session";
const TABS: Tab[] = ["timeline", "now", "files", "undo", "projects", "health"];
const NOTHING: SavedUi = { repo: "", tab: "timeline", file: "" };

/** Reads what was saved; anything missing, damaged or of the wrong shape is "nothing saved" for that part, never an error. */
export function parseSession(raw: string | null): SavedUi {
  if (!raw) return { ...NOTHING };
  try {
    const v = JSON.parse(raw) as Record<string, unknown>;
    const repo = typeof v.repo === "string" ? v.repo : "";
    const tab = TABS.includes(v.tab as Tab) ? (v.tab as Tab) : "timeline";
    // a file belongs to a project: without one it means nothing
    const file = repo && typeof v.file === "string" ? v.file : "";
    return { repo, tab, file };
  } catch {
    return { ...NOTHING };
  }
}

/** The text to save, or null when there is no project yet (what was saved before is kept, so a failed start does not wipe it). */
export function serializeSession(s: SavedUi): string | null {
  if (!s.repo.trim()) return null;
  return JSON.stringify({ repo: s.repo, tab: s.tab, file: s.file });
}
