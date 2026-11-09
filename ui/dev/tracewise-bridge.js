// @ts-nocheck (a Node dev tool, not part of the app)
// Dev-only bridge: lets `vite dev` serve the UI in an ordinary browser against the real `tracewise` binary, the way the desktop app's
// `tracewise` command does (same allow-list, only repositories Tracewise knows, bounded output). Enabled by TRACEWISE_UI_BRIDGE=1; nothing
// of it is in the production build or the desktop app.
//   TRACEWISE_UI_BRIDGE=1 TRACEWISE_BIN=/path/to/tracewise TRACEWISE_HOME=/path/to/home pnpm dev
import { execFile } from "node:child_process";
import { readFileSync, realpathSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";

function allowList() {
  const rust = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
  const block = rust.match(/ALLOWED_COMMANDS: &\[&str\] = &\[([\s\S]*?)\];/)[1];
  return [...block.matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

function registered(home) {
  try {
    return readFileSync(join(home, "repos"), "utf8")
      .split("\n")
      .map((l) => l.split("\t")[0])
      .filter(Boolean)
      .map((p) => realpathSync(p));
  } catch {
    return [];
  }
}

// Runs before the app's first call: what `@tauri-apps/api` expects to find in the desktop app's window.
const INJECT = `
if (typeof window !== "undefined" && !window.__TAURI_INTERNALS__) {
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
    transformCallback: () => 0,
    invoke: async (cmd, args) => {
      const r = await fetch("/__tracewise", { method: "POST", body: JSON.stringify({ cmd, args }) });
      const j = await r.json();
      if (j.error) throw j.error;
      return j.result;
    },
  };
  // for measurements (dev only): when the first timeline row and the first long task appeared, from navigation start
  window.__perf = { firstRowAt: null, longTasks: [] };
  new MutationObserver((_m, obs) => {
    if (document.querySelector(".timeline-row")) { window.__perf.firstRowAt = performance.now(); obs.disconnect(); }
  }).observe(document, { childList: true, subtree: true });
  try { new PerformanceObserver((l) => l.getEntries().forEach((e) => window.__perf.longTasks.push([Math.round(e.startTime), Math.round(e.duration)]))).observe({ entryTypes: ["longtask"] }); } catch {}
}
`;

export function tracewiseBridge() {
  if (process.env.TRACEWISE_UI_BRIDGE !== "1") return { name: "tracewise-bridge-off" };
  const bin = process.env.TRACEWISE_BIN;
  const home = process.env.TRACEWISE_HOME;
  if (!bin || !home) throw new Error("TRACEWISE_UI_BRIDGE=1 needs TRACEWISE_BIN and TRACEWISE_HOME");
  const allowed = allowList();
  return {
    name: "tracewise-bridge",
    // SvelteKit renders the HTML itself, so the stand-in is put in front of the one module that calls Tauri
    transform(code, id) {
      return id.replace(/\\/g, "/").endsWith("/src/lib/api.ts") ? INJECT + code : null;
    },
    configureServer(server) {
      server.middlewares.use("/__snapshot.js", (_req, res) => {
        res.setHeader("content-type", "text/javascript");
        res.end(readFileSync(new URL("./snapshot.js", import.meta.url), "utf8"));
      });
      // snapshots of the page for before/after comparison of a refactor (see dev/snapshot.js)
      server.middlewares.use("/__snapshot", (req, res) => {
        let body = "";
        req.on("data", (c) => (body += c));
        req.on("end", () => {
          const { label, out } = JSON.parse(body);
          const path = `/tmp/ui-snap-${String(label).replace(/[^\w-]/g, "")}.json`;
          writeFileSync(path, JSON.stringify(out));
          res.setHeader("content-type", "application/json");
          res.end(JSON.stringify({ path }));
        });
      });
      server.middlewares.use("/__tracewise", (req, res) => {
        let body = "";
        req.on("data", (c) => (body += c));
        req.on("end", () => {
          const reply = (obj) => {
            res.setHeader("content-type", "application/json");
            res.end(JSON.stringify(obj));
          };
          try {
            const { cmd, args } = JSON.parse(body);
            if (cmd !== "tracewise") return reply({ result: null });
            const argv = args.args ?? [];
            if (!allowed.includes(argv[0])) return reply({ error: `subcommand not permitted: '${argv[0]}'` });
            let cwd = registered(home)[0] ?? process.cwd(); // like the app, a call without a repository uses a known one
            // like the desktop side: a repository that is given must exist and be one Tracewise knows; an empty one does not exist
            if (args.repo !== null && args.repo !== undefined) {
              let real;
              try {
                real = realpathSync(args.repo);
              } catch {
                return reply({ error: `repository path does not exist: '${args.repo}'` });
              }
              if (!registered(home).includes(real)) return reply({ error: `not a repository Tracewise knows: ${args.repo}` });
              cwd = real;
            }
            // the desktop app is a person: nothing from an agent session that happens to start the dev server may leak in
            const env = { ...process.env, TRACEWISE_HOME: home };
            for (const k of ["CLAUDECODE", "CLAUDE_CODE_SESSION_ID", "ANTIGRAVITY_AGENT", "ANTIGRAVITY_CONVERSATION_ID", "TRACEWISE_ACTOR", "TRACEWISE_WHY"]) delete env[k];
            execFile(bin, argv, { cwd, env, timeout: 120000, maxBuffer: 8 << 20 }, (err, stdout, stderr) => {
              reply({ result: { stdout, stderr, exit_code: err ? (typeof err.code === "number" ? err.code : 1) : 0 } });
            });
          } catch (e) {
            reply({ error: String(e) });
          }
        });
      });
    },
  };
}
