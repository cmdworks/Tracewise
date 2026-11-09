<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import { app } from "$lib/app/state.svelte";
  import * as fx from "$lib/app/fx";
  import { alerts, level } from "$lib/protection";
  import { kindsText, shares, unnamedPercent, UNNAMED_WARN_PERCENT } from "$lib/attribution";

  let att = $derived(app.doctor?.attribution ?? null);
  let segments = $derived(shares(att));
  let unnamed = $derived(unnamedPercent(att));
  // agents whose edits cannot be credited exactly: no hooks at all, or hooks missing some events
  let noHooks = $derived(Object.entries(app.doctor?.coverage?.agents ?? {}).filter(([, on]) => !on).map(([n]) => n));
  let partial = $derived(Object.entries(app.doctor?.coverage?.missing_events ?? {}).filter(([, ev]) => ev.length > 0));
  // the menu names each section and previews the credit headline, so it is found without scrolling
  let lvl = $derived(level(app.doctor));
  let holes = $derived(alerts(app.doctor, app.notices));
  let sections = $derived([
    { id: "health-protection", label: `Protection · ${lvl.name}` },
    { id: "health-doctor", label: "Doctor" },
    { id: "health-credited", label: segments.length ? `Who is credited · ${segments[0].percent}% ${segments[0].label.toLowerCase()}` : "Who is credited" },
    { id: "health-maintenance", label: "Maintenance" },
    { id: "health-cli", label: "CLI runner" }
  ]);

  function go(id: string) {
    document.getElementById(id)?.scrollIntoView({ block: "start" });
  }
</script>

<div class="health-view-container">
  <nav class="health-nav" aria-label="Health sections">
    {#each sections as s (s.id)}
      <button type="button" class="btn btn-secondary btn-sm font-mono" onclick={() => go(s.id)}>{s.label}</button>
    {/each}
  </nav>

  <section class="card mb-4" id="health-protection" aria-labelledby="protection-title">
    <span class="badge badge-accent font-mono">LEVEL {lvl.rank} OF 3</span>
    <h2 id="protection-title" class="card-title mt-1 font-mono">Protection: {lvl.name}</h2>
    {#if !app.doctor}
      <p class="attr-note font-mono">Waiting for the health check…</p>
    {:else}
      <p class="section-sub"><strong>Guarantees.</strong> {lvl.guarantees}</p>
      <p class="section-sub"><strong>Does not.</strong> {lvl.limits}</p>
      {#if holes.length > 0}
        <ul class="alert-list font-mono" aria-label="Ways this can be bypassed">
          {#each holes as h}
            <li><Icon name="alert" size={12} /> <span>{h}</span></li>
          {/each}
        </ul>
      {:else}
        <p class="attr-note font-mono">No bypass alerts.</p>
      {/if}
    {/if}
  </section>

  <section class="card mb-4" id="health-doctor">
    <div class="card-header-flex">
      <div>
        <span class="badge badge-accent font-mono">DIAGNOSTICS</span>
        <h2 class="card-title mt-1 font-mono">System Doctor Matrix</h2>
        <p class="section-sub">Live component health via <code>tracewise doctor --json</code>:</p>
      </div>
      <button type="button" class="btn btn-secondary btn-sm font-mono" onclick={() => fx.fetchDoctor()}>
        <Icon name="refresh" size={12} />
        <span>Run Doctor</span>
      </button>
    </div>

    {#if app.doctor}
      <div class="doctor-matrix-grid mt-3">
        {#each [
          { label: "Watch Daemon", ok: app.doctor.coverage?.daemon },
          { label: "Git Wrapper", ok: app.doctor.coverage?.git_wrapper },
          { label: "Shell Integration", ok: app.doctor.coverage?.shell },
          { label: "Tool Shims", ok: app.doctor.coverage?.shims },
        ] as item}
          <div class="doctor-cell font-mono" class:cell-ok={item.ok}>
            <Icon name={item.ok ? "check" : "alert"} size={14} />
            <span>{item.label}</span>
            <span class="badge {item.ok ? 'badge-accent' : 'badge-subtle'}">
              {item.ok ? "HEALTHY" : "INACTIVE"}
            </span>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <section class="card mb-4" id="health-credited" aria-labelledby="attribution-title">
    <h2 id="attribution-title" class="card-title font-mono">Who is credited</h2>
    <p class="section-sub">
      The last {att?.days ?? 7} days of changes, without the baseline, by how sure Tracewise is about who made each one.
    </p>
    {#if !app.doctor}
      <p class="attr-note font-mono">Waiting for the health check…</p>
    {:else if segments.length === 0}
      <p class="attr-note font-mono">No changes in the last {att?.days ?? 7} days.</p>
    {:else}
      <p class="attr-headline font-mono">
        <strong>{segments[0].percent}% {segments[0].label.toLowerCase()}</strong> of {att?.changes} changes{kindsText(att) ? ` (${kindsText(att)})` : ""}
      </p>
      <div class="attr-bar" role="img" aria-label={segments.map((s) => `${s.label} ${s.percent}%`).join(", ")}>
        {#each segments as s (s.key)}
          <span class="attr-seg seg-{s.key}" style="flex: {s.count} 1 0" title="{s.label}: {s.count} ({s.percent}%)"></span>
        {/each}
      </div>
      <ul class="attr-legend font-mono">
        {#each segments as s (s.key)}
          <li>
            <span class="attr-swatch seg-{s.key}" aria-hidden="true"></span>
            <strong>{s.label}</strong> <span class="tabular-nums">{s.count} · {s.percent}%</span>
            <span class="text-muted">{`— ${s.meaning}`}</span>
          </li>
        {/each}
      </ul>
      {#if unnamed > UNNAMED_WARN_PERCENT}
        <div class="attr-warn font-mono" role="note">
          <Icon name="alert" size={14} />
          <span>{unnamed}% of these changes have no named actor. Setting up hooks for the agents listed below lets Tracewise credit their edits exactly; editors and background writes stay unknown.</span>
        </div>
      {/if}
    {/if}
    {#if app.doctor && (noHooks.length > 0 || partial.length > 0)}
      <div class="attr-hooks font-mono text-xs">
        {#if noHooks.length > 0}
          <p>No hooks yet for <strong>{noHooks.join(", ")}</strong>: their edits cannot be credited as exact. Attach them from the Now tab.</p>
        {/if}
        {#each partial as [name, events] (name)}
          <p><strong>{name}</strong> is missing {events.join(", ")}: part of its work will not be credited exactly.</p>
        {/each}
      </div>
    {/if}
  </section>

  <section class="card mb-4" id="health-maintenance">
    <h2 class="card-title font-mono">Maintenance</h2>
    <p class="section-sub">Each button runs the matching <code>tracewise</code> command as you and reports the result.</p>
    <div class="quick-chips mt-3">
      <button type="button" class="btn btn-secondary btn-sm font-mono" disabled={app.busy !== null} onclick={() => fx.maintain("Verify history", ["verify", "--json"])}>Verify history</button>
      <button type="button" class="btn btn-secondary btn-sm font-mono" disabled={app.busy !== null} onclick={() => fx.maintain("Compact history", ["gc", "--json"])}>Compact (gc)</button>
      <button type="button" class="btn btn-secondary btn-sm font-mono" disabled={app.busy !== null} onclick={() => fx.maintain("Start daemon", ["daemon", "start"])}>Start daemon</button>
      <button type="button" class="btn btn-secondary btn-sm font-mono" disabled={app.busy !== null} onclick={() => { if (fx.arm("daemon:stop")) fx.maintain("Stop daemon", ["daemon", "stop"]); }}>
        {app.armed === "daemon:stop" ? "Click again to stop" : "Stop daemon"}
      </button>
    </div>
    {#if app.busy}<p class="font-mono text-xs text-muted mt-2">Running: {app.busy}…</p>{/if}
  </section>

  <section class="card" id="health-cli">
    <div class="card-header-flex">
      <div>
        <h2 class="card-title font-mono">IPC CLI Runner</h2>
        <p class="section-sub">Verified allow-listed commands executed outside agent shells:</p>
      </div>
      <div class="quick-chips">
        <button type="button" class="chip font-mono" onclick={() => fx.runCustomCommand("status --json")}>status --json</button>
        <button type="button" class="chip font-mono" onclick={() => fx.runCustomCommand("doctor --json")}>doctor --json</button>
        <button type="button" class="chip font-mono" onclick={() => fx.runCustomCommand("verify --json")}>verify --json</button>
      </div>
    </div>

    <div class="cmd-run-bar mt-3">
      <span class="cmd-tracewise-lbl font-mono font-bold">tracewise</span>
      <input
        type="text"
        class="cmd-run-input font-mono"
        bind:value={app.customCmd}
        onkeydown={(e) => e.key === "Enter" && fx.runCustomCommand()}
      />
      <button
        type="button"
        class="btn btn-primary btn-sm font-mono"
        disabled={app.cmdLoading}
        onclick={() => fx.runCustomCommand()}
      >
        {app.cmdLoading ? "…" : "Run"}
      </button>
    </div>

    {#if app.cmdError}
      <div class="cmd-err-box font-mono mt-3">{app.cmdError}</div>
    {/if}

    {#if app.cmdOutput}
      <div class="cmd-out-box mt-3 font-mono">
        <div class="cmd-out-top">
          <span class="tabular-nums">Exit code: {app.cmdExitCode ?? 0}</span>
          <button type="button" class="btn-ghost" onclick={() => (app.cmdOutput = "")}>Clear</button>
        </div>
        <pre class="cmd-pre">{app.cmdOutput}</pre>
      </div>
    {/if}
  </section>
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
  .health-nav {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 12px;
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
  .btn-ghost {
    background: transparent;
    border: none;
    color: var(--muted-foreground);
    cursor: pointer;
    padding: 2px;
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
  .badge-subtle {
    background: var(--muted);
    color: var(--muted-foreground);
  }
  .doctor-matrix-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 8px;
    margin-top: 10px;
  }
  .doctor-cell {
    padding: 10px;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 5px;
  }
  .doctor-cell {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
  }
  .doctor-cell.cell-ok {
    border-color: var(--border-accent);
  }
  .health-view-container {
    overflow-y: auto;
  }
  .quick-chips {
    display: flex;
    gap: 4px;
  }
  .chip {
    padding: 2px 6px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--bg-subtle);
    color: var(--muted-foreground);
    font-size: 10px;
    cursor: pointer;
  }
  .cmd-run-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 4px 8px;
  }
  .cmd-tracewise-lbl {
    color: var(--accent);
  }
  .cmd-run-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--foreground);
    font-size: 11px;
  }
  .cmd-err-box {
    padding: 8px;
    background: var(--destructive-dim);
    color: var(--destructive);
    border-radius: 5px;
    font-size: 11px;
  }
  .cmd-out-box {
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 8px;
  }
  .cmd-out-top {
    display: flex;
    justify-content: space-between;
    font-size: 10px;
    color: var(--muted-foreground);
    margin-bottom: 4px;
  }
  .cmd-pre {
    margin: 0;
    max-height: 220px;
    overflow: auto;
    font-size: 11px;
  }
  .attr-note {
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--muted-foreground);
  }
  .attr-headline {
    margin: 12px 0 8px;
    font-size: 13px;
  }
  .attr-bar {
    display: flex;
    gap: 2px;
    height: 12px;
    border-radius: 6px;
    overflow: hidden;
    background: var(--muted);
  }
  .attr-seg {
    min-width: 4px;
  }
  .attr-swatch {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 2px;
    margin-right: 6px;
    vertical-align: -1px;
  }
  .seg-exact { background: var(--accent); }
  .seg-likely { background: var(--ring); }
  .seg-ambiguous { background: var(--warning); }
  .seg-unknown { background: var(--muted-foreground); }
  .attr-legend {
    list-style: none;
    margin: 10px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
  }
  .attr-warn {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-top: 12px;
    padding: 8px 10px;
    border: 1px solid var(--warning);
    border-radius: 6px;
    background: var(--warning-dim);
    font-size: 12px;
  }
  .attr-hooks {
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
    color: var(--muted-foreground);
  }
  .alert-list {
    list-style: none;
    margin: 12px 0 0;
    padding: 8px 10px;
    border: 1px solid var(--warning);
    border-radius: 6px;
    background: var(--warning-dim);
    font-size: 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .attr-hooks p {
    margin: 0 0 4px;
  }
</style>
