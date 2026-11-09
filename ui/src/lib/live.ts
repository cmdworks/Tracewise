import type { Tab, TrailStatus } from "./types";

export interface ReloadDecision {
  reloadTimeline: boolean;
  reloadPending: boolean;
  reloadLocks: boolean;
  reloadAgents: boolean;
  newChangesCount: number;
}

/**
 * Decides what data needs reloading when a new status snapshot arrives.
 * Pure function: takes previous status and next status.
 */
export function decideReload(
  prev: TrailStatus | null,
  next: TrailStatus | null,
  activeTab: Tab = "timeline"
): ReloadDecision {
  if (!next) {
    return {
      reloadTimeline: false,
      reloadPending: false,
      reloadLocks: false,
      reloadAgents: false,
      newChangesCount: 0,
    };
  }

  // Initial load or repository changed: reload everything
  if (!prev || prev.repo !== next.repo) {
    return {
      reloadTimeline: true,
      reloadPending: true,
      reloadLocks: true,
      reloadAgents: true,
      newChangesCount: 0,
    };
  }

  const prevChanges = prev.changes ?? 0;
  const nextChanges = next.changes ?? 0;
  const changesMoved = nextChanges !== prevChanges;
  const newCount = nextChanges > prevChanges ? nextChanges - prevChanges : 0;

  const prevLocks = prev.locks ?? 0;
  const nextLocks = next.locks ?? 0;
  const locksMoved = nextLocks !== prevLocks;

  return {
    reloadTimeline: changesMoved,
    reloadPending: changesMoved,
    reloadLocks: locksMoved,
    reloadAgents: false,
    newChangesCount: newCount,
  };
}

/**
 * Formats relative time since last update (e.g. "updated 2s ago").
 */
export function formatRelativeTime(date: Date, now: Date = new Date()): string {
  const diffSec = Math.max(0, Math.floor((now.getTime() - date.getTime()) / 1000));
  if (diffSec < 3) return "updated just now";
  if (diffSec < 60) return `updated ${diffSec}s ago`;
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `updated ${diffMin}m ago`;
  const diffHours = Math.floor(diffMin / 60);
  return `updated ${diffHours}h ago`;
}

/**
 * Checks if status data is stale (e.g. no successful poll for over 10 seconds).
 */
export function isStatusStale(lastUpdated: Date | null, now: Date = new Date(), thresholdSec = 10): boolean {
  if (!lastUpdated) return true;
  const diffSec = (now.getTime() - lastUpdated.getTime()) / 1000;
  return diffSec >= thresholdSec;
}
