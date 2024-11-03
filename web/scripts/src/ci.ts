import runShell from "./runShell";
import { Command } from "@commander-js/extra-typings";

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
    ["validate", "*", "--release"],
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

export const addCICommand = (command: Command): void => {
  command
    .command("ci")
    .description("Run a full CI pass")
    .action(async () => {
      await execute();
    });
};
