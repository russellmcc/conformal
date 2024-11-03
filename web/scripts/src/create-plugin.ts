import {
  Config,
  metadatas,
  postBuild,
  toEnv,
  toTemplate,
} from "@conformal/create-plugin";
import { buildStampCommand } from "@conformal/stamp";
import { findWorkspaceRoot } from "./workspaceRoot";
import { Command } from "@commander-js/extra-typings";

export const addCreatePlugCommand = (command: Command) => {
  buildStampCommand<keyof Config>({
    command: command.command("create-plugin"),
    metadatas,
    toEnv: toEnv,
    toDest: () => findWorkspaceRoot(process.cwd()),
    toTemplate,
    postBuild: async (config) =>
      postBuild(await findWorkspaceRoot(process.cwd()), config),
    options: {
      merge: true,
    },
  }).description("Create a new plug-in from a template");
};
