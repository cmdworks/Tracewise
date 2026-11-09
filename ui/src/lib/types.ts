export type Tab = "timeline" | "now" | "files" | "undo" | "projects" | "health" | "settings";

export interface Profile {
  name: string;
  core: boolean;
  coordination: boolean;
  policy: boolean;
  share: boolean;
  strict: boolean;
}

export interface TracewiseStatus {
  repo: string;
  data_dir: string;
  changes: number;
  locks: number;
  daemon: string;
  profile: Profile;
  repo_id: string;
  share?: any;
  contract?: string;
  version?: string;
}

export interface AgentStatus {
  name: string;
  hooks: boolean;
  skill: boolean;
  shims: boolean;
  shell: boolean;
}

export interface ChangeItem {
  /** What recorded it: `scan`, `hook`, `shell`, `shim`, `undo`, `baseline`, ... */
  source?: string;
  /** For an undo record: the ids of the changes it put back. */
  undoes?: string[] | null;
  id: string;
  time: { start: string; end: string };
  path: string;
  op: string;
  old_path?: string | null;
  before?: string | null;
  after?: string | null;
  hash_only?: boolean;
  hunks?: Array<{ old_start: number; old_lines: number; new_start: number; new_lines: number }>;
  actor?: { kind?: string; name?: string; session?: string | null };
  confidence?: string;
  command?: string | null;
  intent?: string | null;
  /** The reason text behind `intent` (which is only an id). */
  why?: string | null;
  session?: string | null;
  git?: { branch?: string; head?: string };
}

export interface BlameLine {
  line: number;
  text: string;
  change: string | null;
  actor?: { kind?: string; name?: string };
  time?: string | null;
  why?: string | null;
}

export interface PendingItem {
  id: string;
  path: string;
  actor?: { kind?: string; name?: string };
  session?: string | null;
  time?: { start: string; end: string };
}

import type { AttributionSummary } from "./attribution";

export interface DoctorReport {
  /** How the last days of changes are credited (`tracewise doctor --json`). */
  attribution?: AttributionSummary | null;
  ok: boolean;
  profile: string;
  problems: string[];
  coverage?: {
    daemon?: boolean;
    git_wrapper?: boolean;
    shell?: boolean;
    shims?: boolean;
    agents?: Record<string, boolean>;
    /** Per agent: the hook events its config lacks (an empty list or no entry means complete). */
    missing_events?: Record<string, string[]>;
    strict?: {
      active?: boolean;
      installed?: boolean;
      running?: boolean;
      reason?: string;
    };
  };
}

export interface TracewiseResult {
  stdout: string;
  stderr: string;
  exit_code: number;
}

export interface RepoDetail {
  changes?: number;
  locks?: number;
  profile?: string;
  loading?: boolean;
  error?: string;
}

export interface ParsedDiffLine {
  type: "header" | "hunk" | "add" | "del" | "ctx" | "meta";
  marker: string;
  code: string;
  oldNum: number | null;
  newNum: number | null;
}

export interface ToastMessage {
  id: string;
  type: "success" | "error" | "info" | "undo";
  title?: string;
  message: string;
  undoId?: string;
  /** The repository the undo ran in: Redo runs there, whatever project is open now. */
  repo?: string;
  originalId?: string;
}
