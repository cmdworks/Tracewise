// All of the page's state, as one object: the panels read and write `app`, the modules next to this one act on it.
import { mergeSearch, searchNote } from "$lib/search";
import { recentUndos } from "$lib/undos";
import { buildTimelineActorOptions, matchesActorFilter, extractDistinctFiles } from "$lib/actors";
import type { Notice } from "$lib/protection";
import type { SettingRow, ShareStatus } from "$lib/settings";
import type { Tab, TrailStatus, AgentStatus, ChangeItem, BlameLine, PendingItem, DoctorReport, RepoDetail, ParsedDiffLine, ToastMessage } from "$lib/types";
import { getActorInfo } from "./fx-core";

export type Who = { kind?: string; name?: string; session?: string } | string | null | undefined;
export interface LockItem { path: string; holder?: Who; since?: string; expires?: string }
export interface BgItem { id: string; actor?: Who; command?: string | null; start?: string; expires?: string | null }

export const PANEL_PROBLEMS: Record<Tab, string[]> = {
    timeline: ["timeline", "search"],
    now: ["pending", "locks", "background"],
    files: [],
    undo: [],
    projects: ["daemon"],
    health: ["doctor", "agents", "daemon"],
    settings: []
  };

export const PROBLEM_LABEL: Record<string, string> = {
    timeline: "events",
    search: "the search",
    pending: "pending changes",
    locks: "locks",
    background: "background work",
    daemon: "daemon status",
    doctor: "the health check",
    agents: "agent status"
  };

// Batch Undo State
  /** Tracewise refuses an undo it may not do (an agent undoing a person's change): that is a permission, not a conflict. */
export const isRefusal = (message?: string) => /refused/i.test(message ?? "");

// How many events are loaded; `trail log` has no cursor, so "more" asks for a longer list (UI8).
export const TIMELINE_PAGE = 120;
/** Most events the timeline holds: `trail log` has no cursor, so each page refetches the whole list, and one reply may not pass 8 MB. */
export const TIMELINE_MAX = 5000;

export class AppState {
  // App & Layout State
  activeTab = $state<Tab>("timeline");
  sidebarCollapsed = $state<boolean>(false);
  showShortcutsModal = $state<boolean>(false);
  // Inspector Resizable Split
  inspectorWidth = $state<number>(440);
  isDraggingSplit = $state<boolean>(false);
  inspectorVisible = $state<boolean>(true);
  // What failed to load, per panel (UI7): a failure is shown with a Retry, and what is on screen is marked as the last good data.
  loadProblem = $state<Record<string, string>>({});
  lastGood = $state<Record<string, string>>({});
  panelProblems = $derived(PANEL_PROBLEMS[app.activeTab].filter((k) => app.loadProblem[k]));
  // Notifications Toast Stack
  toasts = $state<ToastMessage[]>([]);
  // Active Project & Repositories
  currentRepo = $state<string>("");
  /** The project came from the last session, not from a choice made now: if it is gone, fall back instead of showing errors. */
  repoRestored = $state<boolean>(false);
  /** The saved session has been read; only then may the page write one (an empty page must not overwrite what was saved). */
  sessionReady = $state<boolean>(false);
  watchedRepos = $state<string[]>([]);
  daemonPid = $state<number | null>(null);
  daemonUptime = $state<number | null>(null);
  showRepoPopover = $state<boolean>(false);
  repoFilterQuery = $state<string>("");
  customRepoInput = $state<string>("");
  newProjectInput = $state<string>("");
  newProjectError = $state<string | null>(null);
  repoDetails = $state<Record<string, RepoDetail>>({});
  status = $state<TrailStatus | null>(null);
  statusLoading = $state<boolean>(true);
  errorMsg = $state<string | null>(null);
  lastRefreshed = $state<string>("");
  lastStatusTime = $state<Date | null>(null);
  statusStale = $state<boolean>(false);
  relativeStatusTime = $state<string>("");
  newChangesPillCount = $state<number>(0);
  statusAnnouncement = $state<string>("");
  // Timeline State
  changes = $state<ChangeItem[]>([]);
  timelineLoading = $state<boolean>(false);
  actorFilter = $state<string>("all");
  opFilter = $state<string>("all");
  onlyNoReason = $state<boolean>(false);
  searchQuery = $state<string>("");
  /** What Tracewise answered for `searchFor` (paths and reasons over the whole history); see `searchSource`. */
  searchRows = $state<ChangeItem[]>([]);
  searchFor = $state<string>("");
  searchLoading = $state<boolean>(false);
  searchSeq = 0;
  selectedChange = $state<ChangeItem | null>(null);
  currentDiff = $state<string>("");
  diffLoading = $state<boolean>(false);
  // Inline Note Editor
  inlineNoteInput = $state<string>("");
  inlineNoteSaving = $state<boolean>(false);
  // Files & Blame State (U2)
  selectedFilePath = $state<string>("");
  fileSearchQuery = $state<string>("");
  blameLines = $state<BlameLine[]>([]);
  blameLoading = $state<boolean>(false);
  blameError = $state<string | null>(null);
  selectedBlameLine = $state<number | null>(null);
  lineWhyData = $state<any | null>(null);
  lineWhyLoading = $state<boolean>(false);
  fileVersionIndex = $state<number>(0);
  /** The selected file's own history, from `trail log <path>` (the loaded events only stand in until it arrives). */
  fileHistory = $state<ChangeItem[]>([]);
  fileHistoryFor = $state<string>("");
  /** Soft-wrap long lines in the blame view (a person's preference, kept in localStorage). */
  blameWrap = $state<boolean>(true);
  // Now Live Screen State
  agents = $state<AgentStatus[]>([]);
  agentsLoading = $state<boolean>(false);
  pendingChanges = $state<PendingItem[]>([]);
  /** There are more changes without a reason than the list shows (see `lib/pending.ts`). */
  pendingMore = $state<boolean>(false);
  pendingLoading = $state<boolean>(false);
  pendingNotes = $state<Record<string, string>>({});
  pendingSavingId = $state<string | null>(null);
  // Health & Doctor State
  doctor = $state<DoctorReport | null>(null);
  doctorLoading = $state<boolean>(false);
  settings = $state<SettingRow[]>([]);
  settingsLoading = $state<boolean>(false);
  share = $state<ShareStatus | null>(null);
  /** Set when Trail says the share profile is off: the Share block then asks for the full profile. */
  shareProfileOff = $state<boolean>(false);
  /** The exported key, shown once on request (it is the key itself); cleared when closed. */
  exportedKey = $state<string>("");
  /** What Trail wrote about itself (restarts, repaired hooks, unexplained writes), newest first. */
  notices = $state<Notice[]>([]);
  // Safe Undo Modal State
  showUndoModal = $state<boolean>(false);
  undoTarget = $state<ChangeItem | null>(null);
  undoDryRunLoading = $state<boolean>(false);
  undoDryRunResult = $state<{ success: boolean; message: string } | null>(null);
  undoStrictMode = $state<boolean>(false);
  undoApplying = $state<boolean>(false);
  batchActor = $state<string>("claude");
  // the three known agents, every other agent that appears in the history (Cursor, ...), and the person
  batchActorOptions = $derived.by(() => {
    const known = ["claude", "codex", "gemini"];
    const label = (n: string) => n.charAt(0).toUpperCase() + n.slice(1);
    const others = [...new Set(app.changes.filter((c) => c.actor?.kind === "agent" && c.actor.name && !known.includes(c.actor.name)).map((c) => c.actor!.name!))];
    return [
      ...known.map((id) => ({ id, label: label(id), icon: id })),
      ...others.map((id) => ({ id, label: label(id), icon: "terminal" })),
      ...(app.humanActorName ? [{ id: app.humanActorName, label: "You", icon: "user" }] : [])
    ];
  });
  batchSince = $state<string>("1h");
  batchDryRunLoading = $state<boolean>(false);
  batchDryRunResult = $state<{ reverted: string[]; skipped: string[]; error?: string } | null>(null);
  batchApplying = $state<boolean>(false);
  // What the preview was computed for; confirming runs exactly this, never the current form values.
  // The person's own actor name, taken from the recorded history instead of a fixed value
  humanActorName = $derived(app.changes.find((c) => c.actor?.kind === "human" && c.actor?.name)?.actor?.name ?? "");
  batchPreview = $state<{ repo: string; actor: string; since: string } | null>(null);
  // CLI Runner
  customCmd = $state<string>("status --json");
  cmdLoading = $state<boolean>(false);
  cmdOutput = $state<string>("");
  cmdExitCode = $state<number | null>(null);
  cmdError = $state<string | null>(null);
  // JSON Raw Drawer Modal
  showJsonModal = $state<boolean>(false);
  rawJsonContent = $state<string>("");
  // Parse unified diff lines with dual old/new line numbers
  parsedDiffLines = $derived.by(() => {
    if (!app.currentDiff) return [] as ParsedDiffLine[];
    const raw = app.currentDiff.split("\n");
    const out: ParsedDiffLine[] = [];
    let oldLine = 0;
    let newLine = 0;

    for (const line of raw) {
      if (line.startsWith("+++") || line.startsWith("---")) {
        out.push({ type: "header", marker: "", code: line, oldNum: null, newNum: null });
      } else if (line.startsWith("@@")) {
        const m = line.match(/@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
        if (m) {
          oldLine = parseInt(m[1], 10);
          newLine = parseInt(m[2], 10);
        }
        out.push({ type: "hunk", marker: "@@", code: line, oldNum: null, newNum: null });
      } else if (line.startsWith("+")) {
        out.push({ type: "add", marker: "+", code: line.slice(1), oldNum: null, newNum: newLine++ });
      } else if (line.startsWith("-")) {
        out.push({ type: "del", marker: "-", code: line.slice(1), oldNum: oldLine++, newNum: null });
      } else if (
        line.startsWith("change ") ||
        line.startsWith("Author:") ||
        line.startsWith("Date:") ||
        line.startsWith("Path:")
      ) {
        out.push({ type: "meta", marker: "", code: line, oldNum: null, newNum: null });
      } else {
        out.push({
          type: "ctx",
          marker: " ",
          code: line.startsWith(" ") ? line.slice(1) : line,
          oldNum: oldLine > 0 ? oldLine++ : null,
          newNum: newLine > 0 ? newLine++ : null,
        });
      }
    }
    return out;
  });
  // Bumped on every selection and repository switch; an answer that arrives for an older one is dropped.
  selectSeq = 0;
  timelineSeq = 0;
  // Bumped when the project changes: an answer that began for the old project is dropped by every panel.
  repoEpoch = 0;
  blameSeq = 0;
  whySeq = 0;
  // File Browser: Extract distinct files touched in history (excluding baseline '.' row)
  distinctFiles = $derived.by(() => {
    const list = extractDistinctFiles(app.changes, (a) => getActorInfo(a).name);
    if (!app.fileSearchQuery.trim()) return list;
    const q = app.fileSearchQuery.toLowerCase();
    return list.filter((f) => f.path.toLowerCase().includes(q));
  });
  selectedFileHistory = $derived(
    app.fileHistoryFor === app.selectedFilePath && app.fileHistory.length > 0
      ? app.fileHistory
      : app.changes.filter((c) => c.path === app.selectedFilePath)
  );
  timelineLimit = $state<number>(TIMELINE_PAGE);
  /** A longer list could not be loaded (most likely a reply over the size limit), so no more is offered. */
  timelineStuck = $state<boolean>(false);
  moreToLoad = $derived(
    !!app.status && !app.timelineStuck && app.changes.length >= app.timelineLimit && app.status.changes > app.changes.length && app.timelineLimit < TIMELINE_MAX
  );
  /** The list is as long as it gets (the cap, or a page too big to load) although the history is longer. */
  timelineCapped = $derived(!!app.status && app.changes.length > 0 && app.status.changes > app.changes.length && !app.moreToLoad && !app.timelineLoading);
  locks = $state<LockItem[]>([]);
  bgJobs = $state<BgItem[]>([]);
  busy = $state<string | null>(null);
  armed = $state<string | null>(null);
  armTimer: ReturnType<typeof setTimeout> | undefined = undefined;
  // Timeline actor filter options built dynamically from history
  timelineActorOptions = $derived(buildTimelineActorOptions(app.changes));
  // Filtered Changes
  recentUndos = $derived(recentUndos(app.changes));
  /** The undo whose Redo is running (its button waits). */
  redoingId = $state<string>("");
  searchActive = $derived(app.searchQuery.trim() !== "");
  /** While a search is typed: Tracewise's matches for it plus the loaded events that match; otherwise the loaded events. */
  searchSource = $derived(
    app.searchActive
      ? mergeSearch(app.searchFor === app.searchQuery.trim() ? app.searchRows : [], app.changes, app.searchQuery)
      : app.changes
  );
  searchNote = $derived(
    searchNote({
      query: app.searchQuery,
      loading: app.searchLoading,
      answeredFor: app.searchFor,
      fromTrail: app.searchRows.length,
      shown: app.searchSource.length,
      failed: !!app.loadProblem.search
    })
  );
  filteredChanges = $derived(
    app.searchSource.filter((c) => {
      if (!matchesActorFilter(c, app.actorFilter)) return false;
      if (app.opFilter !== "all" && c.op !== app.opFilter) return false;
      if (app.onlyNoReason && c.intent) return false;
      return true;
    })
  );
  filteredWatchedRepos = $derived(
    app.watchedRepos.filter((r) => !app.repoFilterQuery.trim() || r.toLowerCase().includes(app.repoFilterQuery.toLowerCase()))
  );
}

export const app = new AppState();
