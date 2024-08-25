import { $ } from "bun";
import { failUnless } from "./util";
import { Command } from "@commander-js/extra-typings";

export const checkFormat = async (): Promise<boolean> =>
  (await $`cargo fmt --check && bun run --filter '*' check-format`.nothrow())
    .exitCode == 0;

export const execute = async () => {
  failUnless(await checkFormat());
};

export const addCheckFormatCommand = (command: Command): void => {
  command
    .command("check-format")
    .description("Check if the code is formatted correctly")
    .action(async () => {
      await execute();
    });
};
