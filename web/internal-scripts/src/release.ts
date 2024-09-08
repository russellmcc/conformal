import { Command } from "@commander-js/extra-typings";
import { setVersion } from "./setVersion";
import { $ } from "bun";
import { deployDocs } from "./deployDocs";

export const release = async (
  tag: string,
  { skipPublish }: { skipPublish?: boolean },
) => {
  skipPublish ??= false;

  if (!tag.startsWith("v")) {
    throw new Error("Version tag must start with 'v'");
  }
  const version = tag.slice(1);
  await setVersion(version);

  if (skipPublish) {
    return;
  }

  await deployDocs();

  // For testing, do not publish any packages!
  return;

  // Publish cargo packages
  // Note that because of https://github.com/rust-lang/cargo/issues/1169
  // We can't do this natively in cargo but have to use some third-party tool.
  // None of these seem ideal, all are _very_ opinionated about the specific
  // publish workflow.
  await $`cargo install --locked cargo-workspaces`;
  await $`cargo workspaces publish -y --publish-as-is --allow-dirty --no-git-commit`;

  // Publish npm packages
  await $`bunx @morlay/bunpublish`;
};

export const addReleaseCommand = (command: Command) => {
  command
    .command("release")
    .description("Release a new version to match the given tag")
    .arguments("<tag>")
    .option("--skip-publish", "Skip publishing to npm and cargo")
    .action(async (tag, opts) => {
      await release(tag, opts);
    });
};
