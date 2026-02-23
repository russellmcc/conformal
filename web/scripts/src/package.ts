import runShell from "./runShell";
import getBundleData from "./bundleData";
import { Config, configArgs, parseConfigArg } from "./configArg";
import { createBundle } from "./bundle";
import { createInstaller } from "./installer";
import { createWindowsVstBundle } from "./windowsVstBundle";
import { createWindowsInstaller } from "./windowsInstaller";
import { findWorkspaceRoot } from "./workspaceRoot";
import { Command } from "@commander-js/extra-typings";

/**
 * Must be called from a package!
 */
const executeMacos = async (
  config: Config,
  dist: boolean,
  linkToLibrary: boolean,
  adHocSign: boolean,
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
    await createInstaller({
      packageRoot,
      bundleData,
      adHocSign,
    });
  }
};

const executeWindows = async (
  config: Config,
  dist: boolean,
  linkToLibrary: boolean,
) => {
  const packageRoot = process.cwd();
  const workspaceRoot = await findWorkspaceRoot(packageRoot);
  if (dist) {
    config = "release";
  }

  await runShell(["bun", "run", "build"]);

  process.chdir(workspaceRoot);

  const bundleData = await getBundleData(packageRoot);
  const { rustPackage } = bundleData;

  await runShell([
    "bun",
    "run",
    "rust-build",
    ...configArgs(config),
    "--package",
    rustPackage,
  ]);

  await createWindowsVstBundle({
    packageRoot,
    bundleData,
    config,
    linkToLibrary,
  });

  if (dist) {
    await createWindowsInstaller({ packageRoot, bundleData });
  }
};

export const execute = async (
  config: Config,
  dist: boolean,
  linkToLibrary: boolean,
  adHocSign = false,
) => {
  if (process.platform === "win32") {
    await executeWindows(config, dist, linkToLibrary);
  } else {
    await executeMacos(config, dist, linkToLibrary, adHocSign);
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
    .option(
      "--ad-hoc-sign",
      "When creating a distributable package, use ad-hoc codesigning (no certs or notarization; for local use only). This only applied to macOS, conformal installers are always unsigned.",
    )
    .option("--release", "Build with optimizations")
    .action(async (options) => {
      const { dist, release, adHocSign } = options;
      if (adHocSign && !dist) {
        throw new Error(
          "Ad-hoc signing is only relevant when creating a distributable package",
        );
      }
      await execute(
        parseConfigArg(release),
        dist ?? false,
        !dist,
        adHocSign ?? false,
      );
    });
};
