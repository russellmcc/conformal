import path from "node:path";
import { $ } from "bun";
import { Command } from "@commander-js/extra-typings";

const workspacePath = path.join(import.meta.path, "..", "..", "..", "..");

export const deployDocs = async () => {
  const outDir = "_site";

  // Build the documentation
  await $`bun run web-build docs`.cwd(workspacePath);

  // Build the rust documentation
  await $`cargo doc --no-deps --all-features\
  --exclude conformal_ui\
  --exclude conformal_preferences\
  --exclude conformal_core\
  --workspace`.cwd(workspacePath);

  // Build the TypeScript documentation
  await $`typedoc`.cwd(workspacePath);

  // Clear the output directory
  await $`rm -rf ${outDir}`.cwd(workspacePath);

  // Copy the documentation into the temporary directory
  await $`mkdir -p ${outDir}`.cwd(workspacePath);
  await $`cp -r web/docs/out/* ${outDir}/`.cwd(workspacePath);
  await $`cp -r target/doc ${outDir}/rust-doc`.cwd(workspacePath);
  await $`cp -r ts-doc ${outDir}/ts-doc`.cwd(workspacePath);
};

export const addDeployDocsCommand = (command: Command) =>
  command
    .command("deploy-docs")
    .description("Deploy the documentation to the website")
    .action(async () => {
      await deployDocs();
    });
