import { Command } from "@commander-js/extra-typings";
import { join, dirname } from "node:path";

import { $, Glob } from "bun";

const checkLicensesForCrate = async (crate: string): Promise<void> => {
  const aboutPath = join(crate, "about.hbs");
  const cargoTomlPath = join(crate, "Cargo.toml");
  console.log(`Checking licenses for ${crate}`);
  await $`cargo about generate -m ${cargoTomlPath} ${aboutPath}`.quiet();
};

export const checkLicenses = async (): Promise<void> => {
  for await (const aboutFile of new Glob("rust/**/about.hbs").scan()) {
    await checkLicensesForCrate(dirname(aboutFile.trim()));
  }
};

export const addCheckLicensesCommand = (command: Command) =>
  command
    .command("check-licenses")
    .description("Check if all rust dependency licenses are valid")
    .action(async () => {
      await checkLicenses();
    });
