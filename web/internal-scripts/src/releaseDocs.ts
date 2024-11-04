import { Command } from "@commander-js/extra-typings";
import { setVersion } from "./setVersion";
import { deployDocs } from "./deployDocs";

export const releaseDocs = async (version: string) => {
  await setVersion(version);
  // Publish documentation
  await deployDocs();
};

export const addReleaseDocsCommand = (command: Command) => {
  command
    .command("release-docs")
    .description("Release a new version of docs without doing a full release")
    .arguments("<version>")
    .action(async (version) => {
      await releaseDocs(version);
    });
};
