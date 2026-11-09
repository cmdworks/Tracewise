import { errorText } from "$lib/result";
import { app } from "./state.svelte";
import type { Who } from "./state.svelte";
import { fetchAgents, fetchBackground, fetchDaemonStatus, fetchDoctor, fetchLocks, fetchPending, fetchSearch, fetchTimeline, selectChange } from "./fx-load";

export function loadOk(key: string) {
    const { [key]: _gone, ...rest } = app.loadProblem;
    app.loadProblem = rest;
    app.lastGood = { ...app.lastGood, [key]: new Date().toLocaleTimeString() };
  }

export function loadFailed(key: string, e: unknown) {
    app.loadProblem = { ...app.loadProblem, [key]: errorText(e) };
  }

export function retryLoad(key: string) {
    const retry: Record<string, () => unknown> = {
      timeline: () => fetchTimeline(),
      pending: () => fetchPending(),
      locks: () => fetchLocks(),
      background: () => fetchBackground(),
      daemon: () => fetchDaemonStatus(),
      doctor: () => fetchDoctor(),
      agents: () => fetchAgents(),
      search: () => fetchSearch()
    };
    retry[key]?.();
  }

export function addToast(type: "success" | "error" | "info" | "undo", message: string, title?: string, undoId?: string, repo?: string) {
    const id = Math.random().toString(36).substring(2, 9);
    app.toasts = [...app.toasts, { id, type, message, title, undoId, repo }];
    // an undo offers a Redo, which must not vanish before anyone can reach it
    setTimeout(() => {
      dismissToast(id);
    }, type === "undo" && undoId ? 30000 : 6000);
  }

export function dismissToast(id: string) {
    app.toasts = app.toasts.filter((t) => t.id !== id);
  }

export function toggleBlameWrap() {
  app.blameWrap = !app.blameWrap;
  if (typeof localStorage !== "undefined") localStorage.setItem("tracewise-blame-wrap", app.blameWrap ? "1" : "0");
}

export function toggleSidebar() {
    app.sidebarCollapsed = !app.sidebarCollapsed;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("tracewise-sidebar-collapsed", app.sidebarCollapsed ? "1" : "0");
    }
  }

/** Splits a command line like a shell would for quotes: "a b" and 'a b' stay one argument. No expansion happens. */
export function splitArgs(line: string): string[] {
    const out: string[] = [];
    let cur = "";
    let quote: string | null = null;
    let started = false;
    for (const ch of line) {
      if (quote) {
        if (ch === quote) quote = null;
        else cur += ch;
      } else if (ch === '"' || ch === "'") {
        quote = ch;
        started = true;
      } else if (/\s/.test(ch)) {
        if (started || cur) out.push(cur);
        cur = "";
        started = false;
      } else {
        cur += ch;
      }
    }
    if (started || cur) out.push(cur);
    return out;
  }

/** Two-click confirm: the first click arms the button for 4 s, the second one runs. */
export function arm(key: string): boolean {
    clearTimeout(app.armTimer);
    if (app.armed === key) {
      app.armed = null;
      return true;
    }
    app.armed = key;
    app.armTimer = setTimeout(() => (app.armed = null), 4000);
    return false;
  }

export function whoText(w: Who): string {
    if (!w) return "unknown";
    if (typeof w === "string") {
      try {
        return whoText(JSON.parse(w));
      } catch {
        return w;
      }
    }
    return w.name || w.kind || "unknown";
  }

export function getActorInfo(actor?: { kind?: string; name?: string }): {
    name: string;
    colorClass: string;
    icon: string;
  } {
    const kind = actor?.kind || "unknown";
    const name = (actor?.name || kind).toLowerCase();

    if (kind === "human" || name === "you") {
      return { name: actor?.name || "you", colorClass: "actor-human", icon: "user" };
    }
    if (name.includes("claude")) {
      return { name: "claude", colorClass: "actor-claude", icon: "claude" };
    }
    if (name.includes("codex")) {
      return { name: "codex", colorClass: "actor-codex", icon: "codex" };
    }
    if (name.includes("cursor")) {
      return { name: "cursor", colorClass: "actor-cursor", icon: "terminal" };
    }
    if (name.includes("antigravity")) {
      return { name: "antigravity", colorClass: "actor-gemini", icon: "gemini" }; // no icon of its own yet
    }
    if (name.includes("gemini")) {
      return { name: "gemini", colorClass: "actor-gemini", icon: "gemini" };
    }
    return { name: actor?.name || "unknown", colorClass: "actor-unknown", icon: "unknown" };
  }

export function getOpPrefix(op: string): string {
    if (op === "create") return "+";
    if (op === "modify") return "~";
    if (op === "delete") return "-";
    if (op === "undo") return "↺";
    return "•";
  }

export function splitFilePath(fullPath: string): { dir: string; base: string } {
    if (!fullPath) return { dir: "", base: "" };
    const idx = fullPath.lastIndexOf("/");
    if (idx === -1) return { dir: "", base: fullPath };
    return { dir: fullPath.slice(0, idx + 1), base: fullPath.slice(idx + 1) };
  }

export function formatTime(iso: string): string {
    if (!iso) return "";
    try {
      const d = new Date(iso);
      return d.toLocaleTimeString([], { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" });
    } catch {
      return iso;
    }
  }

export function stepSelection(delta: number) {
    if (app.filteredChanges.length === 0) return;
    const currentIndex = app.selectedChange
      ? app.filteredChanges.findIndex((c) => c.id === app.selectedChange!.id)
      : -1;
    const nextIndex = Math.max(0, Math.min(app.filteredChanges.length - 1, currentIndex + delta));
    selectChange(app.filteredChanges[nextIndex]);
  }

// Resizable Split Drag Handlers
export function startDragSplit(e: MouseEvent) {
    app.isDraggingSplit = true;
    window.addEventListener("mousemove", onDragSplit);
    window.addEventListener("mouseup", stopDragSplit);
  }

export function onDragSplit(e: MouseEvent) {
    if (!app.isDraggingSplit) return;
    const newWidth = window.innerWidth - e.clientX - 20;
    app.inspectorWidth = Math.max(300, Math.min(800, newWidth));
  }

export function stopDragSplit() {
    app.isDraggingSplit = false;
    window.removeEventListener("mousemove", onDragSplit);
    window.removeEventListener("mouseup", stopDragSplit);
  }
