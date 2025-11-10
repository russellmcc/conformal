import { $ } from "bun";
import { z } from "zod";

const packageJsonSchema = z
  .object({
    catalog: z.optional(z.record(z.string(), z.string())),
    dependencies: z.optional(z.record(z.string(), z.string())),
  })
  .passthrough();

// Cleans any `workspace` and `catalog` protocols for publishing.
// This mutates the package.json files in place.
export const cleanWorkspaceProtocols = async () => {
  const packages = (await $`bun pm list`.quiet())
    .text()
    .trim()
    .split("\n")
    .filter((p) => p.includes("@workspace:"))
    .map((p) => p.split("@workspace:")[1]!.trim());

  const rootPackageJson = packageJsonSchema.parse(
    await Bun.file("package.json").json(),
  );

  for (const p of packages) {
    // Get json contents of package.json
    const packageJson = packageJsonSchema.parse(
      await Bun.file(`${p}/package.json`).json(),
    );
    if (!packageJson.dependencies) {
      // No need to fix dependencies, since there aren't any!
      continue;
    }
    for (const [key, version] of Object.entries(packageJson.dependencies)) {
      // We demand exact versions in workspace cross-dependencies. This is enforced by changeset.
      if (version.startsWith("workspace:^")) {
        packageJson.dependencies[key] = version.replace("workspace:^", "^");
        if (!packageJson.dependencies[key]) {
          throw new Error(
            `Full workspace version for ${key} not found in ${p}/package.json, but it is required. Got ${version} instead.`,
          );
        }
        continue;
      }
      if (version.startsWith("workspace:")) {
        throw new Error(
          `Workspace dependencies must be exact versions: ${p} has ${key}@${version}, which is not allowed.`,
        );
      }
      if (version === "catalog:") {
        // Replace the catalog protocol with the root package.json's catalog.
        const catalogVersion = rootPackageJson.catalog?.[key];
        if (!catalogVersion) {
          throw new Error(
            `Catalog dependency ${key} not found in root package.json, but it is required by ${p}.`,
          );
        }
        packageJson.dependencies[key] = catalogVersion;
        continue;
      } else if (version.startsWith("catalog:")) {
        throw new Error(
          `we don't support catalog-based versions other than 'catalog:', ${p} has ${key}@${version}, which is not allowed.`,
        );
      }
    }

    // Write the package.json back to the file system
    await Bun.write(`${p}/package.json`, JSON.stringify(packageJson, null, 2));
  }
};
