<script lang="ts">
  import { onMount, type Snippet } from "svelte";

  interface Props {
    open: boolean;
    onclose: () => void;
    trigger: Snippet;
    children: Snippet;
    align?: "left" | "right";
    width?: string;
  }

  let {
    open = $bindable(false),
    onclose,
    trigger,
    children,
    align = "left",
    width = "320px"
  }: Props = $props();

  let containerEl: HTMLDivElement | null = $state(null);

  function handleWindowClick(e: MouseEvent) {
    if (open && containerEl && !containerEl.contains(e.target as Node)) {
      open = false;
      onclose();
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (open && e.key === "Escape") {
      e.stopPropagation();
      open = false;
      onclose();
    }
  }

  onMount(() => {
    window.addEventListener("click", handleWindowClick, true);
    window.addEventListener("keydown", handleKeyDown, true);
    return () => {
      window.removeEventListener("click", handleWindowClick, true);
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  });
</script>

<div bind:this={containerEl} class="popover-wrapper">
  {@render trigger()}

  {#if open}
    <div
      class="popover-menu {align === 'right' ? 'align-right' : 'align-left'}"
      style="width: {width};"
      role="menu"
      tabindex="-1"
    >
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .popover-wrapper {
    position: relative;
    display: inline-flex;
  }

  .popover-menu {
    position: absolute;
    top: calc(100% + 6px);
    background: var(--card, #11131c);
    border: 1px solid var(--border-strong, rgba(255, 255, 255, 0.16));
    border-radius: 8px;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.55), 0 0 0 1px rgba(255, 255, 255, 0.04);
    z-index: var(--z-dropdown, 50);
    padding: 8px;
    animation: popIn 0.12s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .align-left {
    left: 0;
  }

  .align-right {
    right: 0;
  }

  @keyframes popIn {
    from { opacity: 0; transform: translateY(-3px) scale(0.98); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }
</style>
