import type { TrailResult } from "./types";

/**
 * The repository to send with a call: a path, or null for "the one the app is in". The page starts with no project chosen (an empty
 * string), and the desktop side rejects an empty path as "repository path does not exist", so empty means none.
 */
export function repoOrNull(repo?: string | null): string | null {
  return repo && repo.trim() ? repo : null;
}

export async function invokeTrail(args: string[], repoOverride?: string | null): Promise<TrailResult> {
  const targetRepo = repoOrNull(repoOverride);
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<TrailResult>("trail", { args, repo: targetRepo });
  } catch (e: any) {
    // Never invent data: outside the desktop app there is no Tracewise to ask.
    if (e?.toString().includes("__TAURI_INTERNALS__") || e?.toString().includes("window.__TAURI__")) {
      throw new Error("Cannot reach Tracewise: open this page in the Tracewise desktop app.");
    }
    throw e;
  }
}
