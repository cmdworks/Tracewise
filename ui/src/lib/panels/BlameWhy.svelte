<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import { app } from "$lib/app/state.svelte";
  import * as fx from "$lib/app/fx";
</script>

<aside class="why-sidebar font-mono" aria-label="Where line {app.selectedBlameLine} came from">
  <div class="why-sidebar-top">
    <span class="font-bold text-xs">{app.selectedFilePath.split("/").pop()}:{app.selectedBlameLine}</span>
    <button type="button" class="btn-ghost" aria-label="Close" onclick={() => (app.selectedBlameLine = null)}>
      <Icon name="x" size={13} />
    </button>
  </div>
  {#if app.lineWhyLoading}
    <p class="text-xs text-muted">Loading provenance…</p>
  {:else if app.lineWhyData?.change}
    {@const ch = app.lineWhyData.change}
    {@const act = fx.getActorInfo(ch.actor)}
    <div class="why-rows">
      <div class="why-kv">
        <span class="text-muted">Author:</span>
        <span class="actor-chip {act.colorClass}">
          <Icon name={act.icon} size={11} />
          <span>{act.name}</span>
        </span>
      </div>
      <div class="why-kv"><span class="text-muted">Change ID:</span><span>{ch.id}</span></div>
      <div class="why-kv"><span class="text-muted">Time:</span><span class="tabular-nums">{fx.formatTime(ch.time?.start)}</span></div>
      {#if app.lineWhyData.intent?.why}
        <div class="why-kv"><span class="text-muted">Intent:</span><span class="text-warning">"{app.lineWhyData.intent.why}"</span></div>
      {/if}
      {#if ch.command}
        <div class="why-kv"><span class="text-muted">Command:</span><span>$ {ch.command}</span></div>
      {/if}
    </div>
  {/if}
</aside>

<style>
  .font-mono {
    font-family: var(--font-mono, "JetBrains Mono", monospace);
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

  .text-warning {
    color: var(--warning);
  }

  .actor-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 1px 6px;
    border-radius: 4px;
    font-size: 11px;
    white-space: nowrap;
  }

  .actor-human {
    background: rgba(56, 189, 248, 0.12);
    color: #38bdf8;
  }

  .actor-claude {
    background: rgba(249, 115, 22, 0.12);
    color: #fb923c;
  }

  .actor-codex {
    background: rgba(168, 85, 247, 0.12);
    color: #c084fc;
  }

  .actor-gemini {
    background: rgba(99, 102, 241, 0.12);
    color: #818cf8;
  }

  .actor-unknown {
    background: rgba(148, 163, 184, 0.12);
    color: var(--muted-foreground);
  }

  .btn-ghost {
    background: transparent;
    border: none;
    color: var(--muted-foreground);
    cursor: pointer;
    padding: 2px;
  }

  .why-sidebar {
    width: 260px;
    background: var(--bg-subtle);
    border-left: 1px solid var(--border);
    padding: 10px;
    overflow-y: auto;
  }

  .why-sidebar-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--border);
  }

  .why-rows {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 11px;
  }

  .why-kv {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
</style>
