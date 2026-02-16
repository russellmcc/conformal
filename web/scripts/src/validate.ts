import getBundleData from "./bundleData";
import { Config, parseConfigArg } from "./configArg";
import runShell from "./runShell";
import { findWorkspaceRoot } from "./workspaceRoot";
import { execute as makePackage } from "./package";
import { Command } from "@commander-js/extra-typings";

export const execute = async (config: Config) => {
  // Note - this must be called from a package!
  const packageRoot = process.cwd();
  const workspaceRoot = await findWorkspaceRoot(packageRoot);

  await makePackage(config, false, false);

  // Now, continue from the repo root.
  process.chdir(workspaceRoot);

  const { name } = await getBundleData(packageRoot);

  const bundlePath = `target/${config}/${name}.vst3`;
  const sdkDir = process.env.VST3_SDK_DIR;
  if (!sdkDir) {
    console.warn("VST SDK not detected, skipping validation step.");
    return;
  }

  const outputDir = process.platform === "win32" ? "bin/Debug" : "bin";
  await runShell([`${sdkDir}/build/${outputDir}/validator`, bundlePath]);
};

export const addValidateCommand = (command: Command) => {
  command
    .command("validate")
    .description("Validate a plug-in using the Steinberg validator")
    .option("--release", "Build with optimizations")
    .action(async (options) => {
      const { release } = options as { release: boolean };
      await execute(parseConfigArg(release));
    });
};
