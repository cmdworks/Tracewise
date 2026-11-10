// Builds the site's source folder `site/src` from one place each: the pages in docs/, plus a few root files and the logo.
// `site/src` is git-ignored and rebuilt every time. Links to root files and the repository are rewritten so they work on GitHub and on the site.
import { copyFileSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "site/src");
const repo = "https://github.com/cmdworks/tracewise/blob/main/";

// Root files are written for GitHub: a link into docs/ becomes a site path (keeping any #anchor), any other link a GitHub URL
function fixLinks(text) {
  return text.replace(/\]\((?!https?:|#|mailto:)([^)\s]+)\)/g, (_m, target) => {
    const clean = target.replace(/^(\.\.\/|\.\/)+/, "");
    if (clean.startsWith("docs/") && /\.md(#|$)/.test(clean)) return `](./${clean.slice(5).replace(/\.md/, "")})`;
    return `](${repo}${clean})`;
  });
}

rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, "public/icons"), { recursive: true });
mkdirSync(join(out, "docs"), { recursive: true });
for (const f of readdirSync(join(root, "docs")).filter((f) => f.endsWith(".md"))) copyFileSync(join(root, "docs", f), join(out, "docs", f));

const pages = [
  ["UI.md", "desktop-app.md"],
  ["CHANGELOG.md", "changelog.md"],
  ["CONTRIBUTING.md", "contributing.md"],
];
for (const [src, dst, kind] of pages) {
  writeFileSync(join(out, "docs", dst), fixLinks(readFileSync(join(root, src), "utf8")));
}
copyFileSync(join(root, "site/index.md"), join(out, "index.md"));
copyFileSync(join(root, "site/docs-index.md"), join(out, "docs/index.md"));
copyFileSync(join(root, "assets/logo.svg"), join(out, "public/logo.svg"));

// screenshots of the desktop app for the home page
mkdirSync(join(out, "public/screens"), { recursive: true });
for (const f of readdirSync(join(root, "site/assets"))) copyFileSync(join(root, "site/assets", f), join(out, "public/screens", f));

// Phosphor duotone icons (MIT) for the home page feature cards, in the brand color so they show on light and dark backgrounds
const icons = { search: "magnifying-glass", undo: "arrow-counter-clockwise", lock: "lock-key", verified: "seal-check", share: "share-network", desktop: "app-window" };
for (const [name, file] of Object.entries(icons)) {
  const svg = readFileSync(join(root, "site/node_modules/@phosphor-icons/core/assets/duotone", `${file}-duotone.svg`), "utf8").replace('fill="currentColor"', 'fill="#e06324"');
  writeFileSync(join(out, "public/icons", `${name}.svg`), svg);
}
