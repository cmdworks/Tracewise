<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import { app } from "$lib/app/state.svelte";
  import { installCommandLine, checkCli } from "$lib/app/fx-cli";
  import { loadAll } from "$lib/app/lifecycle";

  const oneLine = "curl -fsSL https://raw.githubusercontent.com/cmdworks/tracewise/main/scripts/install.sh | sh";
  let copied = $state(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(oneLine);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      // the command is on screen; copying is a convenience
    }
  }

  async function again() {
    if (await checkCli()) loadAll();
  }
</script>

<main class="setup" aria-labelledby="setup-title">
  <div class="card">
    <Icon name="alert" size={22} />
    <h1 id="setup-title">The Tracewise command line is not installed</h1>
    <p>
      This app shows what the <code>tracewise</code> program records, so it needs the program on this Mac. Installing adds it to
      <code>~/.tracewise/bin</code>, with its command wrappers, shell hook and background watcher. It asks for no password.
      {#if app.cliAppVersion}Version {app.cliAppVersion} is installed, to match this app.{/if}
    </p>

    <div class="row">
      <button type="button" class="btn btn-primary" disabled={app.cliInstalling} onclick={() => installCommandLine(loadAll)}>
        {app.cliInstalling ? "Installing…" : "Install the command line"}
      </button>
      <button type="button" class="btn btn-secondary" disabled={app.cliInstalling} onclick={again}>Check again</button>
    </div>

    {#if app.cliError}
      <div class="alert" role="alert"><pre>{app.cliError}</pre></div>
    {/if}

    <details>
      <summary>Install it from a terminal instead</summary>
      <pre class="cmd">{oneLine}</pre>
      <button type="button" class="btn btn-secondary btn-sm" onclick={copy}>{copied ? "Copied" : "Copy"}</button>
    </details>
  </div>
</main>

<style>
  .setup {
    height: 100vh;
    display: grid;
    place-items: center;
    padding: 24px;
    background: var(--bg);
    color: var(--foreground);
  }
  .card {
    max-width: 520px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 28px;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 10px;
  }
  h1 {
    margin: 0;
    font-size: 18px;
  }
  p {
    margin: 0;
    color: var(--muted-foreground);
    line-height: 1.55;
  }
  code,
  pre {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 12px;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .btn {
    padding: 8px 14px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid var(--border-strong);
    background: var(--card-elevated);
    color: var(--foreground);
  }
  .btn-primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .btn-sm {
    padding: 4px 10px;
    font-size: 12px;
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .btn:focus-visible,
  summary:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .alert {
    border: 1px solid var(--destructive);
    background: var(--destructive-dim);
    border-radius: 6px;
    padding: 8px 10px;
  }
  .alert pre {
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
  }
  summary {
    cursor: pointer;
    color: var(--muted-foreground);
  }
  .cmd {
    margin: 8px 0;
    padding: 8px 10px;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 6px;
    white-space: pre-wrap;
    word-break: break-all;
  }
</style>
