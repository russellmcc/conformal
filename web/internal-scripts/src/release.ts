import { Command } from "@commander-js/extra-typings";
import { $ } from "bun";
import { deployDocs } from "./deployDocs";
import { cleanWorkspaceProtocols } from "./cleanWorkspace";

export const release = async ({ skipPublish }: { skipPublish?: boolean }) => {
  skipPublish ??= false;

  if (skipPublish) {
    return;
  }

  // Publish cargo packages
  try {
    // Note this could fail since having no changes is considered a failure
    await $`cargo publish --workspace --keep-going`;
  } catch (error) {
    console.error(error);
    // Continue to publish TS packages even if rust fails
  }

  await cleanWorkspaceProtocols();

  // Publish npm packages
  await $`bunx @morlay/bunpublish`;

  // Publish documentation
  await deployDocs();
};

export const addReleaseCommand = (command: Command) => {
  command
    .command("release")
    .option("--skip-publish", "Skip publishing to npm and cargo")
    .action(async (opts) => {
      await release(opts);
    });
};
