import { defineConfig } from "vitepress";

// GitHub Pages serves a project at /<repository>/; SITE_BASE overrides it (a custom domain uses "/")
const base = process.env.SITE_BASE ?? "/Tracewise/";

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
      { text: "Guide", link: "/getting-started", activeMatch: "/(getting-started|how-it-works|desktop-app)" },
      { text: "Reference", link: "/commands", activeMatch: "/(commands|configuration|sharing)" },
      { text: "Changelog", link: "/changelog" },
    ],
    sidebar: [
      {
        text: "Guide",
        items: [
          { text: "Getting started", link: "/getting-started" },
          { text: "How it works", link: "/how-it-works" },
          { text: "Desktop app", link: "/desktop-app" },
        ],
      },
      {
        text: "Reference",
        items: [
          { text: "Commands", link: "/commands" },
          { text: "Configuration", link: "/configuration" },
          { text: "Sharing", link: "/sharing" },
        ],
      },
      {
        text: "Project",
        items: [
          { text: "Changelog", link: "/changelog" },
          { text: "Contributing", link: "/contributing" },
        ],
      },
    ],
    socialLinks: [{ icon: "github", link: "https://github.com/cmdworks/Tracewise" }],
    search: { provider: "local" },
    editLink: { pattern: "https://github.com/cmdworks/Tracewise/edit/main/docs/:path", text: "Edit this page on GitHub" },
    footer: { message: "Released under the GNU GPL v3 or later.", copyright: "Copyright © 2026 Asmat" },
    outline: { level: [2, 3] },
  },
});
