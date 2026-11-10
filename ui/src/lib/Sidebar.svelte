<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { Tab, TracewiseStatus } from "./types";
  import { apply, load, next, save, type Theme } from "./theme";
  import { onMount } from "svelte";

  interface Props {
    activeTab: Tab;
    status: TracewiseStatus | null;
    currentRepo?: string;
    watchedReposCount: number;
    pendingCount: number;
    pendingMore?: boolean;
    collapsed: boolean;
    ontoggleCollapse: () => void;
    onselectTab: (tab: Tab) => void;
    onopenShortcuts: () => void;
  }

  let {
    activeTab,
    status,
    currentRepo = "",
    watchedReposCount,
    pendingCount,
    pendingMore = false,
    collapsed,
    ontoggleCollapse,
    onselectTab,
    onopenShortcuts,
  }: Props = $props();

  let theme = $state<Theme>("system");
  onMount(() => {
    theme = load();
    apply(theme);
    // `system` has to follow the operating system while the app is open
    const mq = matchMedia("(prefers-color-scheme: dark)");
    const follow = () => theme === "system" && apply(theme);
    mq.addEventListener("change", follow);
    return () => mq.removeEventListener("change", follow);
  });
  function cycleTheme() {
    theme = next(theme);
    save(theme);
    apply(theme);
  }

  const projectName = $derived(currentRepo ? currentRepo.split("/").pop() || currentRepo : "No project");
  const displayRepoPath = $derived(
    currentRepo
      ? currentRepo.replace(/^\/Users\/[^/]+/, "~")
      : "Choose a project"
  );
</script>

<aside class="sidebar" class:collapsed aria-label="Primary Navigation">
  <div class="sidebar-top">
    <!-- App Title Header -->
    <div class="sidebar-brand">
      <div class="brand-left">
        <span class="brand-glyph">
          <img src="/logo.svg" alt="" width="26" height="26" />
        </span>
        {#if !collapsed}
          <span class="brand-title text-14">Tracewise</span>
        {/if}
      </div>
      {#if !collapsed}
        <span class="profile-tag font-mono text-12">{status?.profile?.name ?? "core"}</span>
      {/if}
    </div>

    <!-- Project Selector Dropdown Box -->
    {#if !collapsed}
      <div class="project-box-wrap">
        <button
          type="button"
          class="project-box-btn"
          onclick={() => onselectTab("projects")}
          title="Switch or inspect project repository ({currentRepo})"
        >
          <div class="project-box-info min-w-0">
            <Icon name="folder" size={13} class="text-muted shrink-0" />
            <div class="project-names truncate">
              <span class="project-name text-12 font-medium truncate">{projectName}</span>
              <span class="project-path font-mono text-12 text-muted truncate">{displayRepoPath}</span>
            </div>
          </div>
          <Icon name="chevron-down" size={13} class="text-muted shrink-0" />
        </button>
      </div>
    {/if}

    <!-- Section: WORK -->
    <div class="nav-section">
      {#if !collapsed}
        <div class="section-label text-12">WORK</div>
      {/if}

      <!-- Timeline -->
      <button
        type="button"
        class="nav-item"
        class:active={activeTab === "timeline"}
        title={collapsed ? "Timeline (1)" : undefined}
        onclick={() => onselectTab("timeline")}
      >
        <Icon name="timeline" size={15} />
        {#if !collapsed}
          <span class="nav-label text-13">Timeline</span>
          {#if status && status.changes > 0}
            <span class="nav-badge font-mono text-12 tabular-nums">
              {status.changes > 9999 ? (status.changes / 1000).toFixed(0) + "k" : status.changes.toLocaleString()}
            </span>
          {/if}
        {/if}
      </button>

      <!-- Now -->
      <button
        type="button"
        class="nav-item"
        class:active={activeTab === "now"}
        title={collapsed ? "Now (2)" : undefined}
        onclick={() => onselectTab("now")}
      >
        <div class="nav-item-left">
          <Icon name="now" size={15} />
          {#if !collapsed}
            <span class="nav-label text-13">Now</span>
          {/if}
        </div>
        {#if !collapsed && pendingCount > 0}
          <span class="nav-badge badge-amber font-mono text-12 tabular-nums">
            {pendingCount}{pendingMore ? "+" : ""}
          </span>
        {/if}
      </button>

      <!-- Files -->
      <button
        type="button"
        class="nav-item"
        class:active={activeTab === "files"}
        title={collapsed ? "Files (3)" : undefined}
        onclick={() => onselectTab("files")}
      >
        <Icon name="files" size={15} />
        {#if !collapsed}
          <span class="nav-label text-13">Files</span>
        {/if}
      </button>

      <!-- Undo -->
      <button
        type="button"
        class="nav-item"
        class:active={activeTab === "undo"}
        title={collapsed ? "Undo (4)" : undefined}
        onclick={() => onselectTab("undo")}
      >
        <Icon name="undo" size={15} />
        {#if !collapsed}
          <span class="nav-label text-13">Undo</span>
        {/if}
      </button>
    </div>

    <!-- Section: SYSTEM -->
    <div class="nav-section">
      {#if !collapsed}
        <div class="section-label text-12">SYSTEM</div>
      {/if}

      <!-- Projects -->
      <button
        type="button"
        class="nav-item"
        class:active={activeTab === "projects"}
        title={collapsed ? "Projects (5)" : undefined}
        onclick={() => onselectTab("projects")}
      >
        <Icon name="projects" size={15} />
        {#if !collapsed}
          <span class="nav-label text-13">Projects</span>
          {#if watchedReposCount > 0}
            <span class="nav-badge font-mono text-12 tabular-nums">{watchedReposCount}</span>
          {/if}
        {/if}
      </button>

      <!-- Health -->
      <button
        type="button"
        class="nav-item"
        class:active={activeTab === "health"}
        title={collapsed ? "Health (6)" : undefined}
        onclick={() => onselectTab("health")}
      >
        <Icon name="health" size={15} />
        {#if !collapsed}
          <span class="nav-label text-13">Health</span>
        {/if}
      </button>

      <!-- Settings -->
      <button
        type="button"
        class="nav-item"
        class:active={activeTab === "settings"}
        title={collapsed ? "Settings (7)" : undefined}
        onclick={() => onselectTab("settings")}
      >
        <Icon name="settings" size={15} />
        {#if !collapsed}
          <span class="nav-label text-13">Settings</span>
        {/if}
      </button>
    </div>
  </div>

  <!-- Bottom Pinned Footer -->
  <div class="sidebar-footer">
    <button type="button" class="footer-action-btn text-12" onclick={cycleTheme} title="Theme: {theme} (click to change)">
      <Icon name={theme === "dark" ? "moon" : "sun"} size={14} />
      {#if !collapsed}<span class="text-13">Theme: {theme}</span>{/if}
    </button>
    <button
      type="button"
      class="footer-action-btn text-12"
      onclick={onopenShortcuts}
      title="Keyboard shortcuts (?)"
    >
      <Icon name="keyboard" size={14} />
      {#if !collapsed}
        <span class="text-13">Shortcuts</span>
        <kbd class="shortcut-kbd font-mono text-12">?</kbd>
      {/if}
    </button>

    <button
      type="button"
      class="collapse-btn text-12"
      onclick={ontoggleCollapse}
      title={collapsed ? "Expand sidebar (⌘B)" : "Collapse sidebar (⌘B)"}
    >
      <Icon name={collapsed ? "chevron-right" : "chevron-left"} size={14} />
      {#if !collapsed}
        <span class="text-12 font-mono text-muted">Collapse ⌘B</span>
      {/if}
    </button>
  </div>
</aside>

<style>
  .sidebar {
    width: 210px;
    min-width: 210px;
    background: var(--sidebar-bg);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    user-select: none;
    transition: width 0.15s ease, min-width 0.15s ease;
    padding: 10px 8px;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .sidebar.collapsed {
    width: 52px;
    min-width: 52px;
    padding: 10px 4px;
  }

  .sidebar-top {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .sidebar-brand {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 2px 6px;
    height: 32px;
  }

  .brand-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .brand-glyph {
    width: 26px;
    height: 26px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--foreground);
  }

  .brand-title {
    font-weight: 600;
    color: var(--foreground);
    letter-spacing: -0.01em;
  }

  .profile-tag {
    padding: 2px 6px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--muted-foreground);
  }

  .project-box-wrap {
    padding: 0 2px;
  }

  .project-box-btn {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 8px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    cursor: pointer;
    text-align: left;
    transition: border-color 0.15s ease, background 0.15s ease;
  }

  .project-box-btn:hover {
    border-color: var(--border-strong);
    background: var(--card-elevated);
  }

  .project-box-info {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .project-names {
    display: flex;
    flex-direction: column;
    line-height: 1.25;
  }

  .project-name {
    color: var(--foreground);
    font-weight: 500;
  }

  .project-path {
    font-size: 11px;
    color: var(--muted-foreground);
  }

  .nav-section {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .section-label {
    padding: 4px 8px 2px;
    font-weight: 600;
    letter-spacing: 0.05em;
    color: var(--muted-foreground);
    text-transform: uppercase;
  }

  .nav-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 6px 8px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
    color: var(--muted-foreground);
    cursor: pointer;
    text-align: left;
    transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease;
  }

  .sidebar.collapsed .nav-item {
    justify-content: center;
    padding: 8px 0;
  }

  .nav-item-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .nav-item:hover {
    background: var(--card-elevated);
    color: var(--foreground);
  }

  .nav-item.active {
    background: var(--card-elevated);
    color: var(--foreground);
    border-color: var(--border);
    font-weight: 500;
  }

  .nav-label {
    margin-left: 8px;
    flex: 1;
    text-align: left;
  }

  .nav-badge {
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--card);
    border: 1px solid var(--border);
    color: var(--foreground);
  }

  .badge-amber {
    background: var(--warning-dim);
    border-color: var(--warning-border);
    color: var(--warning);
    font-weight: 600;
  }

  .sidebar-footer {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }

  .footer-action-btn,
  .collapse-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
    color: var(--muted-foreground);
    cursor: pointer;
    transition: background 0.12s ease, color 0.12s ease;
  }

  .sidebar.collapsed .footer-action-btn,
  .sidebar.collapsed .collapse-btn {
    justify-content: center;
    padding: 8px 0;
  }

  .footer-action-btn:hover,
  .collapse-btn:hover {
    background: var(--card-elevated);
    color: var(--foreground);
  }

  .shortcut-kbd {
    margin-left: auto;
    padding: 1px 5px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--muted-foreground);
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
  .text-muted {
    color: var(--muted-foreground);
  }
  .font-mono {
    font-family: var(--font-mono, monospace);
  }
  .tabular-nums {
    font-variant-numeric: tabular-nums;
  }
</style>
