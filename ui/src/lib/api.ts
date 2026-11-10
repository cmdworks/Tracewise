import type { CliStatus, TracewiseResult } from "./types";

/**
 * The repository to send with a call: a path, or null for "the one the app is in". The page starts with no project chosen (an empty
 * string), and the desktop side rejects an empty path as "repository path does not exist", so empty means none.
 */
export function repoOrNull(repo?: string | null): string | null {
  return repo && repo.trim() ? repo : null;
}

export async function invokeTracewise(args: string[], repoOverride?: string | null): Promise<TracewiseResult> {
  const targetRepo = repoOrNull(repoOverride);
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<TracewiseResult>("tracewise", { args, repo: targetRepo });
  } catch (e: any) {
    // Never invent data: outside the desktop app there is no Tracewise to ask.
    if (e?.toString().includes("__TAURI_INTERNALS__") || e?.toString().includes("window.__TAURI__")) {
      throw new Error("Cannot reach Tracewise: open this page in the Tracewise desktop app.");
    }
    throw e;
  }
}

/** Where the command line is. Null outside the desktop app (or with the dev bridge): nothing to install there, so the page carries on. */
export async function cliStatus(): Promise<CliStatus | null> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return (await invoke<CliStatus | null>("cli_status")) ?? null;
  } catch {
    return null;
  }
}

/** Installs the command line of this app's version; the reply holds the installer's output and exit code. */
export async function installCli(): Promise<TracewiseResult> {
  const { invoke } = await import("@tauri-apps/api/core");
  return await invoke<TracewiseResult>("install_cli");
}
