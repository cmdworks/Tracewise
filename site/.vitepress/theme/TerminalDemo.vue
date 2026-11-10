<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import pauseIcon from "@phosphor-icons/core/assets/bold/pause-bold.svg?raw";
import playIcon from "@phosphor-icons/core/assets/bold/play-bold.svg?raw";

// The home page's terminal: blame, why, undo. Output is the project's own README example; nothing here is measured or live.
const CMD1 = "tracewise blame src/parser.rs";
const CMD2 = "tracewise why src/parser.rs:2";
const CMD3 = "tracewise undo --actor claude --since 1h";
const NOTE = "  # take it all back; your own edits stay";

const BLAME = [
  ["1", "claude/s1", "cl", "fn parse(input: &str) -> Result<Ast> {"],
  ["2", "claude/s1", "cl", "    let tokens = lex(input)?;"],
  ["3", "baseline", "bs", "    build(tokens)"],
  ["4", "baseline", "bs", "}"],
];

const typed1 = ref("");
const typed2 = ref("");
const typed3 = ref("");
const note = ref("");
const blameLines = ref(0); // 0-4 lines of blame output shown
const why = ref(false);
const undone = ref(false);
const kept = ref(false);
const paused = ref(false);
const reduced = ref(false);
const root = ref<HTMLElement | null>(null);

let run = 0; // bumped to cancel a running show
let visible = false;
let observer: IntersectionObserver | null = null;

class Cancelled extends Error {}

function sleep(ms: number, token: number) {
  return new Promise<void>((resolve, reject) => {
    const tick = (left: number) => {
      if (token !== run) return reject(new Cancelled());
      if (paused.value || !visible) return void setTimeout(() => tick(left), 120);
      if (left <= 0) return resolve();
      setTimeout(() => tick(left - 40), 40);
    };
    tick(ms);
  });
}

async function type(text: string, target: { value: string }, token: number, cps = 38) {
  for (let i = 1; i <= text.length; i++) {
    target.value = text.slice(0, i);
    await sleep(1000 / cps, token);
  }
}

function resetAll() {
  typed1.value = typed2.value = typed3.value = note.value = "";
  blameLines.value = 0;
  why.value = undone.value = kept.value = false;
}

function showEnd() {
  typed1.value = CMD1;
  typed2.value = CMD2;
  typed3.value = CMD3;
  note.value = NOTE;
  blameLines.value = 4;
  why.value = true;
  undone.value = true;
  kept.value = true;
}

async function show() {
  const token = ++run;
  try {
    for (;;) {
      resetAll();
      await sleep(600, token);
      await type(CMD1, typed1, token);
      await sleep(250, token);
      for (let i = 1; i <= 4; i++) {
        blameLines.value = i;
        await sleep(300, token);
      }
      await sleep(900, token);
      await type(CMD2, typed2, token, 44);
      await sleep(300, token);
      why.value = true;
      await sleep(2600, token);
      await type(CMD3, typed3, token, 44);
      await type(NOTE, note, token, 70);
      await sleep(500, token);
      undone.value = true;
      await sleep(900, token);
      kept.value = true;
      await sleep(4200, token);
    }
  } catch (e) {
    if (!(e instanceof Cancelled)) throw e;
  }
}

function toggle() {
  paused.value = !paused.value;
}

onMounted(() => {
  reduced.value = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (reduced.value) return showEnd();
  observer = new IntersectionObserver(([entry]) => (visible = entry.isIntersecting), { threshold: 0.2 });
  if (root.value) observer.observe(root.value);
  show();
});
onBeforeUnmount(() => {
  run++;
  observer?.disconnect();
});

const label = computed(() => (paused.value ? "Play the animation" : "Pause the animation"));
</script>

<template>
  <div ref="root" class="demo">
    <div class="term" aria-hidden="true">
      <div class="bar"><i /><i /><i /><span class="path">~/api</span></div>
      <div class="body">
        <div class="ln"><span class="p">$</span> {{ typed1 }}<span v-if="typed1 !== CMD1" class="caret" /></div>
        <div v-for="(l, i) in BLAME" :key="i" class="ln blame" :class="{ on: blameLines > i }">
          <span class="no">{{ l[0] }}</span><span class="who" :class="l[2]">{{ l[1] }}</span><span class="sep">│</span><span>{{ l[3] }}</span>
        </div>
        <div class="ln gap"><span v-if="typed2" class="p">$</span> {{ typed2 }}<span v-if="typed2 && typed2 !== CMD2" class="caret" /></div>
        <div class="whyline" :class="{ on: why }"><b>Why:</b> handle empty input without panicking</div>
        <div class="ln gap"><span v-if="typed3" class="p">$</span> {{ typed3 }}<span class="cm">{{ note }}</span><span v-if="typed3 && note !== NOTE" class="caret" /></div>
      </div>
    </div>

    <div class="file" aria-hidden="true">
      <div class="fh"><span>src/parser.rs</span><span>who wrote each line</span></div>
      <div class="rows">
        <div class="fr" :class="{ gone: undone }"><i class="bar cl" /><span class="no">1</span><span class="who cl">claude/s1</span><span class="code">fn parse(input: &amp;str) -&gt; Result&lt;Ast&gt; {</span></div>
        <div class="fr" :class="{ gone: undone }"><i class="bar cl" /><span class="no">2</span><span class="who cl">claude/s1</span><span class="code">    let tokens = lex(input)?;</span></div>
        <div class="fr"><i class="bar bs" /><span class="no">3</span><span class="who bs">baseline</span><span class="code">    build(tokens)</span><span class="kept" :class="{ on: kept }">kept</span></div>
        <div class="fr"><i class="bar bs" /><span class="no">4</span><span class="who bs">baseline</span><span class="code">}</span><span class="kept" :class="{ on: kept }">kept</span></div>
      </div>
    </div>

    <!-- the same story for screen readers, once -->
    <p class="sr">
      A terminal session. tracewise blame src/parser.rs shows four lines: lines 1 and 2 were written by claude/s1, lines 3 and 4 are baseline.
      tracewise why src/parser.rs:2 answers: handle empty input without panicking. tracewise undo --actor claude --since 1h takes the agent's
      lines back and your own edits stay.
    </p>

    <button v-if="!reduced" type="button" class="pause" :aria-label="label" :aria-pressed="paused" @click="toggle">
      <span aria-hidden="true" v-html="paused ? playIcon : pauseIcon" />
    </button>
  </div>
</template>

<style scoped>
.demo {
  position: relative;
  display: grid;
  gap: 16px;
}
.term,
.file {
  overflow: hidden;
  background: #14161b;
  border: 1px solid #2a2e38;
  border-radius: 12px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
  color: #e6e8ee;
}
.bar,
.fh {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 10px 14px;
  background: #1b1e25;
  border-bottom: 1px solid #2a2e38;
}
.bar i {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: #3b404d;
}
.path,
.fh {
  font: 12px var(--vp-font-family-mono);
  color: #9aa0ad;
}
.path {
  margin-left: 10px;
}
.fh {
  justify-content: space-between;
}
.body {
  min-height: 268px;
  padding: 14px 16px;
  font: 13px/1.75 var(--vp-font-family-mono);
  white-space: pre;
  overflow-x: auto;
}
.ln {
  min-height: 1.75em;
}
.gap {
  margin-top: 10px;
}
.p {
  color: #e06324;
  user-select: none;
}
.cm {
  color: #9aa0ad;
}
.caret {
  display: inline-block;
  width: 7px;
  height: 14px;
  margin-left: 1px;
  vertical-align: -2px;
  background: #e06324;
  animation: blink 1s steps(1) infinite;
}
@keyframes blink {
  50% {
    opacity: 0;
  }
}
.blame {
  opacity: 0;
  transform: translateY(4px);
  transition: opacity 0.25s, transform 0.25s;
}
.blame.on {
  opacity: 1;
  transform: none;
}
.no {
  display: inline-block;
  width: 3ch;
  color: #5b6272;
}
.who {
  display: inline-block;
  width: 11ch;
  font-weight: 600;
}
.cl {
  color: #f59e0b;
}
.bs {
  color: #9aa0ad;
}
.sep {
  display: inline-block;
  width: 3ch;
  color: #3b404d;
}
.whyline {
  margin-top: 10px;
  padding: 8px 12px;
  border-left: 4px solid #e06324;
  background: rgba(224, 99, 36, 0.12);
  border-radius: 0 8px 8px 0;
  font: 600 14px var(--vp-font-family-base);
  white-space: normal;
  opacity: 0;
  transition: opacity 0.3s;
}
.whyline b {
  color: #e06324;
}
.whyline.on {
  opacity: 1;
}
.rows {
  font: 13px var(--vp-font-family-mono);
}
.fr {
  display: flex;
  align-items: center;
  gap: 0;
  max-height: 40px;
  min-height: 0;
  height: 40px;
  overflow: hidden;
  border-bottom: 1px solid #2a2e38;
  transition: max-height 0.4s ease, height 0.4s ease, opacity 0.3s, transform 0.4s;
  white-space: pre;
}
.fr.gone {
  max-height: 0;
  height: 0;
  opacity: 0;
  transform: translateX(-16px);
  border-bottom-width: 0;
}
.bar.cl,
.bar.bs {
  display: inline-block;
  width: 5px;
  height: 40px;
  margin-right: 14px;
  border-radius: 0;
}
.bar.cl {
  background: #f59e0b;
}
.bar.bs {
  background: #5b6272;
}
.fr .no {
  width: 2.5ch;
}
.fr .who {
  width: 12ch;
}
.code {
  color: #e6e8ee;
}
.kept {
  margin: 0 12px 0 auto;
  padding: 2px 10px;
  border: 1px solid rgba(52, 211, 153, 0.4);
  border-radius: 999px;
  background: rgba(52, 211, 153, 0.12);
  color: #34d399;
  font: 600 12px var(--vp-font-family-base);
  opacity: 0;
  transition: opacity 0.3s;
}
.kept.on {
  opacity: 1;
}
.pause {
  position: absolute;
  top: 8px;
  right: 8px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: #9aa0ad;
  cursor: pointer;
}
.pause:hover {
  background: rgba(255, 255, 255, 0.08);
  color: #e6e8ee;
}
.pause:focus-visible {
  outline: 2px solid #e06324;
  outline-offset: 0;
}
.pause span {
  display: inline-flex;
  width: 18px;
  height: 18px;
}
.pause span :deep(svg) {
  width: 100%;
  height: 100%;
}
.sr {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
}
@media (prefers-reduced-motion: reduce) {
  .blame,
  .whyline,
  .fr,
  .kept {
    transition: none;
  }
  .caret {
    animation: none;
  }
}
</style>
