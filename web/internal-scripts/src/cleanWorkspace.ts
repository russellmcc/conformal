import { $ } from "bun";

// Removes any `workspace` protocols for publishing.
export const cleanWorkspaceProtocols = async () => {
  // Note we use perl as a sed replacement because of https://github.com/oven-sh/bun/issues/13197,
  // which makes sed unusable on macOS.
  //
  // Note that we make a couple of assumptions here about the formatting of the project files,
  // which is a bit fragile. It would be a nice improvement to use proper parsing here rather
  // than just regexes.

  const packageFiles = (await $`git ls-files -- '*package.json'`.text())
    .trim()
    .split("\n");
  console.log(packageFiles);
  await $`perl -pi -e 's/"workspace:([^"]+)"/"$1"/' ${packageFiles}`;
};
