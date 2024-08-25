export type Config = "release" | "debug";

export const configArgs = (config: Config): string[] =>
  config === "release" ? ["--release"] : [];

export const parseConfigArg = (isRelease: boolean): Config =>
  isRelease ? "release" : "debug";
