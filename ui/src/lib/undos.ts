import type { ChangeItem } from "./types";

export interface UndoEntry {
  /** The undo record itself (its id is what `trail redo` takes). */
  undo: ChangeItem;
  /** The change it put back, when Tracewise named it. */
  undid: string | null;
  /**
   * A redo record does not say which undo it applied, so this is a judgement: a later redo of the same file exists. It is shown as
   * "likely redone", never as a fact, and the Redo button is withheld only then.
   */
  likelyRedone: boolean;
}

/** The newest undos among the given changes (newest first), each with whether a later redo of the same file exists. */
export function recentUndos(changes: ChangeItem[], limit = 20): UndoEntry[] {
  return changes
    .filter((c) => c.op === "undo")
    .slice(0, limit)
    .map((undo) => ({
      undo,
      undid: undo.undoes?.[0] ?? null,
      likelyRedone: changes.some((r) => r.op === "redo" && r.path === undo.path && (r.time?.start ?? "") > (undo.time?.start ?? ""))
    }));
}
