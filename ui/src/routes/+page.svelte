<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "$lib/Icon.svelte";
  import Toast from "$lib/Toast.svelte";
  import Sidebar from "$lib/Sidebar.svelte";
  import StatusBar from "$lib/StatusBar.svelte";
  import TimelinePanel from "$lib/panels/TimelinePanel.svelte";
  import NowPanel from "$lib/panels/NowPanel.svelte";
  import FilesPanel from "$lib/panels/FilesPanel.svelte";
  import UndoPanel from "$lib/panels/UndoPanel.svelte";
  import ProjectsPanel from "$lib/panels/ProjectsPanel.svelte";
  import HealthPanel from "$lib/panels/HealthPanel.svelte";
  import SettingsPanel from "$lib/panels/SettingsPanel.svelte";
  import Topbar from "$lib/panels/Topbar.svelte";
  import Dialogs from "$lib/panels/Dialogs.svelte";
  import { app } from "$lib/app/state.svelte";
  import * as fx from "$lib/app/fx";
  import { start } from "$lib/app/lifecycle";
  import { SESSION_KEY, serializeSession } from "$lib/session";

  onMount(start);

  // remember the project, the tab and the open file, so a reload (a hot reload too) comes back to the same place
  $effect(() => {
    if (!app.sessionReady) return;
    const text = serializeSession({ repo: app.currentRepo, tab: app.activeTab, file: app.selectedFilePath });
    if (text === null) return;
    try {
      localStorage.setItem(SESSION_KEY, text);
    } catch {
      // storage can be unavailable; remembering is a convenience
    }
  });
</script>

<div class="sr-only" aria-live="polite">{app.statusAnnouncement}</div>

<div class="app-layout">
  <!-- Collapsible Precision Sidebar (A1) -->
  <Sidebar
    activeTab={app.activeTab}
    status={app.status}
    currentRepo={app.currentRepo}
    watchedReposCount={app.watchedRepos.length}
    pendingCount={app.pendingChanges.length}
    pendingMore={app.pendingMore}
    collapsed={app.sidebarCollapsed}
    ontoggleCollapse={fx.toggleSidebar}
    onselectTab={(tab) => {
      app.activeTab = tab;
      if (tab === "now") { fx.fetchAgents(); fx.fetchPending(); fx.fetchLocks(); fx.fetchBackground(); }
      // a file restored from the last session is highlighted, but its lines load only on request
      if (tab === "files" && app.selectedFilePath && app.blameLines.length === 0 && !app.blameLoading && !app.blameError) fx.openFileBlame(app.selectedFilePath);
      if (tab === "projects") { fx.fetchDaemonStatus(); }
      if (tab === "health") { fx.fetchDoctor(); }
      if (tab === "settings") { fx.fetchSettings(); fx.fetchShare(); }
    }}
    onopenShortcuts={() => (app.showShortcutsModal = true)}
  />

  <!-- Main Workspace -->
  <div class="main-wrapper">
    <!-- Top Action Bar with TypeSafe Popover & Search (A3 / M1) -->
    <Topbar />

    <!-- Content Workspace -->
    <main class="content-pane">
      {#if app.errorMsg}
        <div class="alert alert-destructive" role="alert">
          <div class="alert-row">
            <Icon name="alert" size={15} />
            <div class="alert-desc font-mono">{app.errorMsg}</div>
            <button type="button" class="text-btn font-mono" onclick={() => (app.errorMsg = null)}>Dismiss</button>
          </div>
        </div>
      {/if}

      {#each app.panelProblems as key (key)}
        <div class="alert alert-destructive" role="alert" data-problem={key}>
          <div class="alert-row">
            <Icon name="alert" size={15} />
            <div class="alert-desc font-mono">
              Could not load {fx.PROBLEM_LABEL[key]}: {app.loadProblem[key]}
              {#if app.lastGood[key]}<br />What is shown is the last good data, from {app.lastGood[key]}.{/if}
            </div>
            <button type="button" class="text-btn font-mono" onclick={() => fx.retryLoad(key)}>Retry</button>
          </div>
        </div>
      {/each}

      <!-- 1. TIMELINE & RESIZABLE DIFF INSPECTOR (A2 / V5) -->
      {#if app.activeTab === "timeline"}
  <TimelinePanel />
{:else if app.activeTab === "now"}
  <NowPanel />
{:else if app.activeTab === "files"}
  <FilesPanel />
{:else if app.activeTab === "undo"}
  <UndoPanel />
{:else if app.activeTab === "projects"}
  <ProjectsPanel />
{:else if app.activeTab === "health"}
  <HealthPanel />
{:else if app.activeTab === "settings"}
  <SettingsPanel />
{/if}
    </main>

    <!-- Bottom VS Code Style Status Bar (A4) -->
    <StatusBar
      status={app.status}
      daemonPid={app.daemonPid}
      daemonUptime={app.daemonUptime}
      currentRepo={app.currentRepo}
      onopenTab={(t) => (app.activeTab = t)}
    />
  </div>
</div>

<!-- TypeSafe Modal: Safe Undo (M3, M4, M5, M6) -->
<Dialogs />

<!-- Floating Toast Notifications (M7) -->
<Toast toasts={app.toasts} ondismiss={fx.dismissToast} onredo={fx.applyRedo} />

<style>
  :global(*) {
    box-sizing: border-box;
  }
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: var(--font-ui, "IBM Plex Sans", sans-serif);
    font-size: 13px;
    line-height: 1.45;
    -webkit-font-smoothing: antialiased;
    overflow: hidden;
    background: var(--bg);
    color: var(--foreground);
  }
  .app-layout {
    display: flex;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
    background: var(--bg);
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    border: 0;
  }
  .font-mono {
    font-family: var(--font-mono, "JetBrains Mono", monospace);
  }
  :global(button:focus-visible) {
    outline: 2px solid var(--ring);
    outline-offset: 1px;
  }
  :global(input:focus-visible) {
    outline: 2px solid var(--ring);
    outline-offset: 1px;
  }
  .main-wrapper {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
  }
  .content-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 0;
    min-height: 0;
    min-width: 0;
  }
  .alert {
    padding: 8px 12px;
    border-radius: 4px;
    margin: 8px 12px;
  }
  .alert-destructive {
    background: var(--destructive-dim);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: var(--destructive);
  }
  .alert-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .alert-desc {
    flex: 1;
    font-size: 11px;
  }
  .text-btn {
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
    text-decoration: underline;
    font-size: 10px;
  }
</style>
