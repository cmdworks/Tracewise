<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import Popover from "$lib/Popover.svelte";
  import TimelineInspector from "$lib/panels/TimelineInspector.svelte";
  import { app } from "$lib/app/state.svelte";
  import * as fx from "$lib/app/fx";
  import VirtualList from "$lib/VirtualList.svelte";
  import {
    shortenPathFront,
    formatEventRelativeTime,
    flattenTimelineWithHeaders,
    headerAt,
    opLetter,
    type TimelineListItem,
  } from "$lib/timeline-view";

  type FooterItem = { kind: "footer"; id: string };
  type Item = TimelineListItem | FooterItem;

  const OPS = ["all", "modify", "create", "delete", "rename"];

  let showFilters = $state(false);
  let topIndex = $state(0);

  let hasMore = $derived(app.timelineCapped || app.moreToLoad);
  let listItems = $derived.by<Item[]>(() => {
    const rows: Item[] = flattenTimelineWithHeaders(app.filteredChanges);
    // paging sits at the end of the list, where the eye already is when it runs out
    if (rows.length > 0 && hasMore && !app.searchActive) rows.push({ kind: "footer", id: "footer" });
    return rows;
  });
  let selectedIdx = $derived(
    app.selectedChange ? listItems.findIndex((it) => it.kind === "change" && it.change.id === app.selectedChange!.id) : -1
  );
  let pinned = $derived(topIndex > 0 ? headerAt(listItems as TimelineListItem[], topIndex) : null);

  let filtersOn = $derived(app.actorFilter !== "all" || app.opFilter !== "all" || app.onlyNoReason || app.searchActive);
  let filterCount = $derived((app.actorFilter !== "all" ? 1 : 0) + (app.opFilter !== "all" ? 1 : 0));
  let actorLabel = $derived(app.timelineActorOptions.find((a) => a.id === app.actorFilter)?.label ?? app.actorFilter);

  function lockFor(path: string) {
    if (!path || path === ".") return null;
    return app.locks.find((l) => l.path === path) || null;
  }

  function clearFilters() {
    app.actorFilter = "all";
    app.opFilter = "all";
    app.onlyNoReason = false;
  }
</script>

<div class="timeline-root">
  <!-- One toolbar row: what this is, how many, and the filters that are on -->
  <div class="timeline-toolbar">
    <h1 class="toolbar-title text-14">
      <span class="font-semibold">Timeline</span>
      <span class="toolbar-count text-12 text-muted" aria-live="polite">
        {#if app.searchActive}
          {app.searchNote}
        {:else if filtersOn}
          {app.filteredChanges.length} of {app.changes.length} loaded
        {:else if app.status}
          {app.status.changes.toLocaleString()} {app.status.changes === 1 ? "change" : "changes"}
        {/if}
      </span>
    </h1>

    <div class="toolbar-filters">
      {#if app.actorFilter !== "all"}
        <button type="button" class="active-chip text-12" onclick={() => (app.actorFilter = "all")} title="Remove this filter">
          <span>{actorLabel}</span><Icon name="x" size={11} />
        </button>
      {/if}
      {#if app.opFilter !== "all"}
        <button type="button" class="active-chip text-12" onclick={() => (app.opFilter = "all")} title="Remove this filter">
          <span>{app.opFilter}</span><Icon name="x" size={11} />
        </button>
      {/if}

      <!-- the one coloured control: it is the thing that needs attention -->
      <button
        type="button"
        class="reason-chip text-12"
        class:active={app.onlyNoReason}
        aria-pressed={app.onlyNoReason}
        onclick={() => (app.onlyNoReason = !app.onlyNoReason)}
        title="Show only changes without a reason"
      >
        <span class="amber-dot"></span>
        <span class="reason-chip-label">Needs a reason</span>
        {#if app.pendingChanges.length > 0}
          <span class="font-mono">{app.pendingChanges.length}{app.pendingMore ? "+" : ""}</span>
        {/if}
      </button>

      <Popover bind:open={showFilters} onclose={() => (showFilters = false)} align="right" width="240px">
        {#snippet trigger()}
          <button
            type="button"
            class="filter-btn text-12"
            class:on={filterCount > 0}
            aria-expanded={showFilters}
            onclick={() => (showFilters = !showFilters)}
          >
            <Icon name="filter" size={13} />
            <span class="filter-btn-label">Filter</span>
            {#if filterCount > 0}<span class="filter-badge">{filterCount}</span>{/if}
          </button>
        {/snippet}
        {#snippet children()}
          <div class="filter-menu text-13">
            <div class="filter-group-label text-12 text-muted">Who</div>
            {#each app.timelineActorOptions as a (a.id)}
              <label class="filter-option">
                <input type="radio" name="tl-actor" value={a.id} bind:group={app.actorFilter} />
                <span>{a.label}</span>
              </label>
            {/each}
            <div class="filter-group-label text-12 text-muted">Operation</div>
            {#each OPS as op (op)}
              <label class="filter-option">
                <input type="radio" name="tl-op" value={op} bind:group={app.opFilter} />
                <span>{op === "all" ? "All" : op}</span>
              </label>
            {/each}
            {#if filtersOn && !app.searchActive}
              <button type="button" class="clear-btn text-12" onclick={clearFilters}>Clear filters</button>
            {/if}
          </div>
        {/snippet}
      </Popover>
    </div>
  </div>

  <div class="timeline-split-layout">
    <div class="timeline-list-pane">
      {#if app.newChangesPillCount > 0}
        <button
          type="button"
          class="new-changes-pill text-12"
          onclick={() => {
            app.newChangesPillCount = 0;
            fx.fetchTimeline();
          }}
        >
          <Icon name="chevron-up" size={12} />
          {app.newChangesPillCount} new {app.newChangesPillCount === 1 ? "change" : "changes"}
        </button>
      {/if}

      {#if pinned}
        <div class="day-row day-pinned" aria-hidden="true">
          <span class="day-label text-12">{pinned.label}</span>
          <span class="text-12 text-muted">{pinned.count}</span>
        </div>
      {/if}

      {#if app.timelineLoading && app.changes.length === 0}
        <div class="empty-state text-13 text-muted">Loading changes…</div>
      {:else if app.filteredChanges.length === 0}
        <div class="empty-state text-13 text-muted">
          {#if app.loadProblem.timeline}
            <p>Could not load changes.</p>
          {:else if filtersOn}
            <p>No changes match.</p>
            {#if !app.searchActive}<button type="button" class="clear-btn text-12" onclick={clearFilters}>Clear filters</button>{/if}
          {:else}
            <p>No changes recorded yet.</p>
          {/if}
        </div>
      {:else}
        <VirtualList items={listItems} key={(it) => it.id} selectedIndex={selectedIdx} ontop={(i) => (topIndex = i)}>
          {#snippet row(item)}
            {#if item.kind === "header"}
              <div class="day-row" data-vrow>
                <span class="day-label text-12">{item.label}</span>
                <span class="text-12 text-muted">{item.count}</span>
              </div>
            {:else if item.kind === "footer"}
              <div class="day-row list-end text-12 text-muted" data-vrow>
                {#if app.timelineCapped}
                  <span>Newest {app.changes.length.toLocaleString()} of {app.status?.changes.toLocaleString()}; search reaches the rest</span>
                {:else}
                  <span>Newest {app.changes.length.toLocaleString()} of {app.status?.changes.toLocaleString()}</span>
                  <button type="button" class="link-btn text-12" onclick={fx.loadMoreEvents}>Load {fx.TIMELINE_PAGE} more</button>
                {/if}
              </div>
            {:else}
              {@const c = item.change}
              {@const act = fx.getActorInfo(c.actor)}
              {@const lock = lockFor(c.path)}
              {@const isBaseline = c.path === "." || c.op === "baseline"}
              <button
                type="button"
                data-vrow
                class="timeline-row text-13"
                class:selected={app.selectedChange?.id === c.id}
                onclick={() => {
                  app.inspectorVisible = true;
                  fx.selectChange(c);
                }}
              >
                <span class="col-time text-12 text-muted" title={fx.formatTime(c.time.start)}>
                  {formatEventRelativeTime(c.time.start)}
                </span>
                <span class="col-actor text-12" title="{act.name}{c.confidence ? ` (${c.confidence})` : ''}">
                  <span class="actor-initial" aria-hidden="true">{act.name.charAt(0).toUpperCase()}</span>
                  <span class="actor-name truncate">{act.name}</span>
                </span>
                <span class="col-op font-mono text-12" title={c.op}>{opLetter(c.op)}</span>
                <span class="col-path font-mono text-12 truncate" title={isBaseline ? "Project baseline" : c.old_path ? `${c.old_path} → ${c.path}` : c.path}>
                  {#if isBaseline}<span class="text-muted">(project baseline)</span>{:else}{shortenPathFront(c.path)}{/if}
                </span>
                <span class="col-reason truncate">
                  {#if c.why}
                    <span class="text-muted" title={c.why}>{c.why.replace(/\s+/g, " ")}</span>
                  {:else if isBaseline}
                    <span class="text-muted">Initial project baseline</span>
                  {:else if c.command}
                    <span class="font-mono text-12 text-muted" title={c.command}>$ {c.command.replace(/\s+/g, " ")}</span>
                  {:else}
                    <span class="needs-reason text-12"><span class="amber-dot"></span>Needs a reason</span>
                  {/if}
                </span>
                {#if lock}
                  <span class="col-lock" title="Locked by {fx.whoText(lock.holder)} until {fx.formatTime(lock.expires || '')}">
                    <Icon name="lock" size={12} />
                  </span>
                {/if}
              </button>
            {/if}
          {/snippet}
        </VirtualList>
      {/if}
    </div>

    {#if app.inspectorVisible && app.selectedChange}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        class="split-handle"
        class:dragging={app.isDraggingSplit}
        role="separator"
        aria-orientation="vertical"
        tabindex="0"
        onmousedown={fx.startDragSplit}
        onkeydown={(e) => {
          if (e.key === "ArrowLeft") app.inspectorWidth = Math.min(800, app.inspectorWidth + 24);
          if (e.key === "ArrowRight") app.inspectorWidth = Math.max(300, app.inspectorWidth - 24);
        }}
      ></div>
      <TimelineInspector />
    {/if}
  </div>
</div>

<style>
  .timeline-root {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    min-width: 0;
    container: timeline / inline-size;
  }

  /* toolbar */
  .timeline-toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 44px;
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
    min-width: 0;
  }
  .toolbar-title {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 0;
    flex: 1 1 auto;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
  }
  .toolbar-count {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .toolbar-filters {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .active-chip,
  .filter-btn,
  .reason-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: transparent;
    color: var(--foreground);
    cursor: pointer;
    white-space: nowrap;
  }
  .active-chip {
    background: var(--card-elevated);
  }
  .active-chip:hover,
  .filter-btn:hover {
    background: var(--surface-container-high);
  }
  .filter-btn.on {
    border-color: var(--border-strong);
  }
  .filter-badge {
    min-width: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--accent);
    color: #fff;
    text-align: center;
  }
  .reason-chip {
    color: var(--warning);
  }
  .reason-chip.active {
    background: var(--warning-dim);
    border-color: var(--warning-border);
  }
  .amber-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--warning);
    flex-shrink: 0;
  }
  .filter-menu {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px;
  }
  .filter-group-label {
    padding: 6px 4px 2px;
  }
  .filter-group-label:first-child {
    padding-top: 0;
  }
  .filter-option {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px;
    border-radius: 4px;
    cursor: pointer;
    text-transform: none;
  }
  .filter-option:hover {
    background: var(--card-elevated);
  }
  .clear-btn,
  .link-btn {
    background: none;
    border: none;
    padding: 4px;
    color: var(--accent);
    cursor: pointer;
    text-align: left;
  }
  .clear-btn:hover,
  .link-btn:hover {
    text-decoration: underline;
  }

  /* list */
  .timeline-split-layout {
    display: flex;
    flex: 1;
    min-height: 0;
    position: relative;
  }
  .timeline-list-pane {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    position: relative;
  }
  .new-changes-pill {
    position: absolute;
    top: 46px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 3;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 26px;
    padding: 0 10px;
    border: 1px solid var(--border-strong);
    border-radius: 13px;
    background: var(--card-elevated);
    color: var(--foreground);
    cursor: pointer;
  }

  /* every row, header and footer is 38px: the virtual list needs equal heights */
  .day-row,
  .timeline-row {
    height: 38px;
    box-sizing: border-box;
  }
  .day-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    background: var(--bg);
    border-bottom: 1px solid var(--border);
  }
  .day-label {
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--muted-foreground);
  }
  .day-pinned {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 2;
  }
  .list-end {
    justify-content: center;
    border-bottom: none;
  }

  .timeline-row {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 0 12px;
    border: none;
    border-bottom: 1px solid var(--border);
    background: transparent;
    color: var(--foreground);
    text-align: left;
    cursor: pointer;
    overflow: hidden;
    white-space: nowrap;
  }
  .timeline-row:hover {
    background: var(--card-elevated);
  }
  .timeline-row.selected {
    background: var(--accent-dim);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .col-time {
    flex: 0 0 76px;
  }
  .col-actor {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 0 0 110px;
    min-width: 0;
    color: var(--muted-foreground);
  }
  .actor-initial {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--surface-container-high);
    color: var(--foreground);
    font-size: 12px;
    flex-shrink: 0;
  }
  .col-op {
    flex: 0 0 14px;
    color: var(--muted-foreground);
  }
  .col-path {
    flex: 0 1 260px;
    min-width: 80px;
  }
  .col-reason {
    flex: 1 1 auto;
    min-width: 0;
  }
  .needs-reason {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--warning);
  }
  .col-lock {
    display: flex;
    flex-shrink: 0;
    color: var(--destructive);
  }
  .truncate {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    flex: 1;
    padding: 24px;
  }
  .empty-state p {
    margin: 0;
  }

  .split-handle {
    width: 4px;
    flex-shrink: 0;
    cursor: col-resize;
    background: transparent;
  }
  .split-handle:hover,
  .split-handle.dragging,
  .split-handle:focus-visible {
    background: var(--accent-dim);
  }

  /* Width follows the panel, not the window.
     900 and up: list and inspector side by side (actors shrink to initials under 1100).
     640 to 899: the inspector slides over the list from the right.
     under 640: the inspector takes the whole panel, with Back (in the inspector). */
  @container timeline (max-width: 1099px) {
    .actor-name {
      display: none;
    }
    .col-actor {
      flex-basis: 18px;
    }
  }
  @container timeline (max-width: 899px) {
    .split-handle {
      display: none;
    }
    .timeline-split-layout > :global(.inspector-pane) {
      position: absolute;
      top: 0;
      right: 0;
      bottom: 0;
      z-index: 4;
      width: min(480px, 85%) !important;
      max-width: none;
      box-shadow: -8px 0 24px rgba(0, 0, 0, 0.25);
    }
    .col-path {
      flex-basis: 220px;
    }
  }
  @container timeline (max-width: 639px) {
    .timeline-split-layout > :global(.inspector-pane) {
      width: 100% !important;
      box-shadow: none;
      border-left: none;
    }
    .col-time {
      flex-basis: 64px;
    }
    .col-op {
      display: none;
    }
    .reason-chip-label,
    .filter-btn-label {
      display: none;
    }
  }
  @container timeline (max-width: 520px) {
    .col-reason {
      display: none;
    }
    .col-path {
      flex: 1 1 auto;
    }
  }
</style>
