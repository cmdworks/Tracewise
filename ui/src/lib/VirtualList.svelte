<script lang="ts" generics="T">
  import { untrack, type Snippet } from "svelte";
  import { scrollTopToReveal, windowFor } from "$lib/virtual";

  /**
   * A long list of equal-height rows that renders only the rows on screen (plus a few either side), so ten thousand rows cost
   * the same as thirty. The row height is measured from the first rendered row (give its root element `data-vrow`), and again when
   * the list changes size. The row that is `selectedIndex` is scrolled into view when it changes.
   */
  let {
    items,
    key,
    row,
    selectedIndex = -1,
    overscan = 8,
    ontop
  }: {
    items: T[];
    key: (item: T) => string;
    row: Snippet<[T, number]>;
    selectedIndex?: number;
    overscan?: number;
    /** Told the index of the row at the top of the view, so a caller can pin a header for it. */
    ontop?: (index: number) => void;
  } = $props();

  let el = $state<HTMLDivElement>();
  let scrollTop = $state(0);
  let height = $state(600);
  let rowH = $state(30);
  let win = $derived(windowFor({ scrollTop, height, rowH, count: items.length, overscan }));

  /** The height of a row as laid out now (and remembered), falling back to the last known one. */
  function measure(): number {
    const first = el?.querySelector<HTMLElement>("[data-vrow]");
    const h = first?.getBoundingClientRect().height ?? 0;
    if (h > 0 && Math.abs(h - rowH) > 0.01) rowH = h;
    return h > 0 ? h : rowH;
  }

  // measure again whenever what is rendered changes
  $effect(() => {
    void items.length;
    void win.start;
    queueMicrotask(measure);
  });

  $effect(() => {
    if (!el) return;
    const ro = new ResizeObserver(() => {
      height = el!.clientHeight;
      measure();
    });
    ro.observe(el);
    return () => ro.disconnect();
  });

  $effect(() => {
    ontop?.(rowH > 0 ? Math.floor(scrollTop / rowH) : 0);
  });

  // follow the selection (j/k) without fighting a person who scrolls
  $effect(() => {
    const index = selectedIndex;
    if (index < 0 || !el) return;
    untrack(() => {
      const to = scrollTopToReveal({ index, scrollTop: el!.scrollTop, height: el!.clientHeight, rowH: measure() });
      if (to !== null) {
        el!.scrollTop = to;
        scrollTop = el!.scrollTop; // do not wait for the scroll event to learn where we are
      }
    });
  });
</script>

<div class="vlist" bind:this={el} onscroll={() => (scrollTop = el!.scrollTop)} style="flex: 1; min-height: 0; overflow-y: auto;">
  <div style="height: {win.padTop}px" aria-hidden="true"></div>
  {#each items.slice(win.start, win.end) as item, i (key(item))}
    {@render row(item, win.start + i)}
  {/each}
  <div style="height: {win.padBottom}px" aria-hidden="true"></div>
</div>
