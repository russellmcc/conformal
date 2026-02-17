import { Command } from "@commander-js/extra-typings";
import { join, dirname } from "node:path";
import { mkdtemp, rm } from "node:fs/promises";

import { $, Glob } from "bun";

const checkLicensesForCrate = async (crate: string): Promise<void> => {
  const aboutPath = join(crate, "about.hbs");
  const cargoTomlPath = join(crate, "Cargo.toml");
  const tmpDir = await mkdtemp(join(import.meta.dir, ".cargo-about-"));
  const outputFile = join(tmpDir, "output.html");
  console.log(`Checking licenses for ${crate}`);
  try {
    await $`cargo about generate -m ${cargoTomlPath} -o ${outputFile} ${aboutPath}`.quiet();
  } finally {
    await rm(tmpDir, { recursive: true }).catch(() => {});
  }
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
