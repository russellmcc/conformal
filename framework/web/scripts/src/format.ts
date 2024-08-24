import { $ } from "bun";
import { CommandLineAction } from "@rushstack/ts-command-line";

export const format = async (): Promise<boolean> =>
  (await $`cargo fmt && bun run --filter '*' format`.nothrow()).exitCode == 0;

export const execute = async () => {
  await format();
};

export class FormatAction extends CommandLineAction {
  public constructor() {
    super({
      actionName: "format",
      summary: "Auto-format code",
      documentation: "",
    });
  }

  public async onExecute(): Promise<void> {
    await execute();
  }
}
