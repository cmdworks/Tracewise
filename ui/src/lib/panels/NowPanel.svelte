<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import { app } from "$lib/app/state.svelte";
  import * as fx from "$lib/app/fx";
  import { groupPendingChanges, type PendingGroup } from "$lib/now-grouping";
  import { shortenPathFront, formatEventRelativeTime } from "$lib/timeline-view";

  let batchInputs = $state<Record<string, string>>({});
  let expandedGroups = $state<Record<string, boolean>>({});

  let groups = $derived(groupPendingChanges(app.pendingChanges));

  // Initialize the first group as expanded by default
  $effect(() => {
    if (groups.length > 0 && Object.keys(expandedGroups).length === 0) {
      expandedGroups[groups[0].id] = true;
    }
  });

  function toggleGroup(id: string) {
    expandedGroups[id] = !expandedGroups[id];
  }

  async function handleBatchSubmit(group: PendingGroup) {
    const text = (batchInputs[group.id] || "").trim();
    if (!text) return;
    // keep the draft when Tracewise refuses it (too short or generic), so the person can fix it
    if (await fx.submitBatchPendingNotes(group.items.map((i) => i.id), text)) batchInputs[group.id] = "";
  }
</script>

<div class="now-container select-none">
  <!-- SECTION 1: ACTIVE AGENTS & WORK -->
  <section class="now-section">
    <div class="section-header">
      <h2 class="section-title text-14 font-semibold">Active Work & Agents</h2>
    </div>

    <!-- Active Agents Grid -->
    <div class="active-agents-grid">
      {#each app.agents as ag}
        {@const act = fx.getActorInfo({ kind: ag.name, name: ag.name })}
        <div class="agent-card">
          <div class="agent-card-header">
            <div class="agent-chip-group">
              <span class="actor-pill text-12 {act.colorClass}">
                <span class="actor-initial">{act.name.charAt(0).toUpperCase()}</span>
                <span>{act.name}</span>
              </span>
              <span class="status-tag text-12 font-mono">
                {ag.hooks ? "hooks active" : ag.shims ? "shims only" : "inactive"}
              </span>
            </div>
            <button
              type="button"
              class="hook-toggle-btn text-12"
              disabled={app.busy !== null}
              onclick={() => fx.toggleAgent(ag)}
            >
              {app.armed === `agent:${ag.name}` ? "Confirm?" : ag.hooks ? "Disable" : "Install hooks"}
            </button>
          </div>

          <!-- Component Capabilities Pills -->
          <div class="agent-caps-row text-12 font-mono">
            <span class="cap-tag" class:cap-on={ag.hooks}>hooks</span>
            <span class="cap-tag" class:cap-on={ag.shims}>shims</span>
            <span class="cap-tag" class:cap-on={ag.shell}>shell</span>
            <span class="cap-tag" class:cap-on={ag.skill}>skill</span>
          </div>
        </div>
      {/each}
    </div>

    <!-- Sub-row info footer -->
    <div class="agents-status-subtext text-12 text-muted">
      <Icon name="brand" size={13} class="text-muted" />
      <span>Tracewise coordinates lock prevention and attributes changes across configured AI agents.</span>
    </div>
  </section>

  <!-- SECTION 2: LOCKS -->
  <section class="now-section">
    <div class="section-header">
      <h2 class="section-title text-14 font-semibold">
        <span>Locks</span>
        <span class="count-sub text-12 text-muted font-normal">
          ({app.locks.length} {app.locks.length === 1 ? "active" : "active"})
        </span>
      </h2>
      {#if app.locks.length > 0}
        <button
          type="button"
          class="btn-outline-danger text-12"
          disabled={app.busy !== null}
          onclick={fx.releaseAllLocks}
        >
          {app.armed === "lock:all" ? "Confirm release all?" : "Release all"}
        </button>
      {/if}
    </div>

    {#if app.locks.length === 0}
      <div class="calm-box text-12 text-muted">
        <Icon name="unlock" size={14} class="text-muted" />
        <span>No active locks. Locks are advisory and coordinate agents editing files.</span>
      </div>
    {:else}
      <div class="locks-table">
        {#each app.locks as lk (lk.path)}
          <div class="lock-row">
            <div class="lock-left min-w-0">
              <Icon name="lock" size={14} class="text-muted shrink-0" />
              <span class="lock-path font-mono text-12 truncate" title={lk.path}>
                {shortenPathFront(lk.path)}
              </span>
              <span class="lock-holder-badge text-12">
                locked by {fx.whoText(lk.holder)}
              </span>
              <span class="lock-time text-12 text-muted font-mono">
                until {fx.formatTime(lk.expires || "")}
              </span>
            </div>

            <button
              type="button"
              class="btn-outline text-12"
              disabled={app.busy !== null}
              onclick={() => fx.releaseLock(lk.path)}
            >
              {app.armed === `lock:${lk.path}` ? "Confirm release" : "Release…"}
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <!-- SECTION 3: NEEDS A REASON -->
  <section class="now-section">
    <div class="section-header">
      <div class="header-with-badge">
        <h2 class="section-title text-14 font-semibold">Needs a reason</h2>
        <span class="amber-badge text-12 font-medium">
          {app.pendingChanges.length}{app.pendingMore ? "+" : ""}
        </span>
      </div>
    </div>

    {#if app.pendingLoading && app.pendingChanges.length === 0}
      <div class="calm-box text-12 text-muted">
        <span>Loading changes without a reason…</span>
      </div>
    {:else if app.pendingChanges.length === 0}
      <div class="calm-box calm-box-green text-12">
        <Icon name="check" size={14} />
        <span>Every recorded change has a reason.</span>
      </div>
    {:else}
      <div class="groups-list">
        {#each groups as g (g.id)}
          {@const isExpanded = !!expandedGroups[g.id]}
          <div class="group-card">
            <!-- Group Header -->
            <button
              type="button"
              class="group-header-btn"
              onclick={() => toggleGroup(g.id)}
            >
              <div class="group-title-info min-w-0">
                <Icon name={isExpanded ? "chevron-down" : "chevron-right"} size={13} class="text-muted" />
                <span class="group-title text-13 font-medium">{g.title}</span>
                <span class="dot-sep">·</span>
                <span class="group-actor text-12 font-medium">{g.actor}</span>
                <span class="dot-sep">·</span>
                <span class="group-count text-12 text-muted">{g.items.length} {g.items.length === 1 ? "change" : "changes"}</span>
              </div>
            </button>

            <!-- Expanded Content: Batch Input + File list -->
            {#if isExpanded}
              <div class="group-content">
                <!-- Batch Reason Input Row -->
                <div class="batch-input-row">
                  <input
                    type="text"
                    class="batch-reason-input text-13"
                    placeholder="Describe why for all {g.items.length}…"
                    bind:value={batchInputs[g.id]}
                    onkeydown={(e) => e.key === "Enter" && handleBatchSubmit(g)}
                  />
                  <!-- Solid primary button per Stitch canvas -->
                  <button
                    type="button"
                    class="btn-primary-batch text-13 font-medium"
                    disabled={app.busy !== null || !(batchInputs[g.id] || "").trim()}
                    onclick={() => handleBatchSubmit(g)}
                  >
                    Save reason for {g.items.length} {g.items.length === 1 ? "change" : "changes"}
                  </button>
                </div>

                <!-- Files in Group -->
                <div class="group-files-list">
                  {#each g.items as it (it.id)}
                    <div class="group-file-item">
                      <div class="group-file-left min-w-0">
                        <span class="item-dot"></span>
                        <span class="group-file-path font-mono text-12 truncate" title={it.path}>
                          {shortenPathFront(it.path)}
                        </span>
                      </div>
                      <span class="group-file-time text-12 text-muted font-mono" title={fx.formatTime(it.time?.start || "")}>
                        {formatEventRelativeTime(it.time?.start || "")}
                      </span>
                      <input
                        type="text"
                        class="single-reason-input text-12"
                        placeholder="Why, for this change only"
                        aria-label="Reason for {it.path}"
                        bind:value={app.pendingNotes[it.id]}
                        onkeydown={(e) => e.key === "Enter" && fx.submitPendingNote(it.id)}
                      />
                      <button
                        type="button"
                        class="btn-outline text-12"
                        disabled={app.pendingSavingId === it.id || !(app.pendingNotes[it.id] || "").trim()}
                        onclick={() => fx.submitPendingNote(it.id)}
                      >
                        {app.pendingSavingId === it.id ? "…" : "Save"}
                      </button>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <!-- SECTION 4: BACKGROUND WORK started by agents (attributed to them until it expires) -->
  <section class="now-section">
    <div class="section-header">
      <h2 class="section-title text-14 font-semibold">
        <span>Background work</span>
        <span class="count-sub text-12 text-muted font-normal">({app.bgJobs.length})</span>
      </h2>
    </div>
    {#if app.bgJobs.length === 0}
      <div class="calm-box text-12 text-muted">
        <span>No agent has background work running.</span>
      </div>
    {:else}
      <div class="locks-table">
        {#each app.bgJobs as job (job.id)}
          <div class="lock-row">
            <div class="lock-left min-w-0">
              <span class="text-12 font-medium">{fx.whoText(job.actor)}</span>
              <span class="lock-path font-mono text-12 truncate" title={job.command || job.id}>{job.command || job.id}</span>
              <span class="lock-time text-12 text-muted font-mono">until {fx.formatTime(job.expires || "")}</span>
            </div>
            <button type="button" class="btn-outline text-12" disabled={app.busy !== null} onclick={() => fx.stopBackground(job.id)}>
              {app.armed === `bg:${job.id}` ? "Click again to stop" : "Stop attributing"}
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .single-reason-input {
    flex: 0 1 220px;
    min-width: 120px;
    padding: 3px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--foreground);
  }
  .single-reason-input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .now-container {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 14px;
    background: var(--bg);
  }

  .now-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .section-title {
    margin: 0;
    color: var(--foreground);
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .count-sub {
    color: var(--muted-foreground);
  }

  .header-with-badge {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .amber-badge {
    padding: 1px 7px;
    border-radius: 4px;
    background: var(--warning-dim);
    border: 1px solid var(--warning-border);
    color: var(--warning);
  }

  .active-agents-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 10px;
  }

  .agent-card {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .agent-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .agent-chip-group {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .actor-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--card-elevated);
    border: 1px solid var(--border);
    color: var(--foreground);
    font-weight: 500;
  }

  .actor-initial {
    width: 14px;
    height: 14px;
    border-radius: 3px;
    background: var(--surface-container-highest);
    color: var(--foreground);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-weight: 600;
  }

  .status-tag {
    color: var(--muted-foreground);
  }

  .hook-toggle-btn {
    padding: 3px 8px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--foreground);
    cursor: pointer;
    transition: background 0.12s ease;
  }

  .hook-toggle-btn:hover:not(:disabled) {
    background: var(--card-elevated);
  }

  .agent-caps-row {
    display: flex;
    gap: 4px;
  }

  .cap-tag {
    padding: 1px 6px;
    border-radius: 3px;
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--muted-foreground);
    opacity: 0.6;
  }

  .cap-tag.cap-on {
    opacity: 1;
    color: var(--foreground);
    border-color: var(--border-strong);
  }

  .agents-status-subtext {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 4px;
  }

  .calm-box {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 12px 14px;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .calm-box-green {
    background: rgba(16, 185, 129, 0.08);
    border-color: rgba(16, 185, 129, 0.25);
    color: #10b981;
  }

  .locks-table {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 4px;
  }

  .lock-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    gap: 10px;
    border-bottom: 1px solid var(--border);
  }

  .lock-row:last-child {
    border-bottom: none;
  }

  .lock-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .lock-path {
    color: var(--foreground);
    font-weight: 500;
  }

  .lock-holder-badge {
    padding: 1px 6px;
    background: var(--card-elevated);
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--foreground);
  }

  .btn-outline {
    padding: 4px 10px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--foreground);
    cursor: pointer;
    transition: background 0.12s ease;
  }

  .btn-outline:hover:not(:disabled) {
    background: var(--card-elevated);
  }

  .btn-outline-danger {
    padding: 4px 10px;
    background: var(--destructive-dim);
    border: 1px solid rgba(248, 113, 113, 0.3);
    border-radius: 4px;
    color: var(--destructive);
    cursor: pointer;
  }

  .groups-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .group-card {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 4px;
    overflow: hidden;
  }

  .group-header-btn {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    background: var(--card-elevated);
    border: none;
    cursor: pointer;
    text-align: left;
    transition: background 0.12s ease;
  }

  .group-header-btn:hover {
    background: var(--surface-container-high);
  }

  .group-title-info {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--foreground);
  }

  .dot-sep {
    color: var(--border);
  }

  .group-content {
    display: flex;
    flex-direction: column;
  }

  .batch-input-row {
    padding: 10px 12px;
    display: flex;
    align-items: center;
    gap: 10px;
    border-bottom: 1px solid var(--border);
    background: var(--card);
  }

  .batch-reason-input {
    flex: 1;
    height: 32px;
    padding: 0 10px;
    border-radius: 4px;
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--foreground);
    outline: none;
  }

  .batch-reason-input:focus {
    border-color: var(--accent);
  }

  .btn-primary-batch {
    height: 32px;
    padding: 0 14px;
    background: #3b5bdb;
    color: #ffffff;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    transition: background 0.12s ease;
    flex-shrink: 0;
  }

  .btn-primary-batch:hover:not(:disabled) {
    background: #3451c7;
  }

  .btn-primary-batch:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .group-files-list {
    display: flex;
    flex-direction: column;
    padding: 6px 12px;
  }

  .group-file-item {
    display: flex;
    align-items: center;
    gap: 8px;
    justify-content: space-between;
    padding: 6px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  }

  .group-file-item:last-child {
    border-bottom: none;
  }

  .group-file-left {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .item-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--muted-foreground);
    flex-shrink: 0;
  }

  .group-file-path {
    color: var(--foreground);
  }

  .text-12 {
    font-size: 12px;
    line-height: 16px;
  }
  .text-13 {
    font-size: 13px;
    line-height: 18px;
  }
  .text-14 {
    font-size: 14px;
    line-height: 20px;
  }
  .font-mono {
    font-family: var(--font-mono, monospace);
  }
  .text-muted {
    color: var(--muted-foreground);
  }
</style>
