import type { PendingItem } from "./types";

export interface PendingGroup {
  id: string;
  title: string;
  actor: string;
  session?: string;
  items: PendingItem[];
}

/**
 * Groups pending changes by session if available, or by actor/file,
 * so users can annotate groups of related changes with a single reason.
 */
export function groupPendingChanges(items: PendingItem[]): PendingGroup[] {
  const groups = new Map<string, PendingGroup>();

  for (const item of items) {
    const actorName = item.actor?.name || item.actor?.kind || "unknown";
    const session = item.session || undefined;

    let groupId: string;
    let title: string;

    if (session) {
      groupId = `session:${session}`;
      title = `Session ${session}`;
    } else {
      groupId = `actor:${actorName.toLowerCase()}`;
      title = "No session"; // these may belong to different tasks: say so rather than imply one task

    }

    let g = groups.get(groupId);
    if (!g) {
      g = {
        id: groupId,
        title,
        actor: actorName,
        session,
        items: [],
      };
      groups.set(groupId, g);
    }
    g.items.push(item);
  }

  return Array.from(groups.values());
}
