/** The settings `tracewise config` can change, with the words the screen shows. */
export interface SettingDef {
  key: string;
  label: string;
  help: string;
  /** Allowed values, or null for free text that Tracewise checks itself. */
  options: string[] | null;
  placeholder?: string;
}

export interface SettingRow {
  key: string;
  value: string;
  /** Where the value comes from: env, project, global or default. */
  source: string;
}

export const SETTINGS: SettingDef[] = [
  { key: "reasons.mode", label: "Reasons", help: "off: no reasons asked. warn: say when a change has none. enforce: refuse commits without one.", options: ["off", "warn", "enforce"] },
  { key: "profile.name", label: "Profile", help: "core records and attributes; full adds locks, policy checks, the sensor and sharing.", options: ["core", "full"] },
  { key: "retention.commands", label: "Keep command records", help: "How long recorded commands are kept. History of file changes is never pruned by this.", options: null, placeholder: "30d, 12w or off" },
  { key: "storage.max_file", label: "Largest stored file", help: "A bigger file is recorded by its hash only.", options: null, placeholder: "5mb" },
  { key: "storage.secrets", label: "Files with secrets", help: "hash: a text file holding a known secret is recorded by hash only. store: keep it like any text.", options: ["hash", "store"] },
  { key: "durability.level", label: "Durability", help: "full survives a power cut; normal is a little faster but can lose the newest records.", options: ["full", "normal"] }
];

/** Rows from `tracewise config get --json`, in the order of SETTINGS; a key Tracewise did not return is left out. */
export function rowsFrom(raw: unknown): SettingRow[] {
  const list = Array.isArray(raw) ? raw : [];
  const byKey = new Map<string, SettingRow>();
  for (const r of list) {
    if (r && typeof r.key === "string" && typeof r.value === "string") byKey.set(r.key, { key: r.key, value: r.value, source: String(r.source ?? "default") });
  }
  return SETTINGS.map((d) => byKey.get(d.key)).filter((r): r is SettingRow => !!r);
}

/** Words for a source chip. */
export function sourceText(source: string): string {
  return { env: "set by an environment variable", project: "this project", global: "all projects", default: "default" }[source] ?? source;
}

/** The arguments for saving one setting. */
export function setArgs(key: string, value: string, global: boolean): string[] {
  return ["config", "set", ...(global ? ["--global"] : []), key, value.trim()];
}

/** What `tracewise share status --json` says; never holds the key itself. */
export interface ShareStatus {
  configured: boolean;
  remote?: string;
  store?: "origin" | "url";
  level?: string;
  refs?: string;
  encrypt?: boolean;
  key_id?: string | null;
  key_exported?: boolean;
  trusted?: boolean;
  warnings: string[];
}

export function shareFrom(raw: unknown): ShareStatus {
  const r = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;
  const text = (v: unknown) => (typeof v === "string" ? v : undefined);
  return {
    configured: r.configured === true,
    remote: text(r.remote),
    store: r.store === "origin" ? "origin" : r.store === "url" ? "url" : undefined,
    level: text(r.level),
    refs: text(r.refs),
    encrypt: r.encrypt === true,
    key_id: text(r.key_id) ?? null,
    key_exported: r.key_exported === true,
    trusted: r.trusted === true,
    warnings: Array.isArray(r.warnings) ? r.warnings.filter((w): w is string => typeof w === "string") : []
  };
}

/** Arguments for `tracewise share init`: the project's own origin when `remote` is empty. */
export function shareInitArgs(opts: { remote: string; level: string; encrypt: boolean }): string[] {
  const args = ["share", "init", "--level", opts.level];
  if (opts.remote.trim()) args.push("--remote", opts.remote.trim());
  if (opts.encrypt) args.push("--encrypt");
  return args;
}

/** The refusal Tracewise gives when the share profile is off, in words for the screen. */
export function shareProfileOff(stderr: string): boolean {
  return /share profile is not enabled/i.test(stderr);
}

export const SHARE_LEVELS: Array<{ value: string; label: string; help: string }> = [
  { value: "summary", label: "Summary", help: "Who, when, why and which file. No file contents." },
  { value: "full", label: "Full", help: "Also the file contents, so another machine can undo." },
  { value: "off", label: "Off", help: "Nothing is sent." }
];
