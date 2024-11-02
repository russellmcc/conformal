export type { Config } from "./config";
export { metadatas, toEnv } from "./config";
import path from "node:path";
import { Config } from "./config";
import { parse, stringify } from "smol-toml";
import { z } from "zod";

const workspaceCargoTomlSchema = z.object({
  workspace: z
    .object({
      members: z.array(z.string()),
    })
    .passthrough(),
});

export const toTemplate = (config: Config) => {
  const template =
    config.plug_type === "synth" ? "template-synth" : "template-effect";
  return path.join(path.dirname(import.meta.path), "..", template);
};

export const postBuild = async (projectPath: string, config: Config) => {
  // Add the new crates to the workspace Cargo.toml
  const workspaceCargoTomlPath = path.join(projectPath, "Cargo.toml");

  const parsed = workspaceCargoTomlSchema.parse(
    parse(await Bun.file(workspaceCargoTomlPath).text()),
  );

  parsed.workspace.members.push(`rust/${config.plug_slug}/component`);
  parsed.workspace.members.push(`rust/${config.plug_slug}/vst`);

  await Bun.write(workspaceCargoTomlPath, stringify(parsed));
};
