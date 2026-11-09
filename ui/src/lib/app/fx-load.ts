import { invokeTrail } from "$lib/api";
import { parseJson } from "$lib/result";
import { SEARCH_LIMIT } from "$lib/search";
import { PENDING_SCAN, pendingFrom } from "$lib/pending";
import { decideReload, formatRelativeTime, isStatusStale } from "$lib/live";
import { noticesFrom } from "$lib/protection";
import { rowsFrom, shareFrom, shareProfileOff } from "$lib/settings";
import type { AgentStatus, ChangeItem, PendingItem, DoctorReport, TrailStatus } from "$lib/types";
import { app, TIMELINE_MAX, TIMELINE_PAGE } from "./state.svelte";
import { addToast, loadFailed, loadOk } from "./fx-core";

export async function selectChange(c: ChangeItem) {
    const seq = ++app.selectSeq;
    app.selectedChange = c;
    app.inlineNoteInput = c.why || "";
    app.currentDiff = "";
    app.diffLoading = true;
    try {
      const res = await invokeTrail(["show", c.id, "--json"], app.currentRepo);
      if (seq !== app.selectSeq) return;
      if (res.exit_code === 0 && res.stdout.trim()) {
        try {
          const parsed = JSON.parse(res.stdout);
          app.currentDiff = parsed.diff || "";
        } catch {
          app.currentDiff = res.stdout;
        }
      }
    } catch (e) {
      console.error("Failed to load diff:", e);
    } finally {
      if (seq === app.selectSeq) app.diffLoading = false;
    }
  }

/** Saves the reason; true when Tracewise accepted it (a refusal, such as a too short reason, is a toast and false). */
export async function saveInlineNote(): Promise<boolean> {
    // what was submitted, fixed now: the selection or the project may change while Tracewise answers
    const target = app.selectedChange;
    const text = app.inlineNoteInput.trim();
    if (!target || !text) return false;
    const id = target.id;
    const repo = app.currentRepo;
    app.inlineNoteSaving = true;
    try {
      const res = await invokeTrail(["note", id, text], repo);
      if (res.exit_code !== 0) {
        addToast("error", res.stderr || "Failed to attach note");
        return false;
      }
      addToast("success", target.why ? "Reason changed" : "Reason added");
      if (repo !== app.currentRepo) return true; // another project is open now: nothing of it to update
      // take the saved record (intent id and reason text) from Tracewise instead of guessing it locally
      const saved = await invokeTrail(["show", id, "--json"], repo);
      if (saved.exit_code === 0 && repo === app.currentRepo) {
        const rec = JSON.parse(saved.stdout);
        app.changes = app.changes.map((item) => (item.id === id ? { ...item, intent: rec.intent, why: rec.why } : item));
        if (app.selectedChange?.id === id) {
          app.selectedChange = { ...app.selectedChange, intent: rec.intent, why: rec.why };
          app.inlineNoteInput = rec.why || "";
        }
      }
      return true;
    } catch (e: any) {
      addToast("error", e?.toString() || "Error saving note");
      return false;
    } finally {
      app.inlineNoteSaving = false;
    }
  }

let searchTimer: ReturnType<typeof setTimeout> | undefined;

/** Called as the search text changes: asks Tracewise about the whole history once the typing pauses. */
export function onSearchInput() {
  clearTimeout(searchTimer);
  const q = app.searchQuery.trim();
  if (!q) {
    app.searchLoading = false;
    app.loadProblem = Object.fromEntries(Object.entries(app.loadProblem).filter(([k]) => k !== "search"));
    return;
  }
  app.searchLoading = true;
  searchTimer = setTimeout(() => fetchSearch(q), 300);
}

export async function fetchSearch(q: string = app.searchQuery.trim()) {
  if (!q) return;
  const seq = ++app.searchSeq;
  const epoch = app.repoEpoch;
  app.searchLoading = true;
  try {
    const res = await invokeTrail(["log", "--grep", q, "--limit", String(SEARCH_LIMIT), "--json"], app.currentRepo);
    if (seq !== app.searchSeq || epoch !== app.repoEpoch) return;
    app.searchRows = parseJson<ChangeItem[]>("search", res);
    app.searchFor = q;
    loadOk("search");
  } catch (e) {
    if (seq === app.searchSeq && epoch === app.repoEpoch) loadFailed("search", e);
  } finally {
    if (seq === app.searchSeq) app.searchLoading = false;
  }
}

/** The file's real history (newest first), so "change 3 of 12" is about the file and not about the events on screen. */
export async function fetchFileHistory(path: string) {
  const epoch = app.repoEpoch;
  try {
    const res = await invokeTrail(["log", path, "--limit", "200", "--json"], app.currentRepo);
    if (epoch !== app.repoEpoch || app.selectedFilePath !== path) return;
    app.fileHistory = parseJson<ChangeItem[]>("file history", res);
    app.fileHistoryFor = path;
  } catch {
    // the events already loaded stand in for it
  }
}

export async function openFileBlame(path: string) {
    if (!path) return;
    app.selectedFilePath = path;
    app.activeTab = "files";
    app.blameLoading = true;
    app.blameError = null;
    app.blameLines = [];
    app.selectedBlameLine = null;
    app.lineWhyData = null;
    app.fileVersionIndex = 0;
    fetchFileHistory(path);
    const seq = ++app.blameSeq;
    const epoch = app.repoEpoch;

    try {
      const res = await invokeTrail(["blame", path, "--json"], app.currentRepo);
      if (seq !== app.blameSeq || epoch !== app.repoEpoch) return;
      if (res.exit_code === 0 && res.stdout.trim()) {
        app.blameLines = JSON.parse(res.stdout);
      } else {
        app.blameError = res.stderr || "Could not load blame for this file (it may be untracked or binary).";
      }
    } catch (e: any) {
      if (seq === app.blameSeq) app.blameError = e?.toString() || "Failed to execute trail blame";
    } finally {
      if (seq === app.blameSeq) app.blameLoading = false;
    }
  }

export async function inspectBlameLine(lineNum: number) {
    if (!app.selectedFilePath) return;
    app.selectedBlameLine = lineNum;
    app.lineWhyLoading = true;
    app.lineWhyData = null;
    const seq = ++app.whySeq;
    const epoch = app.repoEpoch;
    try {
      const res = await invokeTrail(["why", `${app.selectedFilePath}:${lineNum}`, "--json"], app.currentRepo);
      if (seq !== app.whySeq || epoch !== app.repoEpoch) return;
      if (res.exit_code === 0 && res.stdout.trim()) {
        app.lineWhyData = JSON.parse(res.stdout);
      }
    } catch (e) {
      console.error("Failed to fetch line why:", e);
    } finally {
      if (seq === app.whySeq) app.lineWhyLoading = false;
    }
  }

export async function fetchPending(repoArg?: string) {
    app.pendingLoading = true;
    const epoch = app.repoEpoch;
    try {
      // not `trail pending`: it has no limit and is huge on a big history (see lib/pending.ts)
      const res = await invokeTrail(["log", "--limit", String(PENDING_SCAN), "--json"], repoArg || app.currentRepo);
      if (epoch !== app.repoEpoch) return;
      const { items, more } = pendingFrom(parseJson<ChangeItem[]>("pending changes", res));
      app.pendingChanges = items;
      app.pendingMore = more;
      loadOk("pending");
    } catch (e) {
      if (epoch === app.repoEpoch) loadFailed("pending", e);
    } finally {
      if (epoch === app.repoEpoch) app.pendingLoading = false;
    }
  }

export async function submitPendingNote(id: string) {
    const text = (app.pendingNotes[id] || "").trim();
    if (!text) return;
    app.pendingSavingId = id;
    try {
      const res = await invokeTrail(["note", id, text], app.currentRepo);
      if (res.exit_code === 0) {
        app.pendingChanges = app.pendingChanges.filter((p) => p.id !== id);
        addToast("success", "Reason saved to change");
        fetchTimeline();
      } else {
        addToast("error", res.stderr || "Failed to save reason");
      }
    } finally {
      app.pendingSavingId = null;
    }
  }

export async function submitBatchPendingNotes(ids: string[], note: string): Promise<boolean> {
    const text = note.trim();
    if (!text || ids.length === 0) return false;
    app.busy = "Saving reasons…";
    const saved = new Set<string>();
    let refusal = "";
    try {
      for (const id of ids) {
        const res = await invokeTrail(["note", id, text], app.currentRepo);
        if (res.exit_code === 0) saved.add(id);
        else refusal = res.stderr || "Failed to save reason";
      }
    } catch (e: any) {
      refusal = e?.toString() || "Failed to save reasons";
    } finally {
      app.busy = null;
    }
    // only what Trail accepted leaves the list; a refused reason keeps its changes pending
    if (saved.size > 0) {
      app.pendingChanges = app.pendingChanges.filter((p) => !saved.has(p.id));
      addToast("success", `Saved reason for ${saved.size} change${saved.size === 1 ? "" : "s"}`);
      fetchTimeline();
    }
    if (refusal) {
      const left = ids.length - saved.size;
      addToast("error", `${left} change${left === 1 ? "" : "s"} not saved: ${refusal.trim()}`);
    }
    return saved.size === ids.length;
  }

export async function fetchDoctor(repoArg?: string) {
    app.doctorLoading = true;
    const epoch = app.repoEpoch;
    try {
      const res = await invokeTrail(["doctor", "--json"], repoArg || app.currentRepo);
      if (epoch !== app.repoEpoch) return;
      app.doctor = parseJson<DoctorReport>("health check", res);
      loadOk("doctor");
      // a failure here only hides the notices; the health check itself is already shown
      invokeTrail(["commands", "--json"], repoArg || app.currentRepo)
        .then((r) => {
          if (epoch === app.repoEpoch) app.notices = noticesFrom(parseJson<Array<{ id: string; kind: string; text: string; start: string }>>("commands", r));
        })
        .catch(() => {});
    } catch (e) {
      if (epoch === app.repoEpoch) loadFailed("doctor", e);
    } finally {
      if (epoch === app.repoEpoch) app.doctorLoading = false;
    }
  }

export async function loadRepoDetail(repoPath: string) {
    app.repoDetails[repoPath] = { ...(app.repoDetails[repoPath] || {}), loading: true, error: undefined };
    try {
      const res = await invokeTrail(["status", "--json"], repoPath);
      if (res.exit_code === 0) {
        const s = JSON.parse(res.stdout);
        app.repoDetails[repoPath] = {
          changes: s.changes ?? 0,
          locks: s.locks ?? 0,
          profile: s.profile?.name ?? "core",
          loading: false,
        };
      } else {
        app.repoDetails[repoPath] = {
          loading: false,
          error: "Not accessible or not a trail project",
        };
      }
    } catch {
      app.repoDetails[repoPath] = {
        loading: false,
        error: "Unable to query status",
      };
    }
  }

export async function loadAllRepoDetails() {
    for (const repo of app.watchedRepos) {
      loadRepoDetail(repo);
    }
  }

export async function registerNewProject() {
    app.newProjectError = null;
    const p = app.newProjectInput.trim();
    if (!p) return;
    try {
      const res = await invokeTrail(["status", "--json"], p);
      if (res.exit_code === 0) {
        if (!app.watchedRepos.includes(p)) {
          app.watchedRepos = [...app.watchedRepos, p];
        }
        app.newProjectInput = "";
        switchRepo(p);
        addToast("success", `Switched to project ${p.split("/").pop()}`);
      } else {
        app.newProjectError = res.stderr || `Not a repository Tracewise tracks yet: run \`trail init\` there first (${res.exit_code})`;
      }
    } catch (e: any) {
      app.newProjectError = e?.message || "Failed to inspect directory";
    }
  }

export async function fetchDaemonStatus() {
    try {
      const res = await invokeTrail(["daemon", "status", "--json"], null);
      const d = parseJson<any>("daemon status", res);
      if (d.repositories && Array.isArray(d.repositories)) {
        app.watchedRepos = d.repositories;
        loadAllRepoDetails();
      }
      app.daemonPid = d.pid ?? null;
      app.daemonUptime = d.uptime_secs ?? null;
      loadOk("daemon");
    } catch (e) {
      loadFailed("daemon", e);
    }
  }

const MISSING_PROJECT = /does not exist|not a repository Tracewise knows/;

/** The project of the last session is gone or no longer registered: say so once and show the project the app itself is in. */
async function fallBackFromMissingProject() {
  const gone = app.currentRepo;
  app.repoRestored = false;
  try {
    const res = await invokeTrail(["status", "--json"], null);
    const there = res.exit_code === 0 ? (JSON.parse(res.stdout) as { repo?: string }).repo : undefined;
    if (there) {
      switchRepo(there);
      app.errorMsg = `The project you had open (${gone}) is not available any more. Showing ${there} instead.`;
      return;
    }
  } catch {
    // fall through to the plain message
  }
  app.currentRepo = "";
  app.errorMsg = `The project you had open (${gone}) is not available any more. Choose a project from the list at the top.`;
}

export async function fetchStatus(repoArg?: string) {
    app.statusLoading = true;
    app.errorMsg = null;
    const epoch = app.repoEpoch;
    try {
      const res = await invokeTrail(["status", "--json"], repoArg || app.currentRepo);
      if (epoch !== app.repoEpoch) return;
      if (res.exit_code === 0) {
        const now = new Date();
        app.status = JSON.parse(res.stdout);
        if (!app.currentRepo && app.status?.repo) {
          app.currentRepo = app.status.repo;
        }
        app.repoRestored = false;
        app.lastRefreshed = now.toLocaleTimeString();
        app.lastStatusTime = now;
        app.relativeStatusTime = formatRelativeTime(now, now);
        app.statusStale = false;
      } else if (app.repoRestored && MISSING_PROJECT.test(res.stderr)) {
        await fallBackFromMissingProject();
      } else {
        app.errorMsg = res.stderr || `Command exited with code ${res.exit_code}`;
      }
    } catch (err: any) {
      // the desktop side refuses a repository path by rejecting the call, not by a failed result
      if (epoch === app.repoEpoch && app.repoRestored && MISSING_PROJECT.test(String(err))) await fallBackFromMissingProject();
      else if (epoch === app.repoEpoch) app.errorMsg = err?.toString() || "Failed to execute trail status";
    } finally {
      if (epoch === app.repoEpoch) app.statusLoading = false;
    }
  }

export async function pollStatus() {
  if (!app.currentRepo || app.statusLoading) return;
  const epoch = app.repoEpoch;
  const now = new Date();
  try {
    const res = await invokeTrail(["status", "--json"], app.currentRepo);
    if (epoch !== app.repoEpoch) return;
    if (res.exit_code === 0) {
      const nextStatus = JSON.parse(res.stdout) as TrailStatus;
      const decision = decideReload(app.status, nextStatus, app.activeTab);
      app.status = nextStatus;
      app.lastStatusTime = now;
      app.relativeStatusTime = formatRelativeTime(now, now);
      app.statusStale = false;
      app.lastRefreshed = now.toLocaleTimeString();

      if (decision.reloadTimeline) {
        if (decision.newChangesCount > 0) {
          app.newChangesPillCount += decision.newChangesCount;
        }
        fetchTimeline();
      }
      if (decision.reloadPending) {
        fetchPending();
      }
      if (decision.reloadLocks) {
        fetchLocks();
      }
    } else {
      if (isStatusStale(app.lastStatusTime, now)) {
        app.statusStale = true;
      }
    }
  } catch {
    if (isStatusStale(app.lastStatusTime, now)) {
      app.statusStale = true;
    }
  }
}

export function loadMoreEvents() {
    app.timelineLimit = Math.min(app.timelineLimit + TIMELINE_PAGE, TIMELINE_MAX);
    fetchTimeline();
  }

export async function fetchTimeline(repoArg?: string) {
    const seq = ++app.timelineSeq;
    app.timelineLoading = true;
    try {
      const res = await invokeTrail(["log", "--limit", String(app.timelineLimit), "--json"], repoArg || app.currentRepo);
      if (seq !== app.timelineSeq) return;
      app.changes = parseJson<ChangeItem[]>("events", res);
      loadOk("timeline");
      if (app.changes.length > 0) {
        // a refresh or "load more" keeps what the person was looking at; only a missing selection falls back to the newest
        if (!app.selectedChange || !app.changes.some((c) => c.id === app.selectedChange!.id)) selectChange(app.changes[0]);
        if (!app.selectedFilePath) {
          app.selectedFilePath = app.changes[0].path;
        }
      }
      app.statusAnnouncement = `Loaded ${app.changes.length} changes`;
    } catch (e) {
      if (seq === app.timelineSeq) {
        if (app.timelineLimit > TIMELINE_PAGE && app.changes.length > 0) {
          // a longer list failed (most likely a reply over the size limit): stay at what is on screen and do not offer more
          app.timelineLimit = Math.max(TIMELINE_PAGE, app.changes.length);
          app.timelineStuck = true;
        }
        // the events already on screen stay, marked as the last good data by the banner
        loadFailed("timeline", e);
      }
    } finally {
      if (seq === app.timelineSeq) app.timelineLoading = false;
    }
  }

export async function fetchAgents(repoArg?: string) {
    app.agentsLoading = true;
    const epoch = app.repoEpoch;
    try {
      const res = await invokeTrail(["agent", "status", "--json"], repoArg || app.currentRepo);
      if (epoch !== app.repoEpoch) return;
      app.agents = parseJson<AgentStatus[]>("agent status", res);
      loadOk("agents");
    } catch (e) {
      if (epoch === app.repoEpoch) loadFailed("agents", e);
    } finally {
      if (epoch === app.repoEpoch) app.agentsLoading = false;
    }
  }

export function switchRepo(newRepo: string) {
    if (!newRepo) return;
    app.currentRepo = newRepo;
    app.showRepoPopover = false;
    app.customRepoInput = "";
    app.selectedFilePath = "";
    // nothing shown or previewed for the old repository may be acted on or kept
    app.selectSeq++;
    app.repoEpoch++;
    app.selectedChange = null;
    app.currentDiff = "";
    app.changes = [];
    app.batchDryRunResult = null;
    app.batchPreview = null;
    app.loadProblem = {};
    app.lastGood = {};
    app.fileHistory = [];
    app.fileHistoryFor = "";
    app.searchRows = [];
    app.searchFor = "";
    app.timelineLimit = TIMELINE_PAGE;
    app.timelineStuck = false;
    app.locks = [];
    app.bgJobs = [];
    app.pendingChanges = [];
    app.pendingMore = false;
    app.blameLines = [];
    app.lineWhyData = null;
    fetchStatus(newRepo);
    fetchTimeline(newRepo);
    fetchAgents(newRepo);
    fetchPending(newRepo);
    fetchLocks();
    fetchBackground();
    fetchDoctor(newRepo);
  }

export async function fetchLocks() {
    const epoch = app.repoEpoch;
    try {
      const res = await invokeTrail(["lock", "list", "--json"], app.currentRepo);
      if (epoch !== app.repoEpoch) return;
      app.locks = parseJson<typeof app.locks>("locks", res);
      loadOk("locks");
    } catch (e) {
      if (epoch === app.repoEpoch) loadFailed("locks", e);
    }
  }

export async function fetchBackground() {
    const epoch = app.repoEpoch;
    try {
      const res = await invokeTrail(["ps", "--json"], app.currentRepo);
      if (epoch !== app.repoEpoch) return;
      app.bgJobs = parseJson<typeof app.bgJobs>("background work", res);
      loadOk("background");
    } catch (e) {
      if (epoch === app.repoEpoch) loadFailed("background", e);
    }
  }

export function refreshAll(repoArg?: string) {
  fetchDaemonStatus();
  fetchStatus(repoArg);
  fetchTimeline(repoArg);
  fetchPending(repoArg);
  fetchLocks();
  fetchAgents(repoArg);
  fetchBackground();
}

export async function fetchSettings() {
  app.settingsLoading = true;
  const epoch = app.repoEpoch;
  try {
    const res = await invokeTrail(["config", "get", "--json"], app.currentRepo);
    if (epoch !== app.repoEpoch) return;
    if (res.exit_code !== 0) throw new Error(res.stderr.trim() || "trail config failed");
    app.settings = rowsFrom(JSON.parse(res.stdout));
  } catch (e) {
    if (epoch === app.repoEpoch) addToast("error", `Could not read the settings: ${e instanceof Error ? e.message : e}`);
  } finally {
    if (epoch === app.repoEpoch) app.settingsLoading = false;
  }
}

export async function fetchShare() {
  const epoch = app.repoEpoch;
  try {
    const res = await invokeTrail(["share", "status", "--json"], app.currentRepo);
    if (epoch !== app.repoEpoch) return;
    app.shareProfileOff = res.exit_code !== 0 && shareProfileOff(res.stderr);
    if (res.exit_code === 0) app.share = shareFrom(JSON.parse(res.stdout));
    else if (!app.shareProfileOff) addToast("error", res.stderr.trim() || "Could not read the sharing setup");
    if (app.shareProfileOff) app.share = null;
  } catch (e) {
    if (epoch === app.repoEpoch) addToast("error", `Could not read the sharing setup: ${e instanceof Error ? e.message : e}`);
  }
}
