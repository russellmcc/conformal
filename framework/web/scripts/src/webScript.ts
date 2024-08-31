// This command is not used directly, but rather is used to create the `web-*` actions that run on a specific sub-library.

import runShell from "./runShell";
import { Command } from "@commander-js/extra-typings";

export const execute = async (
  whichPackage: string,
  script: string,
  args: string[],
) => {
  if (whichPackage.includes("*")) {
    await runShell(["bun", "run", "--filter", whichPackage, script, ...args]);
  } else {
    await runShell(["bun", "run", script, ...args], {
      cwd: `web/${whichPackage}`,
    });
  }
};

export const addWebScriptCommand = (command: Command): void => {
  command
    .command("web-script")
    .description("Run a script in a specific web package")
    .requiredOption("-s, --script <script>", "The script to run")
    .arguments("<package>")
    .arguments("[args...]")
    .allowUnknownOption()
    .action(async (p, args, { script }) => {
      await execute(p, script, args);
    });
};
