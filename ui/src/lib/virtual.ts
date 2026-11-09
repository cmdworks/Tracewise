/** Windowing for a long list of equal-height rows: which rows are on screen, and how much room the others take. */
export interface WindowInput {
  scrollTop: number;
  /** Height of the scrolling area. */
  height: number;
  rowH: number;
  count: number;
  /** Extra rows kept on each side so a fast scroll does not show a gap. */
  overscan: number;
}

export interface Window {
  start: number;
  end: number;
  padTop: number;
  padBottom: number;
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

export function windowFor({ scrollTop, height, rowH, count, overscan }: WindowInput): Window {
  if (count <= 0 || rowH <= 0) return { start: 0, end: 0, padTop: 0, padBottom: 0 };
  const first = Math.floor(Math.max(0, scrollTop) / rowH);
  const last = Math.ceil((Math.max(0, scrollTop) + Math.max(0, height)) / rowH);
  const start = clamp(first - overscan, 0, count);
  const end = clamp(last + overscan, start, count);
  return { start, end, padTop: start * rowH, padBottom: (count - end) * rowH };
}

/** The scroll position that brings row `index` fully into view, or `null` when it already is. */
export function scrollTopToReveal(o: { index: number; scrollTop: number; height: number; rowH: number }): number | null {
  const top = o.index * o.rowH;
  const bottom = top + o.rowH;
  if (top < o.scrollTop) return top;
  if (bottom > o.scrollTop + o.height) return Math.max(0, bottom - o.height);
  return null;
}
