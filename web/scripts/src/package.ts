import {
  CommandLineAction,
  CommandLineFlagParameter,
} from "@rushstack/ts-command-line";
import runShell from "./runShell";
import getBundleData from "./bundleData";
import {
  Config,
  ConfigArgRawParameter,
  configArgs,
  defineConfigArgRaw,
  parseConfigArg,
} from "./configArg";
import { createBundle } from "./bundle";
import { createInstaller } from "./installer";
import { findWorkspaceRoot } from "./workspaceRoot";

/**
 * Must be called from a package!
 */
export const execute = async (config: Config, dist: boolean) => {
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

  await createBundle({ packageRoot, bundleData, config, dist });

  if (dist) {
    await createInstaller({ packageRoot, bundleData });
  }
};

export class PackageAction extends CommandLineAction {
  private _configArgRaw: ConfigArgRawParameter;
  private _dist: CommandLineFlagParameter;
  public constructor() {
    super({
      actionName: "package",
      summary: "Package a plug-in",
      documentation:
        "Note that this must be called from a specific package folder, not the workspace root!",
    });

    this._configArgRaw = defineConfigArgRaw(this);
    this._dist = this.defineFlagParameter({
      parameterLongName: "--dist",
      description:
        "Whether to create a distributable package, including an installer",
    });
  }

  public async onExecute(): Promise<void> {
    await execute(parseConfigArg(this._configArgRaw), this._dist.value);
  }
}
