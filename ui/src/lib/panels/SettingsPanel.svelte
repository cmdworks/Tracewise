<script lang="ts">
  import { app } from "$lib/app/state.svelte";
  import * as fx from "$lib/app/fx";
  import { SETTINGS, SHARE_LEVELS, sourceText } from "$lib/settings";

  // what the person typed or picked and has not saved yet, by key
  let draft = $state<Record<string, string>>({});
  let global = $state(false);

  const defs = new Map(SETTINGS.map((d) => [d.key, d]));
  let rows = $derived(app.settings.map((r) => ({ ...r, def: defs.get(r.key)! })));

  // sharing form
  let shareRemote = $state("");
  let shareLevel = $state("summary");
  let shareEncrypt = $state(false);
  let importText = $state("");
  let importCurrent = $state(false);
  let confirmRotate = $state(false);
  let copied = $state(false);

  async function copyKey() {
    try {
      await navigator.clipboard.writeText(app.exportedKey);
      copied = true;
    } catch {
      copied = false;
    }
  }
  function hideKey() {
    app.exportedKey = "";
    copied = false;
  }

  function shown(key: string, saved: string) {
    return draft[key] ?? saved;
  }
  function dirty(key: string, saved: string) {
    return draft[key] !== undefined && draft[key].trim() !== saved;
  }
  async function save(key: string) {
    if (await fx.saveSetting(key, draft[key], global)) {
      const { [key]: _gone, ...rest } = draft;
      draft = rest;
    }
  }
</script>

<div class="settings-view">
  <section class="card">
    <h2 class="card-title">Settings</h2>
    <p class="section-sub">
      Saved with <code>tracewise config set</code>. A person can change these; an agent cannot.
    </p>
    <label class="scope">
      <input type="checkbox" bind:checked={global} />
      <span>Save for all projects (otherwise only for this project)</span>
    </label>

    {#if app.settingsLoading && rows.length === 0}
      <p class="note">Reading the settings…</p>
    {:else if rows.length === 0}
      <p class="note">No settings to show. Pick a project first.</p>
    {/if}

    <ul class="setting-list">
      {#each rows as r (r.key)}
        <li class="setting">
          <div class="setting-head">
            <label for={`set-${r.key}`} class="setting-label">{r.def.label}</label>
            <span class="source" class:overridden={r.source === "env"}>{sourceText(r.source)}</span>
          </div>
          <p class="help">{r.def.help}</p>
          <div class="setting-edit">
            {#if r.def.options}
              <select id={`set-${r.key}`} value={shown(r.key, r.value)} onchange={(e) => (draft[r.key] = e.currentTarget.value)}>
                {#each r.def.options as o}<option value={o}>{o}</option>{/each}
              </select>
            {:else}
              <input
                id={`set-${r.key}`}
                type="text"
                value={shown(r.key, r.value)}
                placeholder={r.def.placeholder}
                oninput={(e) => (draft[r.key] = e.currentTarget.value)}
                onkeydown={(e) => e.key === "Enter" && dirty(r.key, r.value) && save(r.key)}
              />
            {/if}
            <button type="button" class="btn btn-primary" disabled={!dirty(r.key, r.value)} onclick={() => save(r.key)}>Save</button>
            {#if draft[r.key] !== undefined}
              <button type="button" class="btn btn-secondary" onclick={() => { const { [r.key]: _g, ...rest } = draft; draft = rest; }}>Cancel</button>
            {/if}
          </div>
          {#if r.source === "env"}
            <p class="help">An environment variable decides this now, so a saved value will not show until it is removed.</p>
          {/if}
        </li>
      {/each}
    </ul>
  </section>

  <section class="card share-card" id="share">
    <h2 class="card-title">Sharing</h2>
    <p class="section-sub">Send the history to a git remote so other machines and people can read it. Saved with <code>tracewise share</code>.</p>

    {#if app.shareProfileOff}
      <p class="note">Sharing belongs to the full profile. Set <strong>Profile</strong> above to <code>full</code>, save, and come back.</p>
    {:else if !app.share}
      <p class="note">Reading the sharing setup…</p>
    {:else if !app.share.configured}
      <div class="setting">
        <label class="setting-label" for="share-remote">Where</label>
        <p class="help">Leave empty to use this project's own remote (nothing shows in the branch list). Or paste the URL of a separate repository.</p>
        <input id="share-remote" type="text" bind:value={shareRemote} placeholder="empty = this project's origin" />
      </div>
      <div class="setting">
        <label class="setting-label" for="share-level">What is shared</label>
        <select id="share-level" bind:value={shareLevel}>
          {#each SHARE_LEVELS as l}<option value={l.value}>{l.label}</option>{/each}
        </select>
        <p class="help">{SHARE_LEVELS.find((l) => l.value === shareLevel)?.help}</p>
      </div>
      <label class="scope">
        <input type="checkbox" bind:checked={shareEncrypt} />
        <span>Encrypt with a key kept on this machine (only people you give the key can read it)</span>
      </label>
      <button type="button" class="btn btn-primary" onclick={() => fx.startSharing({ remote: shareRemote, level: shareLevel, encrypt: shareEncrypt })}>Turn on sharing</button>
    {:else}
      <dl class="share-facts">
        <dt>Where</dt><dd>{app.share.store === "origin" ? "This project's own remote" : "A separate repository"} <span class="muted">({app.share.remote})</span></dd>
        <dt>Shared</dt><dd>{SHARE_LEVELS.find((l) => l.value === app.share?.level)?.label ?? app.share.level}</dd>
        <dt>Hidden refs</dt><dd>{app.share.refs === "hidden" ? "Yes: no branches added on the remote" : "No: batches are branches named tracewise/device/…"}</dd>
        <dt>Encryption</dt><dd>{app.share.encrypt ? `On, key ${app.share.key_id ?? "missing"}` : "Off"}</dd>
        <dt>Remote approved</dt><dd>{app.share.trusted ? "Yes" : "No: run tracewise share trust"}</dd>
      </dl>
      {#each app.share.warnings as w}
        <p class="warn" role="note">{w}</p>
      {/each}

      {#if app.share.encrypt}
        <div class="setting">
          <div class="setting-head"><span class="setting-label">Key</span></div>
          <p class="help">The key is the only way to read the shared history. Export it and keep it in a password manager.</p>
          {#if app.exportedKey}
            <p class="warn" role="note">This is the key itself. Anyone who has it can read the shared history.</p>
            <textarea readonly rows="3" class="keybox" value={app.exportedKey}></textarea>
            <div class="setting-edit">
              <button type="button" class="btn btn-primary" onclick={copyKey}>{copied ? "Copied" : "Copy"}</button>
              <button type="button" class="btn btn-secondary" onclick={hideKey}>Hide</button>
            </div>
          {:else}
            <div class="setting-edit">
              <button type="button" class="btn btn-secondary" onclick={() => fx.exportKey()}>Show key to copy</button>
              {#if !confirmRotate}
                <button type="button" class="btn btn-secondary" onclick={() => (confirmRotate = true)}>New key…</button>
              {:else}
                <button type="button" class="btn btn-primary" onclick={async () => { await fx.rotateKey(); confirmRotate = false; }}>Make a new key for new batches</button>
                <button type="button" class="btn btn-secondary" onclick={() => (confirmRotate = false)}>Cancel</button>
              {/if}
            </div>
            {#if confirmRotate}<p class="help">Old batches stay readable with the old key. It does not unread what others already fetched. Everyone must import the new key.</p>{/if}
          {/if}
        </div>
      {/if}
      <div class="setting">
        <label class="setting-label" for="share-import">Have a key from someone else?</label>
        <div class="setting-edit">
          <input id="share-import" type="text" bind:value={importText} placeholder="tracewisekey1:…" />
          <button type="button" class="btn btn-secondary" disabled={!importText.trim()} onclick={async () => { if (await fx.importKey(importText, importCurrent)) importText = ""; }}>Import</button>
        </div>
        <label class="scope"><input type="checkbox" bind:checked={importCurrent} /><span>Use it for new batches too (after a rotation)</span></label>
      </div>
    {/if}
  </section>
</div>

<style>
  .settings-view { overflow-y: auto; padding: 14px; }
  .card { background: var(--card); border: 1px solid var(--border); border-radius: 8px; padding: 14px; max-width: 720px; }
  .card-title { margin: 0; font-size: 14px; font-weight: 600; }
  .section-sub, .help, .note { margin: 4px 0 0; font-size: 11px; color: var(--muted-foreground); }
  .scope { display: flex; align-items: center; gap: 8px; margin: 12px 0; font-size: 12px; }
  .setting-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; }
  .setting { padding: 12px 0; border-top: 1px solid var(--border); }
  .setting-head { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; }
  .setting-label { font-size: 13px; font-weight: 600; }
  .source { font-size: 10px; color: var(--muted-foreground); }
  .source.overridden { color: var(--warning); }
  .setting-edit { display: flex; gap: 8px; margin-top: 8px; flex-wrap: wrap; }
  select, input[type="text"] {
    flex: 1; min-width: 140px; padding: 5px 8px; font-size: 12px; color: var(--foreground);
    background: var(--bg-subtle); border: 1px solid var(--border); border-radius: 5px;
  }
  .btn { padding: 5px 12px; border-radius: 5px; font-size: 12px; font-weight: 600; cursor: pointer; border: 1px solid transparent; }
  .btn:disabled { opacity: 0.45; cursor: not-allowed; }
  .btn-primary { background: var(--accent); color: #fff; }
  .share-card { margin-top: 14px; }
  .share-card input[type="text"] { width: 100%; box-sizing: border-box; margin-top: 6px; }
  .share-card .setting-edit input[type="text"] { margin-top: 0; }
  .share-facts { display: grid; grid-template-columns: max-content 1fr; gap: 6px 14px; font-size: 12px; margin: 12px 0; }
  .share-facts dt { color: var(--muted-foreground); }
  .share-facts dd { margin: 0; }
  .muted { color: var(--muted-foreground); }
  .warn { margin: 8px 0 0; padding: 8px 10px; border: 1px solid var(--warning); border-radius: 6px; background: var(--warning-dim); font-size: 12px; }
  .keybox { width: 100%; margin-top: 8px; padding: 8px; font-family: var(--font-mono); font-size: 11px; color: var(--foreground); background: var(--bg-subtle); border: 1px solid var(--border); border-radius: 5px; resize: none; }
  .btn-secondary { background: var(--card-elevated); border-color: var(--border); color: var(--foreground); }
</style>
