<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import { app } from "$lib/app/state.svelte";
  import * as fx from "$lib/app/fx";
</script>

<div class="projects-view-container">
  <div class="card mb-4 global-card">
    <span class="badge badge-accent font-mono">GLOBAL ENVIRONMENT</span>
    <h2 class="card-title mt-1 font-mono">Tracewise Global Config</h2>
    <div class="global-kvs-grid mt-3 font-mono">
      <div class="kv-box"><span class="kv-k">Config Path</span><span class="kv-v">~/.tracewise/config.toml</span></div>
      <div class="kv-box"><span class="kv-k">Default Profile</span><span class="kv-v text-accent font-bold">core</span></div>
      <div class="kv-box"><span class="kv-k">Registry File</span><span class="kv-v">~/.tracewise/repos</span></div>
      <div class="kv-box"><span class="kv-k">Daemon Process</span><span class="kv-v tabular-nums">PID {app.daemonPid ?? "none"}</span></div>
    </div>
  </div>

  <div class="card mb-4">
    <div class="card-header-flex">
      <div>
        <h3 class="card-title font-mono">Registered Repositories ({app.watchedRepos.length})</h3>
        <p class="section-sub">Discovered via <code>tracewise daemon status --json</code>:</p>
      </div>
      <button type="button" class="btn btn-secondary btn-sm font-mono" onclick={fx.fetchDaemonStatus}>
        <Icon name="refresh" size={12} />
        <span>Scan Repos</span>
      </button>
    </div>

    <div class="projects-list-grid mt-3">
      {#each app.watchedRepos as repoPath}
        {@const detail = app.repoDetails[repoPath]}
        <div class="repo-card font-mono" class:is-active={repoPath === app.currentRepo}>
          <div class="repo-card-top">
            <Icon name="folder" size={15} class="text-accent" />
            <span class="repo-card-name font-bold">{repoPath.split("/").pop()}</span>
            {#if repoPath === app.currentRepo}
              <span class="badge badge-accent">ACTIVE</span>
            {/if}
          </div>
          <span class="repo-card-path text-muted text-xs">{repoPath}</span>
          <div class="repo-card-stats mt-2">
            {#if detail}
              <span class="mini-tag tag-on tabular-nums">{detail.changes?.toLocaleString() ?? 0} changes</span>
              <span class="mini-tag tabular-nums">{detail.locks ?? 0} locks</span>
              <span class="mini-tag">{detail.profile ?? "core"}</span>
            {:else}
              <span class="text-muted text-xs">Querying…</span>
            {/if}
          </div>
          <div class="repo-card-actions mt-3">
            {#if repoPath !== app.currentRepo}
              <button type="button" class="btn btn-primary btn-xs font-mono" onclick={() => fx.switchRepo(repoPath)}>
                Switch Project
              </button>
            {:else}
              <button type="button" class="btn btn-secondary btn-xs font-mono" onclick={() => (app.activeTab = "timeline")}>
                View Timeline
              </button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </div>

  <div class="card">
    <h3 class="card-title font-mono">Open Another Project</h3>
    <div class="add-proj-row mt-3">
      <input
        type="text"
        class="search-input font-mono"
        placeholder="/path/to/project (already set up with tracewise init)"
        bind:value={app.newProjectInput}
        onkeydown={(e) => e.key === "Enter" && fx.registerNewProject()}
      />
      <button type="button" class="btn btn-primary btn-sm font-mono" onclick={fx.registerNewProject}>
        Open
      </button>
    </div>
    {#if app.newProjectError}
      <div class="text-danger text-xs mt-2 font-mono">{app.newProjectError}</div>
    {/if}
  </div>
</div>

<style>
  .font-mono {
    font-family: var(--font-mono, "JetBrains Mono", monospace);
  }
  .tabular-nums {
    font-variant-numeric: tabular-nums;
  }
  .font-bold {
    font-weight: 600;
  }
  .text-accent {
    color: var(--accent);
  }
  .text-danger {
    color: var(--destructive);
  }
  .text-muted {
    color: var(--muted-foreground);
  }
  .text-xs {
    font-size: 11px;
  }
  .mt-1 {
    margin-top: 4px;
  }
  .mt-2 {
    margin-top: 8px;
  }
  .mt-3 {
    margin-top: 12px;
  }
  .mb-4 {
    margin-bottom: 16px;
  }
  .card {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 14px;
  }
  .card-header-flex {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
  }
  .card-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }
  .section-sub {
    margin: 3px 0 0;
    font-size: 11px;
    color: var(--muted-foreground);
  }
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    padding: 5px 10px;
    border-radius: 5px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
    transition: all 0.12s;
  }
  .btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .btn-sm {
    padding: 4px 8px;
    font-size: 11px;
  }
  .btn-xs {
    padding: 2px 6px;
    font-size: 10px;
  }
  .btn-primary {
    background: var(--accent);
    color: #031e0c;
  }
  .btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .btn-secondary {
    background: var(--card-elevated);
    border-color: var(--border);
    color: var(--foreground);
  }
  .btn-secondary:hover:not(:disabled) {
    background: var(--muted);
    border-color: var(--border-strong);
  }
  .badge {
    display: inline-flex;
    align-items: center;
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.05em;
  }
  .badge-accent {
    background: var(--accent-dim);
    color: var(--accent);
    border: 1px solid var(--border-accent);
  }
  .mini-tag {
    font-size: 9px;
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    color: var(--muted-foreground);
  }
  .mini-tag.tag-on {
    color: var(--accent);
    border-color: var(--border-accent);
  }
  .projects-view-container {
    overflow-y: auto;
  }
  .global-card {
    border-color: var(--border-accent);
  }
  .global-kvs-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 8px;
  }
  .kv-box {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 5px;
  }
  .kv-k {
    font-size: 10px;
    text-transform: uppercase;
    color: var(--muted-foreground);
  }
  .kv-v {
    font-size: 11px;
    word-break: break-all;
  }
  .projects-list-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 10px;
  }
  .repo-card {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 10px;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  .repo-card.is-active {
    border-color: var(--border-accent);
    background: var(--card-elevated);
  }
  .repo-card-top {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .repo-card-name {
    font-size: 13px;
  }
  .repo-card-stats {
    display: flex;
    gap: 4px;
  }
  .add-proj-row {
    display: flex;
    gap: 6px;
    max-width: 500px;
  }
  .search-input {
    flex: 1;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 5px 8px;
    color: var(--foreground);
    font-size: 11px;
  }
</style>
