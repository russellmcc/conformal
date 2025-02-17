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
  // Note that because of https://github.com/rust-lang/cargo/issues/1169
  // We can't do this natively in cargo but have to use some third-party tool.
  // None of these seem ideal, all are _very_ opinionated about the specific
  // publish workflow.
  await $`cargo install --locked cargo-workspaces`;
  await $`cargo workspaces publish -y --publish-as-is --allow-dirty --no-git-commit`;

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
