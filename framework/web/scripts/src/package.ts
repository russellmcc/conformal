import runShell from "./runShell";
import getBundleData from "./bundleData";
import { Config, configArgs, parseConfigArg } from "./configArg";
import { createBundle } from "./bundle";
import { createInstaller } from "./installer";
import { findWorkspaceRoot } from "./workspaceRoot";
import { Command } from "@commander-js/extra-typings";

/**
 * Must be called from a package!
 */
export const execute = async (
  config: Config,
  dist: boolean,
  linkToLibrary: boolean,
) => {
  const packageRoot = process.cwd();
  const workspaceRoot = await findWorkspaceRoot(packageRoot);
  if (dist) {
    config = "release";
  }

  // Build the web-ui!
  await runShell(["bun", "run", "build"]);

  // Now, continue from the repo root.
  process.chdir(workspaceRoot);

  const bundleData = await getBundleData(packageRoot);
  const { rustPackage } = bundleData;

  if (!dist) {
    await runShell([
      "bun",
      "run",
      "rust-build",
      ...configArgs(config),
      "--package",
      rustPackage,
    ]);
  } else {
    await runShell([
      "bun",
      "run",
      "rust-build",
      "--release",
      "--target",
      "aarch64-apple-darwin",
      "--package",
      rustPackage,
    ]);
    await runShell([
      "bun",
      "run",
      "rust-build",
      "--release",
      "--target",
      "x86_64-apple-darwin",
      "--package",
      rustPackage,
    ]);
  }

  await createBundle({ packageRoot, bundleData, config, dist, linkToLibrary });

  if (dist) {
    await createInstaller({ packageRoot, bundleData });
  }
};

export const addPackageCommand = (command: Command) => {
  command
    .command("package")
    .description("Package a plug-in")
    .option(
      "-d, --dist",
      "Whether to create a distributable package, including an installer",
    )
    .option("--release", "Build with optimizations")
    .action(async (options) => {
      const { dist, release } = options as { dist: boolean; release: boolean };
      await execute(parseConfigArg(release), dist, !dist);
    });
};
