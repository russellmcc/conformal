import { findWorkspaceRoot } from "../../scripts/src/workspaceRoot";
import { parse, stringify } from "smol-toml";
import path from "node:path";
import { z } from "zod";

const workspaceCargoTomlSchema = z.object({
  workspace: z.looseObject({
    members: z.array(z.string()),
  }),
});

const workspaceRoot = await findWorkspaceRoot(process.cwd());
const workspaceCargoTomlPath = path.join(workspaceRoot, "Cargo.toml");
const members = workspaceCargoTomlSchema.parse(
  parse(await Bun.file(workspaceCargoTomlPath).text()),
).workspace.members;

const crateCargoTomlSchema = z.object({
  package: z.looseObject({
    name: z.string(),
    version: z.string(),
  }),
});

const createVersions: Record<string, string> = {};

for (const member of members) {
  const crateCargoTomlPath = path.join(workspaceRoot, member, "Cargo.toml");
  const { name, version } = crateCargoTomlSchema.parse(
    parse(await Bun.file(crateCargoTomlPath).text()),
  ).package;
  createVersions[name + "_version"] = version;
}

await Bun.write(
  path.join(__dirname, "..", "rust_versions.toml"),
  stringify(createVersions),
);
