<script setup lang="ts">
import { ref, nextTick } from "vue";
import { withBase } from "vitepress";
import terminal from "@phosphor-icons/core/assets/duotone/terminal-window-duotone.svg?raw";
import appWindow from "@phosphor-icons/core/assets/duotone/app-window-duotone.svg?raw";
import copyIcon from "@phosphor-icons/core/assets/duotone/copy-duotone.svg?raw";
import checkIcon from "@phosphor-icons/core/assets/duotone/check-circle-duotone.svg?raw";
import download from "@phosphor-icons/core/assets/duotone/download-simple-duotone.svg?raw";
import TerminalDemo from "./TerminalDemo.vue";
import book from "@phosphor-icons/core/assets/duotone/book-open-duotone.svg?raw";

const installCmd = "curl -fsSL https://raw.githubusercontent.com/cmdworks/tracewise/main/scripts/install.sh | sh";
const tabs = [
  { id: "cli", label: "CLI", icon: terminal },
  { id: "app", label: "Desktop app", icon: appWindow },
] as const;
const active = ref<"cli" | "app">("cli");
const copied = ref(false);
const buttons = ref<HTMLButtonElement[]>([]);

// arrow keys move between tabs, as in the ARIA tabs pattern
async function onKey(e: KeyboardEvent, i: number) {
  const next = e.key === "ArrowRight" ? i + 1 : e.key === "ArrowLeft" ? i - 1 : e.key === "Home" ? 0 : e.key === "End" ? tabs.length - 1 : null;
  if (next === null) return;
  e.preventDefault();
  const n = (next + tabs.length) % tabs.length;
  active.value = tabs[n].id;
  await nextTick();
  buttons.value[n]?.focus();
}

async function copy() {
  try {
    await navigator.clipboard.writeText(installCmd);
    copied.value = true;
    setTimeout(() => (copied.value = false), 1800);
  } catch {
    // the command is on screen; copying is a convenience
  }
}
</script>

<template>
  <section id="showcase" class="showcase" aria-labelledby="showcase-title">
    <h2 id="showcase-title">Use it your way</h2>
    <p class="lead">The same record, from the terminal or in a window. Both read what Tracewise stores in your repository.</p>

    <div class="tablist" role="tablist" aria-label="How to use Tracewise">
      <button
        v-for="(t, i) in tabs"
        :key="t.id"
        :ref="(el) => (buttons[i] = el as HTMLButtonElement)"
        :id="`tab-${t.id}`"
        role="tab"
        type="button"
        :aria-selected="active === t.id"
        :aria-controls="`panel-${t.id}`"
        :tabindex="active === t.id ? 0 : -1"
        :class="{ on: active === t.id }"
        @click="active = t.id"
        @keydown="onKey($event, i)"
      >
        <span class="ico" aria-hidden="true" v-html="t.icon" />
        {{ t.label }}
      </button>
    </div>

    <div v-show="active === 'cli'" id="panel-cli" role="tabpanel" aria-labelledby="tab-cli" tabindex="0" class="panel">
      <TerminalDemo />
      <div class="side">
        <ul class="points">
          <li><b>Every command speaks JSON</b> with <code>--json</code>, for scripts and agents.</li>
          <li><b>Hooks for your agents</b>: Claude Code, Codex, Cursor, Gemini, Antigravity.</li>
          <li><b>Shell and git wrappers</b> attribute changes made by commands and scripts.</li>
        </ul>
        <div class="cmd">
          <code>{{ installCmd }}</code>
          <button type="button" class="copy" :aria-label="copied ? 'Copied' : 'Copy the install command'" @click="copy">
            <span aria-hidden="true" v-html="copied ? checkIcon : copyIcon" />
          </button>
          <span class="sr" role="status">{{ copied ? "Copied to the clipboard" : "" }}</span>
        </div>
        <p class="note">macOS, Apple silicon and Intel. No <code>sudo</code>.</p>
        <div class="actions">
          <a class="btn primary" :href="withBase('/docs/getting-started')"><span aria-hidden="true" v-html="book" />Getting started</a>
          <a class="btn" :href="withBase('/docs/commands')">All commands</a>
        </div>
      </div>
    </div>

    <div v-show="active === 'app'" id="panel-app" role="tabpanel" aria-labelledby="tab-app" tabindex="0" class="panel app">
      <div class="shots">
        <figure class="shot">
          <div class="bar"><i /><i /><i /></div>
          <img :src="withBase('/screens/app-timeline.png')" width="1920" height="1200" loading="lazy" alt="The Tracewise desktop app: a timeline of changes with who made each one and why, and the selected change's diff on the right" />
        </figure>
        <figure class="shot">
          <div class="bar"><i /><i /><i /></div>
          <img :src="withBase('/screens/app-blame.png')" width="1920" height="1200" loading="lazy" alt="The Files tab: each line of a file with the agent that wrote it and the reason" />
        </figure>
      </div>
      <div class="side">
        <ul class="points">
          <li><b>Live timeline</b> of every change, with its author and reason.</li>
          <li><b>Line by line</b>: who wrote each line, and why.</li>
          <li><b>Undo with a preview</b>, then redo if you change your mind.</li>
        </ul>
        <p class="note">macOS disk image, preview. It offers to install the command line if it is missing.</p>
        <div class="actions">
          <a class="btn primary" href="https://github.com/cmdworks/tracewise/releases/latest"><span aria-hidden="true" v-html="download" />Download for macOS</a>
          <a class="btn" :href="withBase('/docs/desktop-app')">Desktop app guide</a>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.showcase {
  max-width: 1152px;
  margin: 0 auto;
  padding: 48px 24px 24px;
}
h2 {
  margin: 0 0 8px;
  border: 0;
  padding: 0;
  font-size: 28px;
  line-height: 1.25;
  letter-spacing: -0.01em;
}
.lead {
  margin: 0 0 24px;
  max-width: 62ch;
  color: var(--vp-c-text-2);
}
.tablist {
  display: inline-flex;
  gap: 4px;
  padding: 4px;
  margin-bottom: 20px;
  background: var(--vp-c-bg-soft);
  border: 1px solid var(--vp-c-divider);
  border-radius: 12px;
}
.tablist button {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-height: 44px;
  padding: 0 16px;
  border: 0;
  border-radius: 9px;
  background: transparent;
  color: var(--vp-c-text-2);
  font: inherit;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.tablist button:hover {
  color: var(--vp-c-text-1);
}
.tablist button.on {
  background: var(--vp-c-bg);
  color: var(--vp-c-text-1);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
}
.tablist button:focus-visible,
.btn:focus-visible,
.copy:focus-visible,
.panel:focus-visible {
  outline: 2px solid var(--vp-c-brand-1);
  outline-offset: 2px;
}
.ico,
.btn span,
.copy span {
  display: inline-flex;
  width: 20px;
  height: 20px;
  color: var(--vp-c-brand-3);
}
.ico :deep(svg),
.btn span :deep(svg),
.copy span :deep(svg) {
  width: 100%;
  height: 100%;
}
.panel {
  display: grid;
  gap: 24px;
  grid-template-columns: 1fr;
  border-radius: 12px;
}
@media (min-width: 960px) {
  .panel {
    grid-template-columns: minmax(0, 1.35fr) minmax(0, 1fr);
    align-items: center;
  }
}
.term,
.shot {
  margin: 0;
  overflow: hidden;
  background: #14161b;
  border: 1px solid #2a2e38;
  border-radius: 12px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
}
.bar {
  display: flex;
  gap: 6px;
  padding: 10px 12px;
  background: #1b1e25;
  border-bottom: 1px solid #2a2e38;
}
.bar i {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: #3b404d;
}
.term pre {
  margin: 0;
  padding: 16px;
  overflow-x: auto;
  color: #e6e8ee;
  font: 13px/1.6 var(--vp-font-family-mono);
}
.term .p {
  color: #e06324;
  user-select: none;
}
.term .why {
  color: #9aa0ad;
}
.shots {
  display: grid;
  gap: 16px;
}
@media (min-width: 560px) and (max-width: 959px) {
  .shots {
    grid-template-columns: 1fr 1fr;
  }
}
.shot img {
  display: block;
  width: 100%;
  height: auto;
}
.points {
  margin: 0 0 16px;
  padding: 0;
  list-style: none;
  display: grid;
  gap: 12px;
}
.points li {
  padding-left: 14px;
  border-left: 2px solid var(--vp-c-brand-3);
  color: var(--vp-c-text-2);
  line-height: 1.55;
}
.points b {
  color: var(--vp-c-text-1);
}
.cmd {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 8px 8px 14px;
  background: var(--vp-c-bg-soft);
  border: 1px solid var(--vp-c-divider);
  border-radius: 10px;
}
.cmd code {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
  font-size: 12.5px;
  line-height: 1.5;
  background: none;
  color: var(--vp-c-text-1);
}
.copy {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  flex: none;
  border: 0;
  border-radius: 8px;
  background: transparent;
  cursor: pointer;
}
.copy:hover {
  background: var(--vp-c-default-soft);
}
.note {
  margin: 10px 0 16px;
  font-size: 13px;
  color: var(--vp-c-text-3);
}
.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}
.btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-height: 44px;
  padding: 0 18px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 10px;
  color: var(--vp-c-text-1);
  font-weight: 600;
  text-decoration: none;
  transition: border-color 0.15s, background 0.15s;
}
.btn:hover {
  border-color: var(--vp-c-brand-3);
}
.btn.primary {
  background: #e06324;
  border-color: #e06324;
  color: #1a0d05;
}
.btn.primary span {
  color: #1a0d05;
}
.btn.primary:hover {
  background: #cf5712;
}
.sr {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
}
@media (max-width: 480px) {
  .tablist {
    display: flex;
    width: 100%;
  }
  .tablist button {
    flex: 1;
    justify-content: center;
    padding: 0 8px;
    white-space: nowrap;
  }
}
@media (prefers-reduced-motion: reduce) {
  .tablist button,
  .btn {
    transition: none;
  }
}
</style>
