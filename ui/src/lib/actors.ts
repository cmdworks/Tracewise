import type { ChangeItem } from "./types";

export interface ActorOption {
  id: string;
  label: string;
  icon: string;
}

const KNOWN_AGENTS = ["claude", "codex", "gemini"];

export function isBaselineChange(c: { path?: string; op?: string }): boolean {
  return c.path === "." || c.op === "baseline";
}

/**
 * Builds the actor filter options for the timeline:
 * [All], You (if human changes exist), the standard agents, and any extra agents
 * found in history (e.g. Cursor, Antigravity).
 */
export function buildTimelineActorOptions(changes: ChangeItem[]): ActorOption[] {
  const label = (n: string) => n.charAt(0).toUpperCase() + n.slice(1);
  const foundAgents = new Set<string>();

  for (const c of changes) {
    if (c.actor?.kind === "agent" && c.actor.name) {
      foundAgents.add(c.actor.name.toLowerCase());
    }
  }

  const otherAgents = Array.from(foundAgents).filter((a) => !KNOWN_AGENTS.includes(a));

  return [
    { id: "all", label: "All", icon: "" },
    { id: "you", label: "You", icon: "user" },
    ...KNOWN_AGENTS.map((id) => ({ id, label: label(id), icon: id })),
    ...otherAgents.map((id) => ({ id, label: label(id), icon: "terminal" })),
  ];
}

/**
 * Filters a change list by actor filter id: "all", "you", or a specific agent name.
 */
export function matchesActorFilter(c: ChangeItem, actorFilter: string): boolean {
  if (!actorFilter || actorFilter === "all") return true;

  const kind = c.actor?.kind || "unknown";
  const name = (c.actor?.name || "").toLowerCase();

  if (actorFilter === "you") {
    return kind === "human" || name === "you";
  }

  if (actorFilter === "unknown") {
    return kind === "unknown" || name === "unknown";
  }

  // Agent filter: match by agent name
  return name === actorFilter.toLowerCase();
}

/**
 * Extracts distinct files touched in history, excluding the baseline '.' row.
 */
export function extractDistinctFiles(
  changes: ChangeItem[],
  getActorName: (actor?: { kind?: string; name?: string }) => string
): { path: string; count: number; lastActor: string; lastTime: string }[] {
  const map = new Map<string, { path: string; count: number; lastActor: string; lastTime: string }>();

  for (const c of changes) {
    if (!c.path || isBaselineChange(c)) continue;
    const existing = map.get(c.path);
    if (existing) {
      existing.count += 1;
    } else {
      map.set(c.path, {
        path: c.path,
        count: 1,
        lastActor: getActorName(c.actor),
        lastTime: c.time?.start || "",
      });
    }
  }

  return Array.from(map.values());
}
