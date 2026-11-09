<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import { app } from "$lib/app/state.svelte";
  import * as fx from "$lib/app/fx";
  import { formatEventRelativeTime } from "$lib/timeline-view";

  let showRawJson = $state(false);
  let noteInput = $state("");
  let editing = $state(false);
  let editorEl = $state<HTMLTextAreaElement>();

  // another change picked: leave edit mode and forget the draft
  $effect(() => {
    void app.selectedChange?.id;
    editing = false;
  });

  function startEdit() {
    noteInput = app.selectedChange?.why || "";
    editing = true;
    queueMicrotask(() => editorEl?.focus());
  }

  function copyToClipboard(text: string, label: string) {
    if (!text) return;
    try {
      navigator.clipboard
        .writeText(text)
        .then(() => fx.addToast("info", `Copied ${label} to clipboard`))
        .catch(() => fx.addToast("error", `Could not copy ${label}`));
    } catch {
      fx.addToast("error", `Could not copy ${label}`);
    }
  }

  async function handleSaveReason() {
    if (!app.selectedChange) return;
    app.inlineNoteInput = noteInput;
    // a refused reason keeps the editor open with the text, so it can be fixed
    if (await fx.saveInlineNote()) editing = false;
  }
</script>

{#if app.selectedChange}
  {@const c = app.selectedChange}
  {@const act = fx.getActorInfo(c.actor)}
  {@const isBaseline = c.path === "." || c.op === "baseline"}

  <aside class="inspector-pane select-none" style="width: {app.inspectorWidth}px;">
    <!-- 1. Inspector Header -->
    <div class="inspector-header">
      <div class="header-top-row">
        <div class="header-tags">
          <span class="op-chip text-12 op-{c.op}">{c.op}</span>
          <div class="id-badge font-mono text-12" title="Change ID: {c.id}">
            <span>{c.id.slice(0, 8)}</span>
            <button
              type="button"
              class="icon-action-btn"
              title="Copy change ID"
              onclick={() => copyToClipboard(c.id, "ID")}
            >
              <Icon name="copy" size={11} />
            </button>
          </div>
        </div>

        <div class="header-actions">
          <button
            type="button"
            class="icon-action-btn"
            title="Open blame history"
            onclick={() => fx.openFileBlame(c.path)}
          >
            <Icon name="files" size={13} />
          </button>
          <button
            type="button"
            class="icon-action-btn"
            title="Close (back to the list)"
            onclick={() => (app.inspectorVisible = false)}
          >
            <Icon name="x" size={13} />
            <span class="back-label text-12">Back</span>
          </button>
        </div>
      </div>

      <!-- Full File Path with Copy Action -->
      <div class="path-display-row group">
        <span class="full-path-text font-mono text-12 select-text" title={c.path}>
          {isBaseline ? "(project baseline)" : c.path}
        </span>
        {#if !isBaseline}
          <button
            type="button"
            class="copy-path-btn text-muted"
            title="Copy full path"
            onclick={() => copyToClipboard(c.path, "path")}
          >
            <Icon name="copy" size={12} />
          </button>
        {/if}
      </div>
    </div>

    <div class="inspector-body">
      <!-- 2. Meta Grid (WHO / TIMESTAMP) -->
      <div class="meta-grid">
        <!-- Col 1: WHO -->
        <div class="meta-col">
          <span class="meta-label text-12 font-semibold">WHO</span>
          <div class="meta-actor-row">
            <span class="actor-pill text-12 {act.colorClass}">
              <span class="actor-initial">{act.name.charAt(0).toUpperCase()}</span>
              <span>{act.name}</span>
            </span>
            <span class="confidence-tag text-12">{c.confidence || "exact"}</span>
          </div>
          <span class="session-tag font-mono text-12 text-muted">
            session: {c.session || "none"}
          </span>
        </div>

        <!-- Col 2: TIMESTAMP -->
        <div class="meta-col">
          <span class="meta-label text-12 font-semibold">TIMESTAMP</span>
          <span class="time-main text-13 font-mono">{fx.formatTime(c.time.start)}</span>
          <span class="time-relative text-12 text-muted">{formatEventRelativeTime(c.time.start)}</span>
        </div>
      </div>

      <!-- 3. WHY: read it, then Add or Edit; Save and Cancel only while editing -->
      <div class="why-section">
        <div class="why-head">
          <span class="meta-label text-12 font-semibold">WHY</span>
          {#if !editing && !isBaseline}
            <button type="button" class="why-action text-12" class:add={!c.why} onclick={startEdit}>
              <Icon name="pencil" size={12} />
              <span>{c.why ? "Edit" : "Add reason"}</span>
            </button>
          {/if}
        </div>

        {#if editing}
          <div class="why-editor">
            <textarea
              bind:this={editorEl}
              bind:value={noteInput}
              class="why-input text-13"
              rows="3"
              placeholder="Why was this changed? For example: Fix the off-by-one in page counts"
              aria-label="Reason for this change"
              onkeydown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  handleSaveReason();
                } else if (e.key === "Escape") {
                  e.stopPropagation();
                  editing = false;
                }
              }}
            ></textarea>
            <div class="why-editor-foot">
              <span class="text-12 text-muted">
                {c.why ? "Replaces the reason for this change; the old one stays in the history." : "At least 12 characters; say why, not what."}
              </span>
              <div class="why-buttons">
                <button type="button" class="why-cancel text-12" onclick={() => (editing = false)}>Cancel</button>
                <button
                  type="button"
                  class="why-save text-12"
                  disabled={app.inlineNoteSaving || !noteInput.trim() || noteInput.trim() === (c.why || "")}
                  onclick={handleSaveReason}
                >
                  {app.inlineNoteSaving ? "Saving…" : "Save"}
                </button>
              </div>
            </div>
          </div>
        {:else if c.why}
          <p class="why-text text-13 select-text">{c.why}</p>
        {:else if isBaseline}
          <p class="why-text text-13 text-muted">Initial project baseline</p>
        {:else}
          <p class="why-missing text-12"><span class="amber-dot"></span>No reason yet</p>
        {/if}
        {#if c.command}
          <div class="command-banner font-mono text-12 select-text">
            <span class="text-muted">$</span> {c.command}
          </div>
        {/if}
      </div>

      <!-- 4. UNIFIED DIFF View -->
      <div class="diff-section">
        <div class="diff-header text-12">
          <span class="font-semibold uppercase tracking-wider text-muted">UNIFIED DIFF</span>
          <span class="text-muted font-mono">
            {#if c.hash_only}
              Recorded by hash
            {:else if app.diffLoading}
              Loading…
            {:else}
              {app.parsedDiffLines.length} lines
            {/if}
          </span>
        </div>

        {#if c.hash_only}
          <div class="no-diff-box text-12 text-muted">
            No text diff: Tracewise kept only a hash of this version (binary, large, a link or a file with a secret).
          </div>
        {:else if app.diffLoading}
          <div class="no-diff-box text-12 text-muted">
            Loading diff…
          </div>
        {:else if app.parsedDiffLines.length === 0}
          <div class="no-diff-box text-12 text-muted">
            No changes recorded in diff
          </div>
        {:else}
          <div class="diff-code-scroll font-mono text-12 select-text">
            {#each app.parsedDiffLines as dl}
              <div
                class="diff-line"
                class:line-add={dl.type === "add"}
                class:line-del={dl.type === "del"}
                class:line-hunk={dl.type === "hunk"}
              >
                <span class="ln ln-old">{dl.oldNum ?? " "}</span>
                <span class="ln ln-new">{dl.newNum ?? " "}</span>
                <span class="diff-prefix">{dl.marker || (dl.type === "add" ? "+" : dl.type === "del" ? "-" : " ")}</span>
                <span class="diff-content">{dl.code}</span>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <!-- 5. Bottom Action Controls (Fixed bottom of inspector) -->
    <div class="inspector-footer">
      <!-- ONLY ONE solid filled primary action button -->
      <button
        type="button"
        class="primary-undo-btn text-12 font-medium"
        onclick={() => fx.openUndoModal(c)}
      >
        <Icon name="undo" size={13} />
        <span>Undo this change</span>
      </button>

      <!-- Secondary outline action buttons -->
      <div class="secondary-btns-row">
        <button
          type="button"
          class="secondary-btn text-12"
          onclick={() => fx.openFileBlame(c.path)}
        >
          Open file history
        </button>
        <button
          type="button"
          class="secondary-btn text-12"
          disabled={app.busy !== null}
          onclick={() => fx.restoreChange(c)}
          title="Put file back to content after this change"
        >
          {app.armed === `restore:${c.id}` ? "Click to confirm" : "Restore file"}
        </button>
      </div>

      <!-- Collapsible Raw JSON Details -->
      <div class="raw-json-wrap">
        <button
          type="button"
          class="raw-json-toggle-btn text-12 text-muted"
          onclick={() => (showRawJson = !showRawJson)}
        >
          <span>Details (raw JSON)</span>
          <Icon name={showRawJson ? "chevron-up" : "chevron-down"} size={12} />
        </button>
        {#if showRawJson}
          <pre class="raw-json-content font-mono text-12 select-text">{JSON.stringify(c, null, 2)}</pre>
        {/if}
      </div>
    </div>
  </aside>
{/if}

<style>
  .why-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 24px;
  }
  .why-action {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 24px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: none;
    color: var(--muted-foreground);
    cursor: pointer;
  }
  .why-action:hover {
    border-color: var(--border);
    color: var(--foreground);
  }
  .why-action.add {
    border-color: var(--warning-border);
    color: var(--warning);
  }
  .why-text {
    margin: 6px 0 0;
    line-height: 1.5;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .why-missing {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 6px 0 0;
    color: var(--warning);
  }
  .why-missing .amber-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--warning);
  }
  .why-editor {
    margin-top: 6px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .why-input {
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    padding: 8px 10px;
    border: 1px solid var(--accent);
    border-radius: 4px;
    background: var(--bg);
    color: var(--foreground);
    font-family: var(--font-ui);
    line-height: 1.5;
    outline: none;
  }
  .why-editor-foot {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
  }
  .why-buttons {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }
  .why-cancel,
  .why-save {
    height: 28px;
    padding: 0 12px;
    border-radius: 4px;
    cursor: pointer;
  }
  .why-cancel {
    border: 1px solid var(--border);
    background: none;
    color: var(--foreground);
  }
  .why-save {
    border: 1px solid var(--accent);
    background: var(--accent);
    color: #fff;
  }
  .why-save:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .back-label {
    display: none;
    margin-left: 4px;
  }
  @container timeline (max-width: 639px) {
    .back-label {
      display: inline;
    }
  }
  .inspector-pane {
    background: var(--card);
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    flex-shrink: 0;
    min-width: 280px;
    max-width: min(520px, 50vw);
    height: 100%;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .inspector-header {
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: var(--card);
  }

  .header-top-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .header-tags {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .op-chip {
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--card-elevated);
    border: 1px solid var(--border);
    color: var(--foreground);
    text-transform: capitalize;
  }

  .id-badge {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--muted-foreground);
  }

  .icon-action-btn {
    background: transparent;
    border: none;
    color: var(--muted-foreground);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 2px;
    border-radius: 3px;
    transition: color 0.12s ease;
  }

  .icon-action-btn:hover {
    color: var(--foreground);
  }

  .path-display-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
  }

  .full-path-text {
    word-break: break-all;
    color: var(--foreground);
  }

  .copy-path-btn {
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 2px;
    opacity: 0.6;
    transition: opacity 0.12s ease;
  }

  .copy-path-btn:hover {
    opacity: 1;
    color: var(--foreground);
  }

  .inspector-body {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  .meta-grid {
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    background: var(--bg);
  }

  .meta-col {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .meta-label {
    letter-spacing: 0.05em;
    color: var(--muted-foreground);
    text-transform: uppercase;
  }

  .meta-actor-row {
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
  }

  .actor-initial {
    width: 13px;
    height: 13px;
    border-radius: 2px;
    background: var(--surface-container-highest);
    color: var(--foreground);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 9px;
    font-weight: 600;
  }

  .confidence-tag {
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--card-elevated);
    border: 1px solid var(--border);
    color: var(--muted-foreground);
  }

  .session-tag {
    font-size: 11px;
  }

  .why-section {
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .intent-box {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .intent-text {
    color: var(--foreground);
    line-height: 1.4;
  }

  .note-edit-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-top: 4px;
    border-top: 1px solid var(--border);
  }

  .note-input {
    flex: 1;
    background: transparent;
    border: none;
    color: var(--foreground);
    outline: none;
  }

  .btn-save-note {
    padding: 2px 8px;
    background: var(--card-elevated);
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--foreground);
    cursor: pointer;
  }

  .command-banner {
    padding: 4px 8px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--foreground);
  }

  .diff-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 180px;
  }

  .diff-header {
    padding: 6px 12px;
    background: var(--bg);
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .diff-code-scroll {
    flex: 1;
    overflow: auto;
    background: var(--bg);
    padding: 4px 0;
    line-height: 1.45;
  }

  .diff-line {
    display: grid;
    grid-template-columns: 32px 32px 14px 1fr;
    padding: 0 4px;
    white-space: pre;
  }

  .ln {
    color: var(--muted-foreground);
    font-size: 11px;
    text-align: right;
    padding-right: 4px;
    opacity: 0.6;
    user-select: none;
  }

  .diff-prefix {
    user-select: none;
    text-align: center;
    font-weight: 600;
  }

  .line-add {
    background: var(--diff-add-bg);
    color: var(--diff-add-fg);
    border-left: 2px solid var(--diff-add-fg);
  }

  .line-del {
    background: var(--diff-del-bg);
    color: var(--diff-del-fg);
    border-left: 2px solid var(--diff-del-fg);
  }

  .line-hunk {
    background: var(--diff-hunk-bg);
    color: var(--diff-hunk-fg);
  }

  .no-diff-box {
    padding: 30px;
    text-align: center;
  }

  .inspector-footer {
    padding: 10px 12px;
    border-top: 1px solid var(--border);
    background: var(--card);
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex-shrink: 0;
  }

  .primary-undo-btn {
    height: 32px;
    width: 100%;
    background: var(--surface-container-highest);
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    color: var(--foreground);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    cursor: pointer;
    transition: background 0.12s ease;
  }

  .primary-undo-btn:hover {
    background: var(--card-elevated);
  }

  .secondary-btns-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .secondary-btn {
    height: 30px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--foreground);
    cursor: pointer;
    transition: background 0.12s ease;
  }

  .secondary-btn:hover {
    background: var(--card-elevated);
  }

  .raw-json-wrap {
    display: flex;
    flex-direction: column;
  }

  .raw-json-toggle-btn {
    background: transparent;
    border: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 0;
    cursor: pointer;
  }

  .raw-json-toggle-btn:hover {
    color: var(--foreground);
  }

  .raw-json-content {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 8px;
    max-height: 160px;
    overflow: auto;
    margin: 4px 0 0;
  }

  .text-12 {
    font-size: 12px;
    line-height: 16px;
  }
  .text-13 {
    font-size: 13px;
    line-height: 18px;
  }
  .font-mono {
    font-family: var(--font-mono, monospace);
  }
  .text-muted {
    color: var(--muted-foreground);
  }
</style>
