import { $ } from "bun";

// Sets all relevant versions to the given version!
export const setVersion = async (version: string) => {
  // Note we use perl as a sed replacement because of https://github.com/oven-sh/bun/issues/13197,
  // which makes sed unusable on macOS.
  //
  // Note that we make a couple of assumptions here about the formatting of the project files,
  // which is a bit fragile. It would be a nice improvement to use proper parsing here rather
  // than just regexes.

  const cargoFiles = (await $`git ls-files -- '*Cargo.toml'`.text())
    .trim()
    .split("\n");
  await $`perl -pi -e 's/^version = "[^"]+"/version = "${version}"/' ${cargoFiles}`;
  await $`perl -pi -e "s/^(conformal_.*version = )\"[^\"]*\"/\\1\"${version}\"/" ${cargoFiles}`;

  const packageFiles = (await $`git ls-files -- '*package.json'`.text())
    .trim()
    .split("\n");
  await $`perl -pi -e 's/"version": "[^"]+"/"version": "${version}"/' ${packageFiles}`;
  await $`perl -pi -e 's/"workspace:\*"/"^${version}"/' ${packageFiles}`;
};
