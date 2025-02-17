import { Command } from "@commander-js/extra-typings";
import { deployDocs } from "./deployDocs";

export const releaseDocs = async () => {
  // Publish documentation
  await deployDocs();
};

export const addReleaseDocsCommand = (command: Command) => {
  command
    .command("release-docs")
    .description("Release a new version of docs without doing a full release")
    .action(async () => {
      await releaseDocs();
    });
};
