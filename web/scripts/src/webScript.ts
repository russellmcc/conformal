// This command is not used directly, but rather is used to create the `web-*` actions that run on a specific sub-library.

import {
  CommandLineAction,
  CommandLineStringParameter,
} from "@rushstack/ts-command-line";
import runShell from "./runShell";

export const execute = async (
  whichPackage: string,
  script: string,
  args: string[],
) => {
  await runShell(["bun", "run", "--filter", whichPackage, script, ...args]);
};

export class WebScriptAction extends CommandLineAction {
  private _script: CommandLineStringParameter;

  public constructor() {
    super({
      actionName: "web-script",
      summary: "Run a script in a specific web package",
      documentation: "",
    });

    this._script = this.defineStringParameter({
      parameterLongName: "--script",
      parameterShortName: "-s",
      argumentName: "SCRIPT",
      description: "The script to run",
      required: true,
    });

    this.defineCommandLineRemainder({
      description:
        "The package to run the script on, followed by any additional arguments to the script",
    });
  }

  public async onExecute(): Promise<void> {
    const remainder = this.remainder!.values;
    if (remainder.length < 1) {
      throw new Error(
        `Missing package name! Usage: bun run web-${this._script.value!} <package> [args...]`,
      );
    }

    await execute(remainder[0], this._script.value!, remainder.slice(1));
  }
}
