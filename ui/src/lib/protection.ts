import type { DoctorReport } from "./types";

export interface Level {
  /** 0 to 3, higher is stronger. */
  rank: number;
  name: string;
  /** What Tracewise promises at this level. */
  guarantees: string;
  /** What it does not, and what would raise the level. */
  limits: string;
}

export interface Notice {
  id: string;
  kind: string;
  text: string;
  start: string;
}

/** The command kinds Tracewise writes about itself or about writes it could not explain. */
export const NOTICE_KINDS = ["gap", "repair", "unhooked"];

const LEVELS: Level[] = [
  {
    rank: 0,
    name: "Not recording",
    guarantees: "Nothing: the watch daemon is not running, so changes made now are only found by the next scan.",
    limits: "Start the daemon from Maintenance."
  },
  {
    rank: 1,
    name: "Recording",
    guarantees: "Every file change is recorded and can be undone. Who made it is a guess from timing.",
    limits: "Set up the shell, the git wrapper and agent hooks so changes are credited to who made them."
  },
  {
    rank: 2,
    name: "Attributed",
    guarantees: "Changes by agents with hooks, shell commands and git are credited by name. Editors and background writes stay unknown.",
    limits: "A tool that skips the hooks (a full path such as /bin/rm, a script) is still only seen as a file event. The strict sensor closes that."
  },
  {
    rank: 3,
    name: "Strict",
    guarantees: "The system sensor names the process behind each write, so a hook-less tool is credited exactly too.",
    limits: "Anything running as you can still edit Tracewise's own history; only a separate root helper (the Verified level) would stop that."
  }
];

/** The protection level that this machine's health report supports. */
export function level(d: DoctorReport | null | undefined): Level {
  const c = d?.coverage;
  if (!c?.daemon) return LEVELS[0];
  const hooked = Object.values(c.agents ?? {}).some(Boolean);
  const attributed = !!c.shell && !!c.git_wrapper && hooked;
  if (!attributed) return LEVELS[1];
  return c.strict?.active ? LEVELS[3] : LEVELS[2];
}

/** Ways the protection can be bypassed or has a hole right now, most important first. */
export function alerts(d: DoctorReport | null | undefined, notices: Notice[] = []): string[] {
  if (!d) return [];
  const c = d.coverage;
  const out: string[] = [];
  if (c && !c.daemon) out.push("The watch daemon is not running.");
  const bare = Object.entries(c?.agents ?? {}).filter(([, on]) => !on).map(([n]) => n);
  if (bare.length) out.push(`No hooks for ${bare.join(", ")}: their edits cannot be credited exactly.`);
  for (const [n, ev] of Object.entries(c?.missing_events ?? {})) {
    if (ev.length) out.push(`${n}'s hooks are missing ${ev.join(", ")}.`);
  }
  if (c?.strict?.installed && !c.strict.running) out.push("The strict sensor is installed but not running.");
  out.push(...d.problems);
  for (const n of notices) out.push(n.text);
  return out;
}

/** Notices from the command list: the ones Tracewise wrote about itself, newest first, at most `max`. */
export function noticesFrom(commands: Array<{ id: string; kind: string; text: string; start: string }>, max = 5): Notice[] {
  return commands
    .filter((c) => NOTICE_KINDS.includes(c.kind))
    .sort((a, b) => b.start.localeCompare(a.start))
    .slice(0, max)
    .map(({ id, kind, text, start }) => ({ id, kind, text, start }));
}
