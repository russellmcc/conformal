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
  await $`cargo install --locked cargo-release`;
  await $`cargo release publish --workspace`;

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
