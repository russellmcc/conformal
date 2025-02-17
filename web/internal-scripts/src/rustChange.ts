import { Command } from "@commander-js/extra-typings";
import { $ } from "bun";
import * as fs from "node:fs/promises";

const withKnope = async (f: () => Promise<void>) => {
  // UGH! both changesets and knope are hyper-opinionated and NEED their change
  // files in `.changeset` folder, but their sets are 100% incompatible :(.
  const changesetDir = ".changeset";
  const tsDir = "ts-changeset";
  const knopeDir = "rust-changeset";

  // Move the existing changeset dir to the temporary tsDir
  await $`mv ${changesetDir} ${tsDir}`;

  // Move the checked-in knopeDir to the changesetDir, if it exists
  if (await fs.exists(knopeDir)) {
    await $`mv ${knopeDir} ${changesetDir}`;
  }
  try {
    await f();
  } finally {
    // Restore paths
    if (await fs.exists(changesetDir)) {
      await $`mv ${changesetDir} ${knopeDir}`;
    }
    await $`mv ${tsDir} ${changesetDir}`;
  }
};

export const rustChange = async () => {
  await withKnope(async () => {
    await $`knope document-change`;
  });
};

export const rustPrepareRelease = async () => {
  await withKnope(async () => {
    await $`knope prepare-release`;
  });
};

export const addRustChangeCommand = (command: Command) => {
  command
    .command("rust-change")
    .description("Document a rust changeset")
    .action(async () => {
      await rustChange();
    });
};

export const addRustPrepareReleaseCommand = (command: Command) => {
  command
    .command("rust-prepare-release")
    .description("Prepare a rust release")
    .action(async () => {
      await rustPrepareRelease();
    });
};
