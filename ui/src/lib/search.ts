import type { ChangeItem } from "./types";

/** How many matches the history search asks Tracewise for (the newest ones). */
export const SEARCH_LIMIT = 200;

/** The page's own match, for what is already loaded: path, reason, command or change id, any case. */
export function matchesQuery(c: ChangeItem, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return [c.path, c.why, c.command, c.id].some((v) => (v || "").toLowerCase().includes(q));
}

/**
 * Tracewise's answer (paths and reasons over the whole history) together with the loaded events that match (which also covers commands and
 * ids, things Tracewise's search does not look at). One row per change, newest first.
 */
export function mergeSearch(fromTracewise: ChangeItem[], loaded: ChangeItem[], query: string): ChangeItem[] {
  const seen = new Set<string>();
  const out: ChangeItem[] = [];
  for (const c of [...fromTracewise, ...loaded.filter((l) => matchesQuery(l, query))]) {
    if (seen.has(c.id)) continue;
    seen.add(c.id);
    out.push(c);
  }
  return out.sort((a, b) => (b.time?.start ?? "").localeCompare(a.time?.start ?? "") || b.id.localeCompare(a.id));
}

/** What the timeline says about its search, so the person knows how far it reached. */
export function searchNote(o: { query: string; loading: boolean; answeredFor: string; fromTracewise: number; shown: number; failed: boolean }): string {
  if (!o.query.trim()) return "";
  if (o.failed) return "SEARCH FAILED · SHOWING MATCHES IN THE LOADED EVENTS ONLY";
  if (o.loading || o.answeredFor !== o.query.trim()) return "SEARCHING THE WHOLE HISTORY…";
  const capped = o.fromTracewise >= SEARCH_LIMIT ? ` (NEWEST ${SEARCH_LIMIT} OF THE PATH/REASON MATCHES)` : "";
  return `${o.shown} ${o.shown === 1 ? "MATCH" : "MATCHES"} IN THE WHOLE HISTORY${capped}`;
}
