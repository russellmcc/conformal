import { $ } from "bun";
import { Command } from "@commander-js/extra-typings";

export const format = async (): Promise<boolean> =>
  (await $`cargo fmt && bun run --filter '*' format`.nothrow()).exitCode == 0;

export const execute = async () => {
  await format();
};

export const addFormatCommand = (command: Command) =>
  command
    .command("format")
    .description("Auto-format code")
    .action(async () => {
      await execute();
    });
