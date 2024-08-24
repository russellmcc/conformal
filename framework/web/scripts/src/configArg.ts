import {
  CommandLineAction,
  CommandLineFlagParameter,
} from "@rushstack/ts-command-line";

export type Config = "release" | "debug";

export const configArgs = (config: Config): string[] =>
  config === "release" ? ["--release"] : [];

export type ConfigArgRawParameter = CommandLineFlagParameter;

export const defineConfigArgRaw = (
  action: CommandLineAction,
): ConfigArgRawParameter =>
  action.defineFlagParameter({
    parameterLongName: "--release",
    description:
      "Package an optimized version of the plug-in suitable for release.",
  });

export const parseConfigArg = (raw: ConfigArgRawParameter): Config =>
  raw.value ? "release" : "debug";
