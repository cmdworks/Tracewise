// Builds the site's source folder `site/src` from one place each: the pages in docs/, plus a few root files and the logo.
// `site/src` is git-ignored and rebuilt every time. Links to root files and the repository are rewritten so they work on GitHub and on the site.
import { copyFileSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "site/src");
const repo = "https://github.com/cmdworks/Tracewise/blob/main/";

// Root files are written for GitHub: a link into docs/ becomes a site path (keeping any #anchor), any other link a GitHub URL
function fixLinks(text) {
  return text.replace(/\]\((?!https?:|#|mailto:)([^)\s]+)\)/g, (_m, target) => {
    const clean = target.replace(/^(\.\.\/|\.\/)+/, "");
    if (clean.startsWith("docs/") && /\.md(#|$)/.test(clean)) return `](./${clean.slice(5).replace(/\.md/, "")})`;
    return `](${repo}${clean})`;
  });
}

rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, "public"), { recursive: true });
for (const f of readdirSync(join(root, "docs")).filter((f) => f.endsWith(".md"))) copyFileSync(join(root, "docs", f), join(out, f));

const pages = [
  ["UI.md", "desktop-app.md"],
  ["CHANGELOG.md", "changelog.md"],
  ["CONTRIBUTING.md", "contributing.md"],
];
for (const [src, dst, kind] of pages) {
  writeFileSync(join(out, dst), fixLinks(readFileSync(join(root, src), "utf8")));
}
copyFileSync(join(root, "site/index.md"), join(out, "index.md"));
copyFileSync(join(root, "assets/logo.svg"), join(out, "public/logo.svg"));
