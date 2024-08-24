import { CommandLineAction } from "@rushstack/ts-command-line";
import runShell from "./runShell";

export const execute = async () => {
  process.env.CI = "1";

  const actions = [
    "check-format",
    "check-todo",
    "check-lfs",
    "web-lint",
    "rust-lint",
    "web-test",
    "rust-test",
    ["validate", "'*'", "--release"],
    "rust-miri",
  ];

  for (const action of actions) {
    if (typeof action === "string") {
      await runShell(["bun", "run", action]);
    } else {
      await runShell(["bun", "run", ...action]);
    }
  }
};

export class CIAction extends CommandLineAction {
  public constructor() {
    super({
      actionName: "ci",
      summary: "Run a full CI pass",
      documentation: "",
    });
  }

  protected async onExecute(): Promise<void> {
    await execute();
  }
}
