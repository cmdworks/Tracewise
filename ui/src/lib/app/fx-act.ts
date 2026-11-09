import { invokeTracewise } from "$lib/api";
import type { AgentStatus, ChangeItem } from "$lib/types";
import { app } from "./state.svelte";
import { addToast, arm, splitArgs } from "./fx-core";
import { setArgs, shareInitArgs } from "$lib/settings";
import { fetchAgents, fetchSettings, fetchShare, fetchBackground, fetchDaemonStatus, fetchDoctor, fetchLocks, fetchStatus, fetchTimeline } from "./fx-load";

// --- UNDO ACTIONS (M5: Conflict Disables Undo by Default) ---
export async function openUndoModal(c: ChangeItem) {
    app.undoTarget = c;
    app.showUndoModal = true;
    app.undoStrictMode = false;
    await executeSingleDryRun(c.id, false);
  }

export async function executeSingleDryRun(id: string, strict: boolean) {
    app.undoDryRunLoading = true;
    app.undoDryRunResult = null;
    try {
      const args = ["undo", id, "--dry-run", ...(strict ? ["--strict"] : []), "--json"];
      const res = await invokeTracewise(args, app.currentRepo);
      if (res.exit_code === 0) {
        app.undoDryRunResult = {
          success: true,
          message: "Dry run passed: Tracewise found no conflict with later changes. Review the diff before confirming.",
        };
      } else {
        app.undoDryRunResult = {
          success: false,
          message: res.stderr || "Conflict detected: file was modified subsequently or lock is active.",
        };
      }
    } catch (e: any) {
      app.undoDryRunResult = {
        success: false,
        message: e?.toString() || "Dry run failed",
      };
    } finally {
      app.undoDryRunLoading = false;
    }
  }

export async function applySingleUndo() {
    const target = app.undoTarget;
    if (!target) return;
    const repo = app.currentRepo;
    app.undoApplying = true;
    try {
      const args = ["undo", target.id, ...(app.undoStrictMode ? ["--strict"] : []), "--json"];
      const res = await invokeTracewise(args, repo);
      if (res.exit_code === 0) {
        let undoId = "";
        try {
          undoId = JSON.parse(res.stdout).change || "";
        } catch {
          undoId = "";
        }
        app.showUndoModal = false;
        if (repo === app.currentRepo) {
          fetchTimeline();
          fetchStatus();
        }
        const name = target.path.split("/").pop();
        // Redo needs the id of the undo record; without it there is nothing safe to offer
        if (undoId) addToast("undo", `Undid change on ${name}`, "Undo Complete", undoId, repo);
        else addToast("success", `Undid change on ${name} (no undo id was returned, so Redo is not offered)`, "Undo Complete");
      } else {
        addToast("error", res.stderr || `Undo failed (code ${res.exit_code})`, "Undo Failed");
      }
    } catch (e: any) {
      addToast("error", e?.toString() || "Undo error");
    } finally {
      app.undoApplying = false;
    }
  }

/** Redo from the Undo Centre's list. */
export async function redoUndo(undoId: string) {
  app.redoingId = undoId;
  try {
    await applyRedo(undoId);
  } finally {
    app.redoingId = "";
  }
}

export async function applyRedo(undoId: string, repo?: string) {
    const target = repo ?? app.currentRepo;
    try {
      const res = await invokeTracewise(["redo", undoId, "--json"], target);
      if (res.exit_code === 0) {
        if (target === app.currentRepo) {
          fetchTimeline();
          fetchStatus();
        }
        addToast("success", "Redo applied successfully");
      } else {
        addToast("error", res.stderr || `Redo failed (code ${res.exit_code})`);
      }
    } catch (e: any) {
      addToast("error", `Redo error: ${e?.toString()}`);
    }
  }

export async function executeBatchDryRun() {
    app.batchDryRunLoading = true;
    app.batchDryRunResult = null;
    app.batchPreview = null;
    const preview = { repo: app.currentRepo, actor: app.batchActor, since: app.batchSince };
    try {
      const args = ["undo", "--actor", preview.actor, "--since", preview.since, "--dry-run", "--json"];
      const res = await invokeTracewise(args, preview.repo);
      if (preview.repo !== app.currentRepo) return;
      if (res.exit_code === 0) {
        app.batchPreview = preview;
        const parsed = JSON.parse(res.stdout);
        app.batchDryRunResult = {
          reverted: parsed.reverted || [],
          skipped: parsed.skipped || [],
        };
      } else {
        app.batchDryRunResult = {
          reverted: [],
          skipped: [],
          error: res.stderr || `Dry run failed (code ${res.exit_code})`,
        };
      }
    } catch (e: any) {
      app.batchDryRunResult = {
        reverted: [],
        skipped: [],
        error: e?.toString(),
      };
    } finally {
      app.batchDryRunLoading = false;
    }
  }

export async function applyBatchUndo() {
    const preview = app.batchPreview;
    if (!preview || preview.repo !== app.currentRepo) {
      app.batchDryRunResult = null;
      addToast("error", "The preview belongs to another repository; run it again.");
      return;
    }
    app.batchApplying = true;
    try {
      const args = ["undo", "--actor", preview.actor, "--since", preview.since, "--json"];
      const res = await invokeTracewise(args, preview.repo);
      if (res.exit_code === 0) {
        let count = 0;
        try {
          const parsed = JSON.parse(res.stdout);
          count = parsed.reverted ? parsed.reverted.length : 0;
        } catch {}
        addToast("success", `Batch reverted ${count} file(s) for ${preview.actor}`);
        app.batchDryRunResult = null;
        app.batchPreview = null;
        fetchTimeline();
        fetchStatus();
      } else {
        addToast("error", res.stderr || `Batch undo failed (code ${res.exit_code})`);
      }
    } catch (e: any) {
      addToast("error", `Batch undo error: ${e?.toString()}`);
    } finally {
      app.batchApplying = false;
    }
  }

export async function runCustomCommand(cmdStr?: string) {
    const toRun = cmdStr || app.customCmd;
    app.customCmd = toRun;
    app.activeTab = "health";
    app.cmdLoading = true;
    app.cmdError = null;
    app.cmdOutput = "";
    app.cmdExitCode = null;

    try {
      const args = splitArgs(toRun);
      const res = await invokeTracewise(args, app.currentRepo);
      app.cmdExitCode = res.exit_code;
      if (res.exit_code === 0) {
        app.cmdOutput = res.stdout;
      } else {
        app.cmdError = res.stderr || `Process returned exit code ${res.exit_code}`;
        app.cmdOutput = res.stdout;
      }
    } catch (err: any) {
      app.cmdError = err?.toString() || "Failed to run command";
    } finally {
      app.cmdLoading = false;
    }
  }

/** Runs one allow-listed tracewise command as the person, reports the result, then refreshes. */
export async function act(label: string, args: string[], after?: () => void): Promise<boolean> {
    app.busy = label;
    try {
      const res = await invokeTracewise(args, app.currentRepo);
      if (res.exit_code === 0) {
        const first = res.stdout.trim().split("\n")[0] || "";
        addToast("success", first.length > 0 && first.length < 160 ? first : `${label} done`, label);
        after?.();
        return true;
      }
      addToast("error", res.stderr.trim() || `${label} failed (code ${res.exit_code})`, `${label} failed`);
    } catch (e: any) {
      addToast("error", e?.toString() || `${label} failed`);
    } finally {
      app.busy = null;
    }
    return false;
  }

export function releaseLock(path: string) {
    if (!arm(`lock:${path}`)) return;
    act("Release lock", ["lock", "release", path], () => {
      fetchLocks();
      fetchStatus();
    });
  }

export function releaseAllLocks() {
    if (!arm("lock:all")) return;
    act("Release all locks", ["lock", "release", "--all"], () => {
      fetchLocks();
      fetchStatus();
    });
  }

export function stopBackground(id: string) {
    if (!arm(`bg:${id}`)) return;
    act("Stop background work", ["background", "stop", id], fetchBackground);
  }

export function restoreChange(c: ChangeItem) {
    if (!arm(`restore:${c.id}`)) return;
    act("Restore file", ["restore", c.path, "--at", c.id], () => {
      fetchTimeline();
      fetchStatus();
    });
  }

export function maintain(label: string, args: string[]) {
    act(label, args, () => {
      fetchStatus();
      fetchDaemonStatus();
      fetchDoctor();
    });
  }

export function toggleAgent(a: AgentStatus) {
    const install = !a.hooks;
    if (!arm(`agent:${a.name}`)) return;
    act(`${install ? "Install" : "Remove"} ${a.name}`, ["agent", install ? "install" : "uninstall", a.name], () =>
      fetchAgents()
    );
  }

export async function openJsonViewer(id: string) {
    try {
      const res = await invokeTracewise(["show", id, "--json"], app.currentRepo);
      app.rawJsonContent = res.stdout || JSON.stringify({ error: res.stderr }, null, 2);
      app.showJsonModal = true;
    } catch (e: any) {
      addToast("error", "Failed to retrieve change JSON");
    }
  }

/** Saves one setting for this project or for all projects, then reads them all again. */
export async function saveSetting(key: string, value: string, global: boolean) {
  try {
    const res = await invokeTracewise(setArgs(key, value, global), app.currentRepo);
    if (res.exit_code !== 0) {
      addToast("error", res.stderr.trim() || `Could not save ${key}`);
      return false;
    }
    addToast("success", `${key} is now ${value.trim()}`);
    await fetchSettings();
    return true;
  } catch (e) {
    addToast("error", `Could not save ${key}: ${e instanceof Error ? e.message : e}`);
    return false;
  }
}

async function shareCall(args: string[], ok: string): Promise<string | null> {
  try {
    const res = await invokeTracewise(args, app.currentRepo);
    if (res.exit_code !== 0) {
      addToast("error", res.stderr.trim() || "Tracewise refused");
      return null;
    }
    if (ok) addToast("success", ok);
    await fetchShare();
    return res.stdout;
  } catch (e) {
    addToast("error", e instanceof Error ? e.message : String(e));
    return null;
  }
}

export async function startSharing(opts: { remote: string; level: string; encrypt: boolean }) {
  return (await shareCall(shareInitArgs(opts), "Sharing is on")) !== null;
}

/** Fetches the key string for the person to copy; it stays in memory only until cleared. */
export async function exportKey() {
  const out = await shareCall(["share", "key", "export"], "");
  if (out) app.exportedKey = out.trim();
}

export async function rotateKey() {
  return (await shareCall(["share", "key", "rotate"], "New key made. Export it and give it to the others.")) !== null;
}

export async function importKey(key: string, current: boolean) {
  return (await shareCall(["share", "key", "import", ...(current ? ["--current"] : []), key.trim()], "Key imported")) !== null;
}
