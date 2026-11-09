import { app } from "./state.svelte";
import { parseSession, SESSION_KEY } from "$lib/session";
import { stepSelection, toggleSidebar } from "./fx-core";
import { fetchAgents, fetchBackground, fetchDaemonStatus, fetchDoctor, fetchLocks, fetchPending, fetchStatus, fetchTimeline, openFileBlame, refreshAll, pollStatus } from "./fx-load";
import { formatRelativeTime, isStatusStale } from "$lib/live";
import { openUndoModal } from "./fx-act";

/** Starts the page: reads the saved layout, loads everything, listens for keys. Returns what stops it. */
export function start(): () => void {
    if (typeof localStorage !== "undefined") {
      const savedCollapsed = localStorage.getItem("trail-sidebar-collapsed");
      if (savedCollapsed === "1") app.sidebarCollapsed = true;
      app.blameWrap = localStorage.getItem("trail-blame-wrap") !== "0";
    }
    // come back to where the last session (or the last hot reload) left off: the same project and tab, and the open file
    const saved = parseSession(typeof localStorage === "undefined" ? null : localStorage.getItem(SESSION_KEY));
    if (saved.repo) {
      app.currentRepo = saved.repo;
      app.repoRestored = true;
      app.activeTab = saved.tab;
      if (saved.file) app.selectedFilePath = saved.file;
    }
    app.sessionReady = true;

    fetchDaemonStatus();
    fetchStatus();
    fetchTimeline();
    fetchAgents();
    // the first change is selected on load; in a narrow window its inspector would cover the list, so it waits for a click
    app.inspectorVisible = window.innerWidth >= 1100;
    fetchLocks(); // the Timeline marks locked files, so it needs them before Now is opened
    fetchPending();
    fetchDoctor();
    if (saved.repo && saved.tab === "files" && saved.file) openFileBlame(saved.file);

    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        app.showUndoModal = false;
        app.showShortcutsModal = false;
        app.showJsonModal = false;
      }
      // while a dialog is open the keys belong to the dialog, not to the page behind it
      if (app.showUndoModal || app.showShortcutsModal || app.showJsonModal) return;
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "b") {
        e.preventDefault();
        toggleSidebar();
        return;
      }

      const active = document.activeElement as HTMLElement | null;
      const tag = active?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || active?.isContentEditable) return;

      if (e.key === "j" || (e.key === "ArrowDown" && app.activeTab === "timeline")) {
        e.preventDefault();
        stepSelection(1);
      } else if (e.key === "k" || (e.key === "ArrowUp" && app.activeTab === "timeline")) {
        e.preventDefault();
        stepSelection(-1);
      } else if (e.key === "r") {
        refreshAll();
      } else if (e.key === "u" && app.selectedChange) {
        openUndoModal(app.selectedChange);
      } else if (e.key === "b" && app.selectedChange) {
        openFileBlame(app.selectedChange.path);
      } else if (e.key === "i") {
        app.inspectorVisible = !app.inspectorVisible;
      } else if (e.key === "?") {
        app.showShortcutsModal = !app.showShortcutsModal;
      } else if (e.key === "1") {
        app.activeTab = "timeline";
      } else if (e.key === "2") {
        app.activeTab = "now";
        fetchAgents();
        fetchPending();
        fetchLocks();
        fetchBackground();
      } else if (e.key === "3") {
        if (app.selectedFilePath) openFileBlame(app.selectedFilePath);
        else app.activeTab = "files";
      } else if (e.key === "4") {
        app.activeTab = "undo";
      } else if (e.key === "5") {
        app.activeTab = "projects";
        fetchDaemonStatus();
      } else if (e.key === "6") {
        app.activeTab = "health";
        fetchDoctor();
      } else if (e.key === "/") {
        e.preventDefault();
        const el = document.getElementById("search-input");
        el?.focus();
      }
    };
    window.addEventListener("keydown", handleKey);

    // The sidebar follows the window while nobody has chosen: a narrow window gives the room to the content, a wide one gets it back.
    // Collapsing or expanding it by hand saves a choice, and from then on the width no longer decides.
    const narrow = window.matchMedia("(max-width: 1099px)");
    const followWidth = () => {
      if (localStorage.getItem("trail-sidebar-collapsed") === null) app.sidebarCollapsed = narrow.matches;
    };
    followWidth();
    narrow.addEventListener("change", followWidth);
    window.addEventListener("resize", followWidth); // some webviews do not fire the media query's change

    // Live status polling every 3s when visible and on focus
    const pollTimer = setInterval(() => {
      if (typeof document !== "undefined" && document.visibilityState === "visible") {
        pollStatus();
      }
      if (app.lastStatusTime) {
        const now = new Date();
        app.relativeStatusTime = formatRelativeTime(app.lastStatusTime, now);
        app.statusStale = isStatusStale(app.lastStatusTime, now);
      }
    }, 3000);

    const handleFocus = () => {
      pollStatus();
    };
    const handleVisibility = () => {
      if (typeof document !== "undefined" && document.visibilityState === "visible") {
        pollStatus();
      }
    };
    window.addEventListener("focus", handleFocus);
    if (typeof document !== "undefined") {
      document.addEventListener("visibilitychange", handleVisibility);
    }

    return () => {
      clearInterval(pollTimer);
      window.removeEventListener("focus", handleFocus);
      if (typeof document !== "undefined") {
        document.removeEventListener("visibilitychange", handleVisibility);
      }
      window.removeEventListener("keydown", handleKey);
      narrow.removeEventListener("change", followWidth);
      window.removeEventListener("resize", followWidth);
    };
  }
