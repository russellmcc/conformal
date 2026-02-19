import { BundleData } from "./bundleData";
import getBundleData from "./bundleData";
import runShell from "./runShell";
import { Command } from "@commander-js/extra-typings";

/** Set the dev_mode preference for a plugin on the current platform. */
export const setDevMode = async (
  bundleData: BundleData,
  enabled: boolean,
): Promise<void> => {
  if (process.platform === "win32") {
    await runShell([
      "reg",
      "add",
      `HKCU\\Software\\${bundleData.vendor}.${bundleData.id}`,
      "/v",
      "dev_mode",
      "/t",
      "REG_SZ",
      "/d",
      enabled ? "true" : "false",
      "/f",
    ]);
  } else {
    await runShell([
      "defaults",
      "write",
      bundleData.id,
      "dev_mode",
      enabled ? "true" : "false",
    ]);
  }
};

export const addDevModeCommand = (command: Command): void => {
  command
    .command("dev-mode")
    .description("Turn the dev mode preference on or off for a plug-in")
    .option("--on", "Enable dev mode")
    .option("--off", "Disable dev mode")
    .action(async ({ on, off }) => {
      if (on && off) {
        throw new Error("Cannot specify both --on and --off");
      }
      if (!on && !off) {
        throw new Error("Must specify either --on or --off");
      }
      const bundleData = await getBundleData(process.cwd());
      await setDevMode(bundleData, !!on);
    });
};
