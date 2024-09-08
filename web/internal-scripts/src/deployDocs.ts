import { withDir } from "tmp-promise";
import path from "node:path";
import { $ } from "bun";
import { publish } from "gh-pages";

export const deployDocs = async () => {
  await withDir(
    async ({ path: tmpDir }) => {
      const workspacePath = path.join(import.meta.path, "..", "..", "..", "..");

      // Build the documentation
      await $`bun run web-build docs`.cwd(workspacePath);

      // Build the rust documentation
      await $`cargo doc --no-deps`.cwd(workspacePath);

      // Copy the documentation into the temporary directory
      await $`cp -r web/docs/out/* ${tmpDir}/`.cwd(workspacePath);
      await $`cp -r target/doc ${tmpDir}/rust-doc`.cwd(workspacePath);

      // Deploy the temporary diretory
      await publish(tmpDir, () => {
        // do nothing
      });
    },
    { unsafeCleanup: true },
  );
};
