import type { ChangeItem } from "./types";

export interface TimelineHeaderItem {
  kind: "header";
  id: string;
  label: string;
  count: number;
}

export interface TimelineChangeItem {
  kind: "change";
  id: string;
  change: ChangeItem;
}

export type TimelineListItem = TimelineHeaderItem | TimelineChangeItem;

/**
 * Shortens a file path from the front (e.g. "crates/tracewise-cli/src/cmd_history/revert.rs" -> "…/cmd_history/revert.rs")
 * preserving the last `maxSegments` segments.
 */
export function shortenPathFront(path: string, maxSegments = 2): string {
  if (!path || path === ".") return path || "";
  const segments = path.split("/").filter(Boolean);
  if (segments.length <= maxSegments) {
    return path;
  }
  const lastParts = segments.slice(-maxSegments);
  return `…/${lastParts.join("/")}`;
}

/**
 * Formats relative timestamp for events (e.g. "2 min ago", "14 min ago", "1 hour ago", "Yesterday").
 */
export function formatEventRelativeTime(iso: string, now: Date = new Date()): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;

  const diffSec = Math.max(0, Math.floor((now.getTime() - d.getTime()) / 1000));
  if (diffSec < 60) return "just now";
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin === 1) return "1 min ago";
  if (diffMin < 60) return `${diffMin} min ago`;
  const diffHours = Math.floor(diffMin / 60);
  if (diffHours === 1) return "1 hour ago";
  if (diffHours < 24) return `${diffHours} hours ago`;
  const diffDays = Math.floor(diffHours / 24);
  if (diffDays === 1) return "1 day ago";
  return `${diffDays} days ago`;
}

/**
 * Returns a human-friendly date group label for a timestamp:
 * "TODAY", "YESTERDAY", or formatted date e.g. "MON, 3 NOV" or "28 OCT 2026".
 */
export function getDateGroupLabel(iso: string, now: Date = new Date()): string {
  if (!iso) return "EARLIER";
  const d = new Date(iso);
  if (isNaN(d.getTime())) return "EARLIER";

  const nowDate = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const itemDate = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  const diffDays = Math.round((nowDate.getTime() - itemDate.getTime()) / (1000 * 60 * 60 * 24));

  if (diffDays === 0) return "TODAY";
  if (diffDays === 1) return "YESTERDAY";

  // e.g. "OCT 28, 2026"
  return d
    .toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" })
    .toUpperCase();
}

/**
 * Flattens a list of changes with date section headers for virtual or standard rendering.
 */
export function flattenTimelineWithHeaders(
  changes: ChangeItem[],
  now: Date = new Date()
): TimelineListItem[] {
  const result: TimelineListItem[] = [];
  if (changes.length === 0) return result;

  // Group by date label
  const groups: Map<string, ChangeItem[]> = new Map();
  for (const c of changes) {
    const label = getDateGroupLabel(c.time.start, now);
    let list = groups.get(label);
    if (!list) {
      list = [];
      groups.set(label, list);
    }
    list.push(c);
  }

  for (const [label, items] of groups.entries()) {
    result.push({
      kind: "header",
      id: `header:${label}`,
      label,
      count: items.length,
    });
    for (const c of items) {
      result.push({
        kind: "change",
        id: c.id,
        change: c,
      });
    }
  }

  return result;
}

/** The day header that owns row `index`: the nearest header at or above it, or null when there is none. */
export function headerAt(items: TimelineListItem[], index: number): TimelineHeaderItem | null {
  for (let i = Math.min(index, items.length - 1); i >= 0; i--) {
    const it = items[i];
    if (it.kind === "header") return it;
  }
  return null;
}

/** One letter for an operation, so the row stays short: M, C, D, R (anything else keeps its first letter). */
export function opLetter(op: string): string {
  return ({ modify: "M", create: "C", delete: "D", rename: "R" } as Record<string, string>)[op] ?? (op || "?").charAt(0).toUpperCase();
}
