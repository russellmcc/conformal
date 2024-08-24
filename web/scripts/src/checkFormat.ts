import { $ } from "bun";
import { failUnless } from "./util";
import { CommandLineAction } from "@rushstack/ts-command-line";

export const checkFormat = async (): Promise<boolean> =>
  (await $`cargo fmt --check && bun run --filter '*' check-format`.nothrow())
    .exitCode == 0;

export const execute = async () => {
  failUnless(await checkFormat());
};

export class CheckFormatAction extends CommandLineAction {
  public constructor() {
    super({
      actionName: "check-format",
      summary: "Check if the code is formatted correctly",
      documentation: "",
    });
  }

  public async onExecute(): Promise<void> {
    await execute();
  }
}
