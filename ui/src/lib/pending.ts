import type { ChangeItem, PendingItem } from "./types";

/**
 * How many of the newest events are looked at for changes without a reason. `trail pending` has no limit and answers with every such
 * change (about 18 MB on a 100,000-change history, over the desktop bridge's 8 MB cap), so the page asks `trail log` for a bounded
 * slice instead and says so.
 */
export const PENDING_SCAN = 500;
export const PENDING_KEEP = 30;

/** The same rule as `trail pending`: no intent, and not a baseline row. Newest first, as they come. */
export function pendingFrom(rows: ChangeItem[], keep = PENDING_KEEP): { items: PendingItem[]; more: boolean } {
  const all = rows.filter((c) => !c.intent && c.source !== "baseline");
  return {
    items: all.slice(0, keep).map((c) => ({ id: c.id, path: c.path, actor: c.actor, session: c.actor?.session ?? null, time: c.time })),
    more: all.length > keep
  };
}
