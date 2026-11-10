import { h } from "vue";
import DefaultTheme from "vitepress/theme";
import type { Theme } from "vitepress";
import HomeShowcase from "./HomeShowcase.vue";
import TerminalDemo from "./TerminalDemo.vue";
import "./custom.css";

// the animated demo takes the hero's picture slot (first thing on the page); the CLI/desktop tabs follow the hero, then the feature cards
export default {
  extends: DefaultTheme,
  Layout: () =>
    h(DefaultTheme.Layout, null, {
      "home-hero-image": () => h("div", { class: "hero-demo" }, [h(TerminalDemo)]),
      "home-hero-after": () => h(HomeShowcase),
    }),
} satisfies Theme;
