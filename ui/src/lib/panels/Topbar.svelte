<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import Popover from "$lib/Popover.svelte";
  import { app } from "$lib/app/state.svelte";
  import * as fx from "$lib/app/fx";
</script>

<header class="topbar">
      <div class="topbar-left">
        <Popover bind:open={app.showRepoPopover} onclose={() => (app.showRepoPopover = false)} width="380px">
          {#snippet trigger()}
            <button
              type="button"
              class="repo-selector-btn"
              onclick={() => (app.showRepoPopover = !app.showRepoPopover)}
              title={app.currentRepo || "Choose a project"}
            >
              <Icon name="folder" size={14} class="text-muted" />
              <span class="repo-label text-13 truncate">
                {app.currentRepo
                  ? app.currentRepo.split("/").pop()
                  : app.statusLoading
                    ? "Connecting…"
                    : "No project"}
              </span>
              <Icon name={app.showRepoPopover ? "chevron-up" : "chevron-down"} size={13} class="text-muted" />
            </button>
          {/snippet}

          {#snippet children()}
            <div class="repo-popover-content">
              <div class="popover-heading font-mono">
                <span>ACTIVE REPOSITORY</span>
                {#if app.daemonPid}
                  <span class="badge badge-accent">PID {app.daemonPid}</span>
                {/if}
              </div>
              <div class="popover-sub-path font-mono">{app.currentRepo || "None selected"}</div>

              <div class="popover-search-wrap">
                <Icon name="search" size={12} class="text-muted" />
                <input
                  type="text"
                  class="popover-filter-input font-mono"
                  placeholder="Filter registered repos…"
                  bind:value={app.repoFilterQuery}
                />
              </div>

              <div class="popover-repo-list">
                {#each app.filteredWatchedRepos as r}
                  <button
                    type="button"
                    class="repo-popover-item font-mono"
                    class:active={r === app.currentRepo}
                    onclick={() => fx.switchRepo(r)}
                  >
                    <Icon name={r === app.currentRepo ? "check" : "folder"} size={13} class={r === app.currentRepo ? "text-accent" : "text-muted"} />
                    <div class="repo-popover-meta">
                      <span class="repo-popover-name font-bold">{r.split("/").pop()}</span>
                      <span class="repo-popover-full text-muted">{r}</span>
                    </div>
                  </button>
                {/each}
              </div>

              <div class="popover-custom-wrap">
                <input
                  type="text"
                  class="popover-custom-input font-mono"
                  placeholder="Path: /path/to/project"
                  bind:value={app.customRepoInput}
                  onkeydown={(e) => e.key === "Enter" && app.customRepoInput && fx.switchRepo(app.customRepoInput)}
                />
                <button
                  type="button"
                  class="btn btn-secondary btn-xs font-mono"
                  onclick={() => app.customRepoInput && fx.switchRepo(app.customRepoInput)}
                >
                  Open
                </button>
              </div>
            </div>
          {/snippet}
        </Popover>

        {#if app.activeTab === "timeline"}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="search-box" class:has-query={!!app.searchQuery} onclick={() => document.getElementById("search-input")?.focus()}>
            <Icon name="search" size={14} class="text-muted" />
            <input
              id="search-input"
              type="text"
              class="text-13"
              placeholder="Search path, reason or id"
              aria-label="Search the history"
              bind:value={app.searchQuery}
              oninput={fx.onSearchInput}
            />
            {#if app.searchQuery}
              <button
                type="button"
                class="search-clear-btn"
                aria-label="Clear search"
                onclick={() => {
                  app.searchQuery = "";
                  fx.onSearchInput();
                }}
              >
                <Icon name="x" size={12} />
              </button>
            {:else}
              <kbd class="search-kbd">/</kbd>
            {/if}
          </div>
        {/if}
      </div>

      <!-- Polling keeps the data fresh; a button appears only when it is not ("r" refreshes any time) -->
      <div class="live-status text-12" role="status" title={app.statusStale ? "No answer from Tracewise for a while" : "Updates by itself every few seconds (r refreshes now)"}>
        <span class="live-dot" class:dot-stale={app.statusStale}></span>
        <span class="live-text text-muted">{app.statusStale ? "Stale" : (app.relativeStatusTime || "Live")}</span>
        {#if app.statusStale}
          <button type="button" class="retry-btn text-12" onclick={() => fx.refreshAll()}>Retry</button>
        {/if}
      </div>
    </header>

<style>
  /* One job: where you are, what you look for, whether the data is fresh. Filters live in the panel that uses them. */
  .topbar {
    container-type: inline-size;
    display: flex;
    align-items: center;
    gap: 12px;
    height: 44px;
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
    background: var(--bg);
    min-width: 0;
  }
  .topbar-left {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1 1 auto;
    min-width: 0;
  }
  .repo-selector-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 220px;
    height: 30px;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: transparent;
    color: var(--foreground);
    cursor: pointer;
    min-width: 0;
  }
  .repo-selector-btn:hover {
    background: var(--card-elevated);
  }
  .repo-label {
    font-weight: 500;
    min-width: 0;
  }
  .search-box {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1 1 320px;
    max-width: 560px;
    min-width: 0;
    height: 30px;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-subtle);
    cursor: text;
  }
  .search-box:focus-within {
    border-color: var(--accent);
  }
  .search-box input {
    flex: 1;
    min-width: 0;
    border: none;
    outline: none;
    background: transparent;
    color: var(--foreground);
    font-family: var(--font-ui);
  }
  .search-kbd {
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 0 5px;
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--muted-foreground);
  }
  .search-clear-btn {
    display: flex;
    background: none;
    border: none;
    padding: 2px;
    color: var(--muted-foreground);
    cursor: pointer;
  }
  .live-status {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .live-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--muted-foreground);
  }
  .live-dot.dot-stale {
    background: var(--warning);
  }
  .retry-btn {
    height: 26px;
    padding: 0 10px;
    border: 1px solid var(--warning-border);
    border-radius: 4px;
    background: var(--warning-dim);
    color: var(--warning);
    cursor: pointer;
  }
  /* narrow: search shrinks to its icon until focused (it stays focusable, so "/" still works), the age text goes */
  @container (max-width: 560px) {
    .search-box:not(:focus-within):not(.has-query) {
      flex: 0 0 30px;
      padding: 0;
      justify-content: center;
    }
    .search-box:not(:focus-within):not(.has-query) input,
    .search-box:not(:focus-within):not(.has-query) .search-kbd {
      width: 0;
      flex: 0 0 0;
      opacity: 0;
    }
    .live-text {
      display: none;
    }
    .repo-selector-btn {
      max-width: 150px;
    }
  }

  .font-mono {
    font-family: var(--font-mono, "JetBrains Mono", monospace);
  }
  .font-bold {
    font-weight: 600;
  }
  .text-muted {
    color: var(--muted-foreground);
  }
  .repo-popover-content {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .popover-heading {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 10px;
    color: var(--muted-foreground);
    letter-spacing: 0.05em;
  }
  .popover-sub-path {
    font-size: 11px;
    word-break: break-all;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--border);
  }
  .popover-search-wrap {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 4px 8px;
  }
  .popover-filter-input {
    background: transparent;
    border: none;
    outline: none;
    color: var(--foreground);
    font-size: 11px;
    width: 100%;
  }
  .popover-repo-list {
    max-height: 200px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .repo-popover-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 5px;
    color: var(--foreground);
    text-align: left;
    cursor: pointer;
  }
  .repo-popover-item:hover {
    background: var(--card-elevated);
  }
  .repo-popover-item.active {
    background: var(--accent-dim);
    border-color: var(--border-accent);
  }
  .repo-popover-meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .repo-popover-name {
    font-size: 12px;
  }
  .repo-popover-full {
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .popover-custom-wrap {
    display: flex;
    gap: 6px;
    padding-top: 6px;
    border-top: 1px solid var(--border);
  }
  .popover-custom-input {
    flex: 1;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 3px 6px;
    color: var(--foreground);
    font-size: 11px;
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
</style>
