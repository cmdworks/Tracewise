<script lang="ts">
  import "@fontsource/ibm-plex-sans/400.css";
  import "@fontsource/ibm-plex-sans/500.css";
  import "@fontsource/ibm-plex-sans/600.css";
  import "@fontsource/ibm-plex-sans/700.css";
  import "@fontsource/jetbrains-mono/400.css";
  import "@fontsource/jetbrains-mono/600.css";
  import "@fontsource/jetbrains-mono/700.css";
  import "$lib/theme.css";

  let { children } = $props();
</script>

{@render children()}
