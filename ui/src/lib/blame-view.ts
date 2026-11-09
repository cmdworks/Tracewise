import type { BlameLine } from "./types";

/** The line numbers of the current file that still come from change `id` (blame only knows the file as it is now). */
export function linesFromChange(lines: BlameLine[], id: string | null | undefined): number[] {
  if (!id) return [];
  return lines.filter((l) => l.change === id).map((l) => l.line);
}

/** What the slider's pick means for the lines shown: how many are still there, said plainly. */
export function survivalNote(count: number, total: number): string {
  if (total === 0) return "";
  if (count === 0) return "None of this change's lines are in the file now; later changes replaced or removed them.";
  if (count === total) return total === 1 ? "The file's only line comes from this change." : `All ${total} lines come from this change.`;
  return `${count} of ${total} lines in the file now come from this change.`;
}
