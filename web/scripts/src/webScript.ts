// This command is not used directly, but rather is used to create the `web-*` actions that run on a specific sub-library.

import runShell from "./runShell";
import { Command } from "@commander-js/extra-typings";

export const execute = async (
  whichPackage: string | undefined,
  script: string,
  args: string[],
) => {
  whichPackage ??= "*";
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
    .description(
      "Run a script defined in a specific web-package. If no package is provided, it will run on all packages that define the script. The package, if provided, must be the first argument after the script name.",
    )
    .requiredOption("-s, --script <script>", "The script to run")
    .argument("[args...]")
    .passThroughOptions()
    .action(async (rawArgs: string[] | undefined, { script }) => {
      const args = rawArgs ?? [];
      const [first, ...rest] = args;
      const pkg =
        first !== undefined && !first.startsWith("-") ? first : undefined;
      const scriptArgs = pkg !== undefined ? rest : args;
      await execute(pkg, script, scriptArgs);
    });
};
