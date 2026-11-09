// Dev-only: records what the page shows (normalised HTML and the computed style of every element) for each tab and dialog, so a
// refactor can be compared before and after. Used from the browser console of `TRACEWISE_UI_BRIDGE=1 pnpm dev`:
//   const { run } = await import("/__snapshot.js"); await run("before");   // writes /tmp/ui-snap-before.json
const PROPS = ["display", "position", "color", "background-color", "font-family", "font-size", "font-weight", "line-height", "letter-spacing",
  "margin-top", "margin-right", "margin-bottom", "margin-left", "padding-top", "padding-right", "padding-bottom", "padding-left",
  "border-top-width", "border-top-color", "border-bottom-width", "border-left-width", "border-right-width", "border-top-left-radius",
  "opacity", "overflow-x", "overflow-y", "flex-direction", "justify-content", "align-items", "gap", "text-align", "text-transform",
  "box-shadow", "cursor", "z-index", "white-space", "visibility", "grid-template-columns"];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const key = (e, i) => `${i}:${e.tagName.toLowerCase()}.${[...e.classList].filter((c) => !/^svelte-/.test(c)).join(".")}`;
const tidy = (s) => s.replace(/\bsvelte-[a-z0-9]+\b/g, "").replace(/ class=""/g, "").replace(/\d{1,2}:\d\d:\d\d(\s?[AP]M)?/g, "T").replace(/\s+/g, " ");

function capture() {
  const els = [...document.body.querySelectorAll("*")];
  const styles = els.map((e, i) => {
    const cs = getComputedStyle(e), r = e.getBoundingClientRect();
    return [key(e, i), PROPS.map((p) => cs.getPropertyValue(p)).join("|"), [r.x, r.y, r.width, r.height].map(Math.round).join(",")];
  });
  return { html: tidy(document.body.innerHTML), text: document.body.innerText.replace(/\d{1,2}:\d\d:\d\d(\s?[AP]M)?/g, "T"), styles };
}
const press = (k, o = {}) => window.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, ...o }));
const clickText = (rx) => [...document.querySelectorAll("button")].find((b) => rx.test(b.innerText))?.click();

export async function run(label) {
  const out = {};
  const snap = async (name, ms = 500) => { await sleep(ms); out[name] = capture(); };
  await sleep(2500);
  await snap("timeline");
  press("j"); press("j"); await snap("timeline-selected");
  press("u"); await snap("undo-modal", 900); press("Escape");
  press("?"); await snap("shortcuts-modal"); press("Escape");
  for (const [name, k] of [["now", "2"], ["files", "3"], ["undo", "4"], ["projects", "5"], ["health", "6"]]) { press(k); await snap(name, 1200); }
  press("1"); await sleep(300); clickText(/JSON/); await snap("json-modal", 900); press("Escape");
  const r = await fetch("/__snapshot", { method: "POST", body: JSON.stringify({ label, out }) });
  return { saved: (await r.json()).path, states: Object.keys(out), elements: Object.fromEntries(Object.entries(out).map(([k, v]) => [k, v.styles.length])) };
}
