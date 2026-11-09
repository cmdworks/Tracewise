import type { TrailResult } from "./types";

/** The text to show for a failed call: the first line Tracewise printed on stderr, else the exit code. */
export function failureText(what: string, res: TrailResult): string {
  const line = (res.stderr || "").split("\n").find((l) => l.trim());
  return line ? line.replace(/^trail:\s*/, "").trim() : `${what} failed (exit ${res.exit_code})`;
}

/**
 * The JSON a successful call printed. A failed call, an empty answer or text that is not JSON throws an Error with a message
 * fit for the user: a failure must never look like an empty list.
 */
export function parseJson<T>(what: string, res: TrailResult): T {
  if (res.exit_code !== 0) throw new Error(failureText(what, res));
  if (!res.stdout.trim()) throw new Error(`${what}: Tracewise printed nothing`);
  try {
    return JSON.parse(res.stdout) as T;
  } catch {
    throw new Error(`${what}: Tracewise's answer could not be read`);
  }
}

export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
