import { z } from "zod";
import runShell from "./runShell";
import { Command } from "@commander-js/extra-typings";

const packageJsonSchema = z.object({
  scripts: z.record(z.string(), z.string()),
});

export const execute = async ({ withMiri }: { withMiri?: true }) => {
  process.env.CI = "1";

  const rootPackageJson = packageJsonSchema.parse(
    await Bun.file("package.json").json(),
  );

  const actions = [
    "check-format",
    "check-todo",
    "check-lfs",
    "web-lint",
    "rust-lint",
    "check-licenses",
    "web-test",
    "rust-test",
    ["validate", "*", "--release"],
  ];

  if (withMiri) {
    actions.push("rust-miri");
  }

  for (const action of actions) {
    // If the action is not available in root package.json, skip it.
    // This acts as a way for client projects to opt-out of certain checks.
    const actionName = typeof action === "string" ? action : action[0];

    if (!actionName || !rootPackageJson.scripts[actionName]) {
      continue;
    }

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
    .option(
      "--with-miri",
      "Run tests with [miri](https://github.com/rust-lang/miri) undefined behavior checks",
    )
    .action(async (options) => {
      await execute(options);
    });
};
