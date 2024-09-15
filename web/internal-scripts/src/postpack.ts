import { Command } from "@commander-js/extra-typings";
import * as fs from "node:fs/promises";

export const postpack = async () => {
  // Restore from backup
  await fs.rename("./package.json.bak", "./package.json");
};

export const addPostpackCommand = (command: Command) => {
  command
    .command("ts-browser-postpack")
    .description("standard postpack script for ts browser libs")
    .action(async () => {
      await postpack();
    });
};
