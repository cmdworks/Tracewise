// The command line the app drives: look for it, and install it when it is missing.
import { app } from "./state.svelte";
import { cliStatus, installCli } from "$lib/api";

/** Looks for the command line. Returns true when the page may load its data (found, or not running in the desktop app). */
export async function checkCli(): Promise<boolean> {
  const status = await cliStatus();
  app.cliInstalled = status === null || status.installed;
  app.cliAppVersion = status?.app_version ?? "";
  app.cliChecked = true;
  return app.cliInstalled;
}

/** Runs the installer, then looks again. The installer's own message is kept when it fails. */
export async function installCommandLine(onReady: () => void): Promise<void> {
  app.cliInstalling = true;
  app.cliError = null;
  try {
    const r = await installCli();
    if (r.exit_code !== 0) {
      app.cliError = (r.stderr || r.stdout || "the installer failed").trim().split("\n").slice(-8).join("\n");
    } else if (await checkCli()) {
      onReady();
    } else {
      app.cliError = "The installer finished, but the command line was not found. Open a new terminal and run: tracewise doctor --all";
    }
  } catch (e) {
    app.cliError = String(e);
  } finally {
    app.cliInstalling = false;
  }
}
