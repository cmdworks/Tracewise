<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { TrailStatus } from "./types";

  interface Props {
    status: TrailStatus | null;
    daemonPid: number | null;
    daemonUptime: number | null;
    currentRepo: string;
    onopenTab: (tab: any) => void;
  }

  let { status, daemonPid, daemonUptime, currentRepo, onopenTab }: Props = $props();

  const repoName = $derived(currentRepo ? currentRepo.split("/").pop() || currentRepo : "no project");
  const isOnline = $derived(status?.daemon === "running");
  const changeCount = $derived(status?.changes?.toLocaleString() ?? 0);
  const lockCount = $derived(status?.locks ?? 0);
</script>

<footer class="status-bar text-12 select-none" aria-label="Status Bar">
  <div class="status-left">
    <!-- Green status indicator dot -->
    <span class="status-dot" class:online={isOnline}></span>

    <!-- Status summary breadcrumb text -->
    <button
      type="button"
      class="status-text-btn"
      onclick={() => onopenTab("projects")}
      title="Active repository: {currentRepo}"
    >
      <span class="font-mono">{repoName}</span>
    </button>
    <span class="dot-sep">·</span>
    <button
      type="button"
      class="status-text-btn"
      onclick={() => onopenTab("health")}
      title="Daemon state ({daemonPid ? `PID ${daemonPid}` : 'stopped'})"
    >
      <span>daemon: {status?.daemon ?? "stopped"}</span>
    </button>
    <span class="dot-sep">·</span>
    <button
      type="button"
      class="status-text-btn"
      onclick={() => onopenTab("timeline")}
      title="Total changes logged in Tracewise"
    >
      <span class="font-mono">{changeCount}</span>
      <span>changes</span>
    </button>
    <span class="dot-sep">·</span>
    <button
      type="button"
      class="status-text-btn"
      onclick={() => onopenTab("now")}
      title="Active file locks"
    >
      <span>{lockCount} {lockCount === 1 ? "lock" : "locks"}</span>
    </button>
  </div>

  <div class="status-right">
    {#if status?.share}
      <button
        type="button"
        class="status-text-btn"
        onclick={() => onopenTab("projects")}
        title="Shared to {status.share.remote}"
      >
        <span class="hover-text">sharing: {status.share.level}</span>
      </button>
    {/if}
    {#if status?.share && status?.version}<span class="dot-sep">·</span>{/if}
    {#if status?.version}
      <span class="version-label font-mono">v{status.version}</span>
    {/if}

  </div>
</footer>

<style>
  .status-bar {
    height: 32px;
    background: var(--sidebar-bg);
    border-top: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    color: var(--muted-foreground);
    z-index: 40;
    flex-shrink: 0;
    white-space: nowrap;
    overflow: hidden;
    gap: 12px;
  }
  .status-left {
    min-width: 0;
    overflow: hidden;
  }
  /* items keep their size and the bar clips at its edge, instead of squeezing them into each other */
  .status-left > :global(*),
  .status-right > :global(*) {
    flex-shrink: 0;
  }
  @media (max-width: 640px) {
    .status-right {
      display: none;
    }
  }

  .status-left,
  .status-right {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--muted-foreground);
    display: inline-block;
  }

  .status-dot.online {
    background: #10b981;
    box-shadow: 0 0 6px rgba(16, 185, 129, 0.4);
  }

  .status-text-btn {
    background: transparent;
    border: none;
    padding: 2px 4px;
    color: inherit;
    font-size: 12px;
    font-family: inherit;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border-radius: 3px;
    transition: color 0.12s ease, background 0.12s ease;
  }

  .status-text-btn:hover {
    color: var(--foreground);
    background: var(--card-elevated);
  }

  .dot-sep {
    color: var(--border);
    user-select: none;
  }

  .hover-text:hover {
    color: var(--foreground);
  }

  .version-label {
    color: var(--foreground);
  }

  .text-12 {
    font-size: 12px;
    line-height: 16px;
  }
  .font-mono {
    font-family: var(--font-mono, monospace);
  }
</style>
