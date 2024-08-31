import { Command } from "@commander-js/extra-typings";
import { setVersion } from "./setVersion";
import { $ } from "bun";

export const release = async (tag: string) => {
  if (!tag.startsWith("v")) {
    throw new Error("Version tag must start with 'v'");
  }
  const version = tag.slice(1);
  await setVersion(version);

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
    .action(async (tag) => {
      await release(tag);
    });
};
