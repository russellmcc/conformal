import { Command } from "@commander-js/extra-typings";
import path from "node:path";
import { z } from "zod";

const workspacePath = path.join(import.meta.path, "..", "..", "..", "..");

const workspacePackageJsonSchema = z.looseObject({
  name: z.optional(z.string()),
  version: z.optional(z.string()),
});

const rawPackageJsonSchema = z.record(z.string(), z.unknown());
const dependencySectionSchema = z.record(z.string(), z.string());

const versionSections = [
  "dependencies",
  "devDependencies",
  "peerDependencies",
  "optionalDependencies",
  "catalog",
] as const;

const syncableRange =
  /^(?<prefix>\^|~)?(?<version>\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?)$/;

const getWorkspaceVersions = async (): Promise<Map<string, string>> => {
  const versions = new Map<string, string>();
  const glob = new Bun.Glob("web/*/package.json");

  for await (const packageJsonPath of glob.scan(workspacePath)) {
    const packageJson = workspacePackageJsonSchema.parse(
      await Bun.file(path.join(workspacePath, packageJsonPath)).json(),
    );
    if (packageJson.name && packageJson.version) {
      versions.set(packageJson.name, packageJson.version);
    }
  }

  return versions;
};

const syncVersion = (current: string, next: string): string | undefined => {
  const match = syncableRange.exec(current);
  const groups = match?.groups;
  if (!groups) {
    return;
  }
  const { prefix = "", version } = groups;
  if (version === next) {
    return current;
  }
  return `${prefix}${next}`;
};

export const syncTemplateVersions = async () => {
  const workspaceVersions = await getWorkspaceVersions();
  const glob = new Bun.Glob("web/**/template/**/package.json");

  for await (const packageJsonPath of glob.scan(workspacePath)) {
    const absolutePath = path.join(workspacePath, packageJsonPath);
    const packageJson = rawPackageJsonSchema.parse(
      await Bun.file(absolutePath).json(),
    );

    let changed = false;
    for (const sectionName of versionSections) {
      const sectionValue = packageJson[sectionName];
      if (sectionValue === undefined) {
        continue;
      }
      const section = dependencySectionSchema.parse(sectionValue);
      let sectionChanged = false;

      for (const [dependencyName, currentVersion] of Object.entries(section)) {
        if (!dependencyName.startsWith("@conformal/")) {
          continue;
        }

        const nextVersion = workspaceVersions.get(dependencyName);
        if (!nextVersion) {
          continue;
        }

        const syncedVersion = syncVersion(currentVersion, nextVersion);
        if (!syncedVersion || syncedVersion === currentVersion) {
          continue;
        }

        section[dependencyName] = syncedVersion;
        sectionChanged = true;
        changed = true;
      }

      if (sectionChanged) {
        packageJson[sectionName] = section;
      }
    }

    if (changed) {
      await Bun.write(
        absolutePath,
        `${JSON.stringify(packageJson, null, 2)}\n`,
      );
    }
  }
};

export const addSyncTemplateVersionsCommand = (command: Command) => {
  command
    .command("sync-template-versions")
    .description("sync template package dependency versions")
    .action(async () => {
      await syncTemplateVersions();
    });
};
