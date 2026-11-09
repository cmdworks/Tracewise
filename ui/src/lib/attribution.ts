/** What `tracewise doctor --json` says about how the last days of changes are credited. */
export interface AttributionSummary {
  days: number;
  changes: number;
  confidence: Record<string, number>;
  actor_kind?: Record<string, number>;
}

export interface Share {
  key: string;
  label: string;
  count: number;
  /** Whole percent; the shares of one summary add up to exactly 100. */
  percent: number;
  /** What it means, in words, from the attribution rules. */
  meaning: string;
}

const KNOWN: Array<[string, string, string]> = [
  ["exact", "Exact", "a hook, a command shim or your own command named who made the change"],
  ["likely", "Likely", "Tracewise matched the change to an open agent, git or shell window"],
  ["ambiguous", "Ambiguous", "more than one actor was active at once, so none could be chosen"],
  ["unknown", "Unknown", "nobody could be named (an editor, a background write, or an agent without hooks)"]
];

/** The confidence shares of a summary, in a fixed order (best first), with whole percents that add up to 100. */
export function shares(a: AttributionSummary | null | undefined): Share[] {
  if (!a || !a.changes) return [];
  const keys = [...KNOWN.map(([k]) => k), ...Object.keys(a.confidence).filter((k) => !KNOWN.some(([n]) => n === k)).sort()];
  const rows = keys
    .map((key) => {
      const known = KNOWN.find(([k]) => k === key);
      const count = a.confidence[key] ?? 0;
      return { key, label: known?.[1] ?? key, count, meaning: known?.[2] ?? "a confidence this view does not describe" };
    })
    .filter((r) => r.count > 0);
  // largest-remainder rounding, so 33.3 + 33.3 + 33.3 shows as 34 + 33 + 33 and not 99
  const exact = rows.map((r) => (r.count * 100) / a.changes);
  const floor = exact.map(Math.floor);
  let left = 100 - floor.reduce((x, y) => x + y, 0);
  const order = exact.map((v, i) => [v - Math.floor(v), i] as const).sort((p, q) => q[0] - p[0] || p[1] - q[1]);
  for (const [, i] of order) {
    if (left <= 0) break;
    floor[i] += 1;
    left -= 1;
  }
  return rows.map((r, i) => ({ ...r, percent: floor[i] }));
}

/** Share of changes with no named actor (unknown or ambiguous). */
export function unnamedPercent(a: AttributionSummary | null | undefined): number {
  if (!a || !a.changes) return 0;
  return Math.round((((a.confidence.unknown ?? 0) + (a.confidence.ambiguous ?? 0)) * 100) / a.changes);
}

/** The same threshold `tracewise doctor` uses before it suggests installing hooks. */
export const UNNAMED_WARN_PERCENT = 30;

/** "165 by agents, 11 by you" from the actor kinds. */
export function kindsText(a: AttributionSummary | null | undefined): string {
  const k = a?.actor_kind ?? {};
  const word: Record<string, string> = { agent: "by agents", human: "by you", unknown: "by nobody named", ambiguous: "by more than one actor" };
  return Object.entries(k)
    .sort((p, q) => q[1] - p[1])
    .map(([kind, n]) => `${n} ${word[kind] ?? `by ${kind}`}`)
    .join(", ");
}
