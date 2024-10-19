import path from "node:path";
import { $ } from "bun";

export const deployDocs = async () => {
  const outDir = "_site";

  const workspacePath = path.join(import.meta.path, "..", "..", "..", "..");

  // Build the documentation
  await $`bun run web-build docs`.cwd(workspacePath);

  // Build the rust documentation
  await $`cargo doc --no-deps --all-features\
  --exclude conformal_ui\
  --exclude conformal_preferences\
  --exclude conformal_macos_bundle\
  --exclude conformal_core\
  --workspace`.cwd(workspacePath);

  // Clear the output directory
  await $`rm -rf ${outDir}`.cwd(workspacePath);

  // Copy the documentation into the temporary directory
  await $`mkdir -p ${outDir}`.cwd(workspacePath);
  await $`cp -r web/docs/out/* ${outDir}/`.cwd(workspacePath);
  await $`cp -r target/doc ${outDir}/rust-doc`.cwd(workspacePath);
};
