import { ConfigMetadata, stampTemplate } from "@conformal/stamp";
import {
  Config as PlugConfig,
  metadatas as plugMetadatas,
  toEnv,
  toTemplate,
  postBuild as plugPostBuild,
} from "@conformal/create-plugin";
import { $ } from "bun";
import path from "node:path";

export type Config = {
  proj_slug: string;
} & PlugConfig;

export const metadatas: Record<keyof Config, ConfigMetadata> = {
  ...plugMetadatas,
  proj_slug: {
    prompt: "Project slug (lower snake_case, e.g. `my_project`)",
    description: "Slug for the project in lower snake_case, e.g. `my_project`",
    default: "my_project",
    positional: true,
  },
};

export const postBuild = async (config: Config, root?: string) => {
  const env = await toEnv(config);
  const template = await toTemplate(config);
  const dest =
    root === undefined ? config.proj_slug : path.join(root, config.proj_slug);

  await stampTemplate(dest, template, env, { merge: true });

  await plugPostBuild(dest, config);

  await $`git init`.cwd(dest);
};
