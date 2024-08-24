import getBundleData from "./bundleData";
import {
  Config,
  ConfigArgRawParameter,
  defineConfigArgRaw,
  parseConfigArg,
} from "./configArg";
import runShell from "./runShell";
import { findWorkspaceRoot } from "./workspaceRoot";
import { execute as makePackage } from "./package";
import { CommandLineAction } from "@rushstack/ts-command-line";

export const execute = async (config: Config) => {
  // Note - this must be called from a package!
  const packageRoot = process.cwd();
  const workspaceRoot = await findWorkspaceRoot(packageRoot);

  await makePackage(config, false, false);

  // Now, continue from the repo root.
  process.chdir(workspaceRoot);

  const { name } = await getBundleData(packageRoot);

  const bundlePath = `target/${config}/${name}.vst3`;

  await runShell([
    `${process.env.VST3_SDK_DIR}/build/bin/validator`,
    bundlePath,
  ]);
};

export class ValidateAction extends CommandLineAction {
  private _configArgRaw: ConfigArgRawParameter;
  public constructor() {
    super({
      actionName: "validate",
      summary: "Validate a plug-in using the steinberg validator",
      documentation:
        "Note that this must be called from a specific package folder, not the workspace root!",
    });

    this._configArgRaw = defineConfigArgRaw(this);
  }

  public async onExecute(): Promise<void> {
    await execute(parseConfigArg(this._configArgRaw));
  }
}
