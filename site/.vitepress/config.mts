import { defineConfig } from "vitepress";

// GitHub Pages serves a project at /<repository>/; SITE_BASE overrides it (a custom domain uses "/")
const base = process.env.SITE_BASE ?? "/tracewise/";

export default defineConfig({
  title: "Tracewise",
  description: "A flight recorder for code changes by people and coding agents: who changed every line, when and why. Recorded, explained, reversible.",
  lang: "en",
  base,
  srcDir: "./src",
  outDir: "./dist",
  cleanUrls: true,
  lastUpdated: true,
  head: [
    ["link", { rel: "icon", type: "image/svg+xml", href: `${base}logo.svg` }],
    ["meta", { name: "theme-color", content: "#e06324" }],
  ],
  themeConfig: {
    logo: "/logo.svg",
    siteTitle: "Tracewise",
    nav: [
      { text: "Guide", link: "/docs/getting-started", activeMatch: "/docs/(getting-started|how-it-works|desktop-app)" },
      { text: "Reference", link: "/docs/commands", activeMatch: "/docs/(commands|configuration|sharing)" },
      { text: "Changelog", link: "/docs/changelog" },
    ],
    sidebar: {
      "/docs/": [
      {
        text: "Guide",
        items: [
          { text: "Getting started", link: "/docs/getting-started" },
          { text: "How it works", link: "/docs/how-it-works" },
          { text: "Desktop app", link: "/docs/desktop-app" },
        ],
      },
      {
        text: "Reference",
        items: [
          { text: "Commands", link: "/docs/commands" },
          { text: "Configuration", link: "/docs/configuration" },
          { text: "Sharing", link: "/docs/sharing" },
        ],
      },
      {
        text: "Project",
        items: [
          { text: "Roadmap", link: "/docs/roadmap" },
          { text: "Changelog", link: "/docs/changelog" },
          { text: "Contributing", link: "/docs/contributing" },
        ],
      },
    ],
    },
    socialLinks: [{ icon: "github", link: "https://github.com/cmdworks/tracewise" }],
    search: { provider: "local" },
    editLink: {
      // pages generated from root files are edited there
      pattern: ({ filePath }) => {
        const root: Record<string, string> = { "docs/desktop-app.md": "UI.md", "docs/changelog.md": "CHANGELOG.md", "docs/contributing.md": "CONTRIBUTING.md" };
        return `https://github.com/cmdworks/tracewise/edit/main/${root[filePath] ?? filePath.replace(/^index\.md$/, "site/index.md")}`;
      },
      text: "Edit this page on GitHub",
    },
    footer: { message: "Released under the GNU GPL v3 or later.", copyright: "Copyright © 2026 Asmat" },
    outline: { level: [2, 3] },
  },
});
