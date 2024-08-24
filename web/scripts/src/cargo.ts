import { CommandLineAction } from "@rushstack/ts-command-line";
import runShell from "./runShell";

export const execute = async (args: readonly string[]) => {
  const cargoArgs: string[] = [
    "cargo",
    ...args,
    '--config=target.\'cfg(all())\'.rustflags = ["-D", "warnings"]',
  ];
  await runShell(cargoArgs);
};

export class CargoAction extends CommandLineAction {
  public constructor() {
    super({
      actionName: "cargo",
      summary: "Runs cargo",
      documentation: "",
    });

    this.defineCommandLineRemainder({
      description: "The arguments to cargo",
    });
  }

  public async onExecute(): Promise<void> {
    await execute(this.remainder!.values);
  }
}
