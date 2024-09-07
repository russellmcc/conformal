import runShell from "./runShell";
import { Command } from "@commander-js/extra-typings";

export const execute = async (args: readonly string[]) => {
  const cargoArgs: string[] = ["cargo", ...args];
  // Disallow warnings if the CI environment variable is set.
  // exception - never disallow warnings when building on nightly,
  // because the set of available warnings changes every night!
  if (process.env.CI && !args.includes("+nightly")) {
    cargoArgs.push(
      '--config=target.\'cfg(all())\'.rustflags = ["-D", "warnings"]',
    );
  }
  await runShell(cargoArgs);
};

export const addCargoCommand = (command: Command): void => {
  command
    .command("cargo")
    .description("Runs cargo")
    .arguments("[args...]")
    .allowUnknownOption()
    .action(async (args) => {
      await execute(args);
    });
};
