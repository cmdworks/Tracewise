<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import { app } from "$lib/app/state.svelte";
  import * as fx from "$lib/app/fx";
</script>

<div class="undo-view-container">
  <div class="card mb-4">
    <h2 class="card-title font-mono">Batch Rollback by Actor & Window</h2>
    <p class="section-sub">Roll back all changes produced by an agent within a timeframe:</p>

    <div class="batch-form-grid mt-3">
      <div class="form-col">
        <span class="form-lbl font-mono">TARGET ACTOR</span>
        <div class="pills-flex">
          {#each app.batchActorOptions as actOpt}
            <button
              type="button"
              class="batch-pill font-mono"
              class:active={app.batchActor === actOpt.id}
              onclick={() => { app.batchActor = actOpt.id; app.batchDryRunResult = null; app.batchPreview = null; }}
            >
              <Icon name={actOpt.icon} size={12} />
              <span>{actOpt.label}</span>
            </button>
          {/each}
        </div>
      </div>

      <div class="form-col">
        <span class="form-lbl font-mono">TIME WINDOW</span>
        <div class="pills-flex">
          {#each ["15m", "1h", "4h", "24h"] as win}
            <button
              type="button"
              class="batch-pill font-mono"
              class:active={app.batchSince === win}
              onclick={() => { app.batchSince = win; app.batchDryRunResult = null; app.batchPreview = null; }}
            >
              {win}
            </button>
          {/each}
        </div>
      </div>
    </div>

    <div class="mt-4">
      <button
        type="button"
        class="btn btn-primary btn-sm font-mono"
        onclick={fx.executeBatchDryRun}
        disabled={app.batchDryRunLoading}
      >
        <Icon name="search" size={13} />
        <span>{app.batchDryRunLoading ? "Scanning…" : `Dry Run Rollback for ${app.batchActor} (${app.batchSince})`}</span>
      </button>
    </div>
  </div>

  {#if app.batchDryRunResult}
    <div class="card">
      <h3 class="card-title font-mono">Dry Run Verification Result</h3>
      {#if app.batchDryRunResult.error}
        <div class="alert alert-destructive mt-3 font-mono">{app.batchDryRunResult.error}</div>
      {:else}
        <div class="dryrun-metrics mt-3 font-mono">
          <div class="stat-box">
            <span class="stat-number text-accent tabular-nums">{app.batchDryRunResult.reverted.length}</span>
            <span class="stat-label">Files Safe to Revert</span>
          </div>
          <div class="stat-box">
            <span class="stat-number tabular-nums" class:text-warning={app.batchDryRunResult.skipped.length > 0}>
              {app.batchDryRunResult.skipped.length}
            </span>
            <span class="stat-label">Files Skipped (Conflicts)</span>
          </div>
        </div>

        {#if app.batchDryRunResult.reverted.length > 0}
          <div class="mt-4">
            <button
              type="button"
              class="btn btn-destructive btn-sm font-mono"
              onclick={fx.applyBatchUndo}
              disabled={app.batchApplying}
            >
              <Icon name="undo" size={13} />
              <span>{app.batchApplying ? "Reverting…" : `Confirm Revert ${app.batchDryRunResult.reverted.length} File(s)`}</span>
            </button>
          </div>
        {/if}
      {/if}
    </div>
  {/if}

  <div class="card mb-4" aria-labelledby="recent-undos-title">
    <h2 id="recent-undos-title" class="card-title font-mono">Recent undos</h2>
    <p class="section-sub">
      Changes that were put back, among the newest {app.changes.length} events. A person may undo anyone's change; an agent only its own.
    </p>
    {#if app.recentUndos.length === 0}
      <p class="empty-note font-mono">No undo among these events. Undo a change from the Timeline (press <kbd>u</kbd>) and it is listed here, with Redo.</p>
    {:else}
      <ul class="undo-list">
        {#each app.recentUndos as u (u.undo.id)}
          {@const parts = fx.splitFilePath(u.undo.path)}
          {@const who = fx.getActorInfo(u.undo.actor)}
          <li class="undo-row">
            <div class="undo-main">
              <span class="undo-path font-mono" title={u.undo.path}><strong>{parts.base}</strong> <span class="text-muted">{parts.dir}</span></span>
              <span class="undo-meta text-muted font-mono">
                undone by {who.name} · {fx.formatTime(u.undo.time?.start)}{u.undo.why ? ` · “${u.undo.why}”` : ""}
              </span>
            </div>
            <button
              type="button"
              class="link-btn font-mono"
              onclick={() => {
                fx.selectChange(u.undo);
                app.activeTab = "timeline";
              }}
            >
              Open in timeline
            </button>
            {#if u.likelyRedone}
              <span class="redone font-mono" title="A later redo of this file is in the history. Tracewise does not record which undo a redo applied, so this is a judgement.">likely redone</span>
            {:else}
              <button
                type="button"
                class="btn btn-secondary btn-sm font-mono"
                disabled={app.redoingId === u.undo.id}
                aria-label="Redo the undo of {u.undo.path}"
                onclick={() => fx.redoUndo(u.undo.id)}
              >
                {app.redoingId === u.undo.id ? "Redoing…" : "Redo"}
              </button>
            {/if}
          </li>
        {/each}
      </ul>
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
  .text-accent {
    color: var(--accent);
  }
  .text-warning {
    color: var(--warning);
  }
  .mt-3 {
    margin-top: 12px;
  }
  .mt-4 {
    margin-top: 16px;
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
  .btn-primary {
    background: var(--accent);
    color: #031e0c;
  }
  .btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .btn-destructive {
    background: var(--destructive);
    color: #ffffff;
  }
  .undo-view-container {
    overflow-y: auto;
  }
  .batch-form-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 12px;
  }
  .form-col {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .form-lbl {
    font-size: 10px;
    color: var(--muted-foreground);
    letter-spacing: 0.05em;
  }
  .pills-flex {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .batch-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--bg-subtle);
    color: var(--foreground);
    font-size: 11px;
    cursor: pointer;
  }
  .batch-pill.active {
    border-color: var(--accent);
    background: var(--accent-dim);
  }
  .dryrun-metrics {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 10px;
  }
  .stat-box {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 5px;
  }
  .stat-number {
    font-size: 20px;
    font-weight: 700;
  }
  .stat-label {
    font-size: 10px;
    text-transform: uppercase;
    color: var(--muted-foreground);
  }
  .alert {
    padding: 8px 12px;
    border-radius: 6px;
    margin-bottom: 8px;
  }
  .alert-destructive {
    background: var(--destructive-dim);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: var(--destructive);
  }
  .text-muted {
    color: var(--muted-foreground);
  }
  .btn-secondary {
    background: var(--card-elevated);
    border: 1px solid var(--border-strong);
    color: var(--foreground);
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .empty-note {
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--muted-foreground);
  }
  .empty-note kbd {
    padding: 0 5px;
    border: 1px solid var(--border-strong);
    border-radius: 3px;
  }
  .undo-list {
    list-style: none;
    margin: 10px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  .undo-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 0;
    border-top: 1px solid var(--border);
  }
  .undo-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .undo-path {
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .undo-meta {
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .redone {
    font-size: 11px;
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--muted-foreground);
    white-space: nowrap;
  }
  .link-btn {
    padding: 0;
    border: none;
    background: none;
    color: var(--ring);
    font-size: 11px;
    cursor: pointer;
    text-decoration: underline;
    white-space: nowrap;
  }
</style>
