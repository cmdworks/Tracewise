<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import { linesFromChange, survivalNote } from "$lib/blame-view";
  import { app } from "$lib/app/state.svelte";
  import * as fx from "$lib/app/fx";
  import BlameWhy from "$lib/panels/BlameWhy.svelte";
  import VirtualList from "$lib/VirtualList.svelte";

  let fileIdx = $derived(app.distinctFiles.findIndex((f) => f.path === app.selectedFilePath));

  // the selected file's path in two parts: the folder (dim) and the name (strong)
  let pathParts = $derived(fx.splitFilePath(app.selectedFilePath || ""));
  // history is newest first; the stepper shows oldest on the left, newest on the right
  let history = $derived(app.selectedFileHistory);
  let lastIndex = $derived(Math.max(0, history.length - 1));
  let picked = $derived(history[app.fileVersionIndex]);
  let pickedActor = $derived(picked ? fx.getActorInfo(picked.actor) : null);
  // the slider picks a change; the lines stay the file as it is now, so show which of them that change still owns
  let pickedLines = $derived(new Set(linesFromChange(app.blameLines, picked?.id)));
  let note = $derived(picked ? survivalNote(pickedLines.size, app.blameLines.length) : "");
  let scrollEl = $state<HTMLDivElement>();

  // bring the first line of the picked change into view when the pick changes
  $effect(() => {
    const first = Math.min(...pickedLines);
    if (!scrollEl || !Number.isFinite(first)) return;
    scrollEl.querySelector<HTMLElement>(`[data-line="${first}"]`)?.scrollIntoView({ block: "nearest" });
  });
  let isBinary = $derived.by(() => {
    if (app.blameLines.some((l) => l.text && l.text.includes("\u0000"))) return true;
    if (history.length > 0) {
      const latest = history[0];
      if ((latest.before || latest.after) && (!latest.hunks || latest.hunks.length === 0)) {
        return true;
      }
    }
    return false;
  });

  function step(delta: number) {
    app.fileVersionIndex = Math.min(lastIndex, Math.max(0, app.fileVersionIndex + delta));
  }
</script>

<div class="files-workspace">
  <!-- Left: the files -->
  <aside class="files-sidebar" aria-label="Files">
    <div class="files-sidebar-header">
      <label class="sr-only" for="files-filter">Filter files</label>
      <input id="files-filter" type="text" class="files-filter-input font-mono" placeholder="Filter files…" bind:value={app.fileSearchQuery} />
      <p class="files-scope text-muted">
        {app.distinctFiles.length} {app.distinctFiles.length === 1 ? "file" : "files"} in the newest {app.changes.length} events
        {#if app.moreToLoad}
          <button type="button" class="link-btn font-mono" onclick={fx.loadMoreEvents}>Load more</button>
        {/if}
      </p>
    </div>
    <VirtualList items={app.distinctFiles} key={(f) => f.path} selectedIndex={fileIdx}>
      {#snippet row(f)}
        {@const parts = fx.splitFilePath(f.path)}
        <button
          type="button"
          data-vrow
          class="file-item-btn font-mono"
          class:active={app.selectedFilePath === f.path}
          aria-current={app.selectedFilePath === f.path ? "true" : undefined}
          title={f.path}
          onclick={() => fx.openFileBlame(f.path)}
        >
          <div class="file-item-meta">
            <span class="path-base font-bold">{parts.base}</span>
            <span class="path-dir text-xs"><bdi>{parts.dir}</bdi></span>
          </div>
          <span class="file-badge tabular-nums" title="{f.count} changes among the loaded events">{f.count}</span>
        </button>
      {/snippet}
    </VirtualList>
  </aside>

  <!-- Right: who wrote each line -->
  <div class="blame-view-pane">
    <div class="blame-view-header">
      <div class="blame-title-row">
        <span class="badge badge-subtle font-mono">LINE PROVENANCE</span>
        <h2 class="blame-file-name font-mono" title={app.selectedFilePath}>
          {#if app.selectedFilePath}
            {#if pathParts.dir}<span class="blame-dir">{pathParts.dir}</span>{/if}<strong>{pathParts.base}</strong>
          {:else}
            Select a file
          {/if}
        </h2>
        {#if app.selectedFilePath}
          <button type="button" class="btn btn-secondary btn-xs font-mono nowrap wrap-toggle" aria-pressed={app.blameWrap} onclick={fx.toggleBlameWrap}>
            Wrap lines
          </button>
        {/if}
      </div>

      {#if history.length > 0}
        <div class="history-row font-mono" role="group" aria-label="This file's changes">
          <span class="history-label tabular-nums">Change {history.length - app.fileVersionIndex} of {history.length}</span>
          <button type="button" class="step-btn" aria-label="Older change" disabled={app.fileVersionIndex >= lastIndex} onclick={() => step(1)}>
            <Icon name="chevron-left" size={14} />
          </button>
          <input
            type="range"
            class="history-slider"
            aria-label="Step through this file's changes, oldest to newest"
            min="0"
            max={lastIndex}
            value={lastIndex - app.fileVersionIndex}
            oninput={(e) => (app.fileVersionIndex = lastIndex - Number(e.currentTarget.value))}
          />
          <button type="button" class="step-btn" aria-label="Newer change" disabled={app.fileVersionIndex <= 0} onclick={() => step(-1)}>
            <Icon name="chevron-right" size={14} />
          </button>
          {#if picked && pickedActor}
            <span class="history-meta text-muted" title={picked.why || ""}>
              <span class="history-actor {pickedActor.colorClass}">{pickedActor.name}</span>
              · {fx.formatTime(picked.time?.start)}{picked.why ? ` · “${picked.why}”` : ""}
            </span>
            <button
              type="button"
              class="btn btn-secondary btn-xs font-mono nowrap"
              onclick={() => {
                fx.selectChange(picked);
                app.activeTab = "timeline";
              }}
            >
              Open in timeline
            </button>
          {/if}
        </div>
      {/if}
    </div>

    {#if app.blameLoading}
      <div class="loading-state font-mono">Calculating line-by-line blame…</div>
    {:else if app.blameError}
      <div class="empty-state font-mono"><p>{app.blameError}</p></div>
    {:else if app.blameLines.length === 0}
      <div class="empty-state font-mono"><p>Select a file to view attribution.</p></div>
    {:else if isBinary}
      <div class="empty-state font-mono">
        <p>Binary file, no line-by-line blame available.</p>
        {#if picked && pickedActor}
          <p class="text-muted text-xs mt-2">
            Last changed by <span class="font-bold {pickedActor.colorClass}">{pickedActor.name}</span>
            ({picked.confidence || "exact"})
            {#if picked.time?.start}· {fx.formatTime(picked.time.start)}{/if}
          </p>
        {/if}
      </div>
    {:else}
      {#if note}
          <div class="survival-note text-12" class:none-left={pickedLines.size === 0} aria-live="polite">{note}</div>
      {/if}
      <div class="blame-split-view">
        <div class="blame-lines-scroll font-mono" class:no-wrap={!app.blameWrap} class:focusing={pickedLines.size > 0 && pickedLines.size < app.blameLines.length} bind:this={scrollEl}>
          {#each app.blameLines as bl, i (bl.line)}
            {@const act = fx.getActorInfo(bl.actor)}
            {@const runStart = i === 0 || app.blameLines[i - 1].change !== bl.change}
            <button
              type="button"
              class="blame-line-row {act.colorClass}"
              class:run-start={runStart}
              class:selected={app.selectedBlameLine === bl.line}
              class:from-picked={pickedLines.has(bl.line)}
              data-line={bl.line}
              aria-label="Line {bl.line}, {act.name}{bl.why ? `, ${bl.why}` : ''}"
              onclick={() => fx.inspectBlameLine(bl.line)}
            >
              <span class="blame-num tabular-nums">{bl.line}</span>
              <span class="blame-prov">
                {#if runStart}
                  <span class="blame-actor {act.colorClass}">
                    <Icon name={act.icon} size={10} />
                    <span>{act.name}</span>
                  </span>
                  <span class="blame-why-text" class:no-reason={!bl.why} title={bl.why || (bl.change ? `no reason recorded (${bl.change})` : "")}>{bl.why || "no reason"}</span>
                {/if}
              </span>
              <span class="blame-code">{bl.text}</span>
            </button>
          {/each}
        </div>

        {#if app.selectedBlameLine !== null}
          <BlameWhy />
        {/if}
      </div>
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
  .text-warning {
    color: var(--warning);
  }
  .text-muted {
    color: var(--muted-foreground);
  }
  .text-xs {
    font-size: 11px;
  }
  /* a long folder path keeps its end (the part that tells folders apart); the full path is the button's title */
  .path-dir {
    color: var(--muted-foreground);
    direction: rtl;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path-dir bdi {
    direction: ltr;
    unicode-bidi: isolate;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    padding: 0;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
  .files-scope {
    margin: 6px 2px 0;
    font-size: 11px;
    line-height: 1.4;
  }
  .link-btn {
    padding: 0;
    border: none;
    background: none;
    color: var(--ring);
    font-size: 11px;
    cursor: pointer;
    text-decoration: underline;
  }
  .path-base {
    color: var(--foreground);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
  .actor-cursor {
    background: rgba(6, 182, 212, 0.12);
    color: #06b6d4;
  }
  .actor-antigravity {
    background: rgba(236, 72, 153, 0.12);
    color: #ec4899;
  }
  .actor-unknown {
    background: rgba(148, 163, 184, 0.12);
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
  .btn-xs {
    padding: 2px 6px;
    font-size: 10px;
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
  .badge-subtle {
    background: var(--muted);
    color: var(--muted-foreground);
  }
  .files-workspace {
    flex: 1;
    display: grid;
    grid-template-columns: 240px 1fr;
    gap: 10px;
    min-height: 0;
    overflow: hidden;
  }
  .files-sidebar {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .files-sidebar-header {
    padding: 6px;
    border-bottom: 1px solid var(--border);
  }
  .files-filter-input {
    width: 100%;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 4px 6px;
    color: var(--foreground);
    font-size: 11px;
  }
  .file-item-btn {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    width: 100%;
    padding: 6px 8px;
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--border);
    color: var(--foreground);
    text-align: left;
    cursor: pointer;
  }
  .file-item-btn:hover {
    background: var(--card-elevated);
  }
  .file-item-btn.active {
    background: var(--accent-dim);
    border-left: 2px solid var(--accent);
  }
  .file-item-meta {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .file-badge {
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 999px;
    background: var(--muted);
    color: var(--muted-foreground);
  }
  .blame-view-pane {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .blame-view-header {
    padding: 10px 12px 8px;
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: var(--bg-subtle);
    min-width: 0;
  }
  .blame-title-row {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }
  .blame-file-name {
    margin: 0;
    font-size: 13px;
    font-weight: 400;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .blame-file-name strong {
    font-weight: 600;
  }
  .blame-dir {
    color: var(--muted-foreground);
  }
  .history-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    min-width: 0;
  }
  .history-label {
    font-size: 11px;
    white-space: nowrap;
  }
  .history-slider {
    flex: 1 1 120px;
    min-width: 80px;
    max-width: 280px;
  }
  .history-meta {
    font-size: 11px;
    flex: 1 1 160px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .history-actor {
    padding: 0 4px;
    border-radius: 3px;
  }
  .step-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--card);
    color: var(--foreground);
    cursor: pointer;
  }
  .step-btn:hover:not(:disabled) {
    background: var(--card-elevated);
  }
  .step-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .nowrap {
    white-space: nowrap;
  }
  .wrap-toggle {
    margin-left: auto;
    flex: none;
  }
  .no-reason {
    font-style: italic;
    opacity: 0.8;
  }
  .btn[aria-pressed="true"] {
    background: var(--accent-dim);
    border-color: var(--border-accent);
  }
  .blame-split-view {
    flex: 1;
    display: flex;
    min-height: 0;
    overflow: hidden;
  }
  .blame-lines-scroll {
    flex: 1;
    min-width: 0;
    overflow: auto;
    font-size: 12px;
    line-height: 1.5;
  }
  /* one block per change: the who and why show on its first line, a coloured bar runs down all of its lines */
  .blame-line-row {
    --bar: var(--actor-unknown);
    display: grid;
    grid-template-columns: 40px minmax(120px, 210px) minmax(0, 1fr);
    gap: 10px;
    align-items: start;
    width: 100%;
    min-height: 24px;
    padding: 1px 10px 1px 8px;
    border: none;
    border-left: 3px solid var(--bar);
    background: transparent;
    color: var(--foreground);
    text-align: left;
    cursor: pointer;
  }
  .blame-line-row.run-start {
    border-top: 1px solid var(--border);
  }
  .blame-line-row.actor-human { --bar: var(--actor-you); }
  .blame-line-row.actor-claude { --bar: var(--actor-claude); }
  .blame-line-row.actor-codex { --bar: var(--actor-codex); }
  .blame-line-row.actor-gemini { --bar: var(--actor-gemini); }
  .blame-line-row.actor-cursor { --bar: var(--actor-cursor); }
  .blame-line-row.actor-antigravity { --bar: var(--actor-antigravity); }
  .survival-note {
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
    color: var(--muted-foreground);
    font-family: var(--font-ui);
  }
  .survival-note.none-left {
    color: var(--foreground);
  }
  /* lines from the picked change stand out, the rest step back */
  .blame-lines-scroll.focusing .blame-line-row:not(.from-picked) {
    opacity: 0.45;
  }
  .blame-line-row.from-picked {
    background: var(--accent-dim);
  }
  .blame-line-row:hover {
    background: var(--card-elevated);
  }
  .blame-line-row:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }
  .blame-line-row.selected {
    background: rgba(56, 189, 248, 0.1);
  }
  .no-wrap .blame-line-row {
    width: max-content;
    min-width: 100%;
  }
  .blame-num {
    color: var(--muted-foreground);
    text-align: right;
  }
  .blame-prov {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .blame-actor {
    display: inline-flex;
    align-items: center;
    flex: none;
    gap: 3px;
    padding: 0 5px;
    border-radius: 3px;
    font-size: 10px;
    white-space: nowrap;
  }
  .blame-why-text {
    font-size: 11px;
    color: var(--muted-foreground);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .blame-code {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    min-width: 0;
  }
  .no-wrap .blame-code {
    white-space: pre;
    overflow-wrap: normal;
  }  .why-sidebar-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--border);
  }  .why-kv {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 36px 16px;
    color: var(--muted-foreground);
    text-align: center;
  }
  .loading-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 36px 16px;
    color: var(--muted-foreground);
    text-align: center;
  }
</style>
