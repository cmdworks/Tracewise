import DefaultTheme from "vitepress/theme";
import type { Theme } from "vitepress";
import HomeShowcase from "./HomeShowcase.vue";
import "./custom.css";

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component("HomeShowcase", HomeShowcase);
  },
} satisfies Theme;
