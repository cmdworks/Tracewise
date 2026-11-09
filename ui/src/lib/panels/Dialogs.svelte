<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import Modal from "$lib/Modal.svelte";
  import { app } from "$lib/app/state.svelte";
  import * as fx from "$lib/app/fx";
</script>

<Modal
  bind:open={app.showUndoModal}
  title={app.undoTarget ? `Undo: ${app.undoTarget.path.split("/").pop()}` : "Undo Change"}
  badge="SAFE UNDO"
  badgeType={app.undoDryRunResult?.success ? "accent" : "warning"}
  maxWidth="560px"
  onclose={() => (app.showUndoModal = false)}
>
  {#if app.undoTarget}
    <div class="undo-modal-content font-mono">
      <div class="undo-meta-grid">
        <div class="meta-kv"><span class="text-muted">Target Path:</span><span>{app.undoTarget.path}</span></div>
        <div class="meta-kv"><span class="text-muted">Change ID:</span><span>{app.undoTarget.id}</span></div>
        <div class="meta-kv"><span class="text-muted">Author:</span><span>{fx.getActorInfo(app.undoTarget.actor).name} ({app.undoTarget.confidence || "unknown"})</span></div>
        {#if app.undoTarget.why}
          <div class="meta-kv"><span class="text-muted">Intent:</span><span class="text-warning">"{app.undoTarget.why}"</span></div>
        {/if}
      </div>

      <!-- Dry Run Result (M5: Disabled when conflict detected) -->
      <div class="dryrun-card mt-3">
        {#if app.undoDryRunLoading}
          <div class="text-muted text-xs">Simulating 3-way reverse patch (dry-run)…</div>
        {:else if app.undoDryRunResult?.success}
          <div class="dryrun-status-line status-ok">
            <Icon name="check" size={15} />
            <div>
              <span class="font-bold text-accent">Safe to Invert</span>
              <p class="text-muted text-xs m-0">Zero subsequent conflicts detected on target lines.</p>
            </div>
          </div>
        {:else if app.undoDryRunResult && !app.undoDryRunResult.success}
          <div class="dryrun-status-line status-warn">
            <Icon name="alert" size={15} />
            <div>
              <span class="font-bold text-warning">{fx.isRefusal(app.undoDryRunResult.message) ? "Not allowed" : "Conflict Blocked"}</span>
              <p class="text-xs m-0">{app.undoDryRunResult.message}</p>
            </div>
          </div>
        {/if}
      </div>

      <div class="strict-box mt-3">
        <label class="strict-check-label">
          <input
            type="checkbox"
            bind:checked={app.undoStrictMode}
            onchange={() => app.undoTarget && fx.executeSingleDryRun(app.undoTarget.id, app.undoStrictMode)}
          />
          <span>Strict mode (<code>--strict</code>): Enforce unchanged working tree</span>
        </label>
      </div>

      <p class="text-muted text-xs mt-3">
        Tracewise will append an <code>op: "undo"</code> record. You can redo this change at any time.
      </p>
    </div>
  {/if}

  {#snippet footer()}
    <button type="button" class="btn btn-secondary btn-sm font-mono" onclick={() => (app.showUndoModal = false)}>
      Cancel
    </button>
    <button
      type="button"
      class="btn btn-destructive btn-sm font-mono"
      disabled={app.undoApplying || app.undoDryRunLoading || !app.undoDryRunResult?.success}
      onclick={fx.applySingleUndo}
    >
      <Icon name="undo" size={13} />
      <span>{app.undoApplying ? "Inverting…" : "Confirm & Invert"}</span>
    </button>
  {/snippet}
</Modal>

<!-- TypeSafe Modal: Keyboard Shortcuts (M11) -->
<Modal
  bind:open={app.showShortcutsModal}
  title="Keyboard Shortcuts"
  badge="PRECISION CONTROLS"
  badgeType="subtle"
  maxWidth="500px"
  onclose={() => (app.showShortcutsModal = false)}
>
  <div class="shortcuts-list font-mono">
    <div class="shortcut-group-title">NAVIGATION</div>
    <div class="shortcut-item"><kbd>j</kbd> / <kbd>↓</kbd><span>Select next change in timeline</span></div>
    <div class="shortcut-item"><kbd>k</kbd> / <kbd>↑</kbd><span>Select previous change in timeline</span></div>
    <div class="shortcut-item"><kbd>/</kbd><span>Focus timeline search filter</span></div>
    <div class="shortcut-item"><kbd>⌘B</kbd><span>Toggle navigation sidebar</span></div>
    <div class="shortcut-item"><kbd>1</kbd>–<kbd>6</kbd><span>Switch primary tabs</span></div>

    <div class="shortcut-group-title mt-3">ACTIONS</div>
    <div class="shortcut-item"><kbd>u</kbd><span>Safe undo selected change</span></div>
    <div class="shortcut-item"><kbd>b</kbd><span>Open file line blame</span></div>
    <div class="shortcut-item"><kbd>i</kbd><span>Toggle diff inspector split</span></div>
    <div class="shortcut-item"><kbd>r</kbd><span>Refresh repository state</span></div>
    <div class="shortcut-item"><kbd>?</kbd><span>Toggle this shortcut guide</span></div>
  </div>
</Modal>

<!-- TypeSafe Modal: Raw JSON Viewer (M9) -->
<Modal
  bind:open={app.showJsonModal}
  title="Raw Change JSON"
  badge="CONTRACT PAYLOAD"
  badgeType="subtle"
  maxWidth="640px"
  onclose={() => (app.showJsonModal = false)}
>
  <pre class="raw-json-box font-mono">{app.rawJsonContent}</pre>
</Modal>

<style>
  .font-mono {
    font-family: var(--font-mono, "JetBrains Mono", monospace);
  }
  .font-bold {
    font-weight: 600;
  }
  .text-accent {
    color: var(--accent);
  }
  .text-warning {
    color: var(--warning);
  }
  .text-muted {
    color: var(--muted-foreground);
  }
  .text-xs {
    font-size: 11px;
  }
  .mt-3 {
    margin-top: 12px;
  }
  .m-0 {
    margin: 0;
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
  .btn-secondary {
    background: var(--card-elevated);
    border-color: var(--border);
    color: var(--foreground);
  }
  .btn-secondary:hover:not(:disabled) {
    background: var(--muted);
    border-color: var(--border-strong);
  }
  .btn-destructive {
    background: var(--destructive);
    color: #ffffff;
  }
  .undo-modal-content {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .undo-meta-grid {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 11px;
  }
  .meta-kv {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    padding: 2px 0;
    border-bottom: 1px solid var(--border);
  }
  .dryrun-card {
    padding: 10px;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  .dryrun-status-line {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .strict-check-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    cursor: pointer;
  }
  .shortcuts-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .shortcut-group-title {
    font-size: 10px;
    color: var(--muted-foreground);
    letter-spacing: 0.05em;
  }
  .shortcut-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 3px 0;
    border-bottom: 1px solid var(--border);
    font-size: 11px;
  }
  .raw-json-box {
    margin: 0;
    padding: 10px;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 6px;
    max-height: 400px;
    overflow: auto;
    font-size: 11px;
  }
</style>
