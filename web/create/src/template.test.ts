import { describe, test, expect } from "bun:test";
import { withDir } from "tmp-promise";
import path from "node:path";
import { unlink, rm, readdir } from "node:fs/promises";
import { $ } from "bun";
import { Config, postBuild } from "./config";
import { stampTemplate } from "@conformal/stamp";
import { toEnv } from "@conformal/create-plugin";
import { z } from "zod";

const TEST_CONFIG: Config = {
  proj_slug: "test",
  plug_slug: "test_plug",
  plug_name: "Test Plug",
  vendor_name: "Test Project",
  plug_type: "effect",
};

const MINUTE = 60_000;

const runShell = async (args: string[], options: { cwd?: string } = {}) => {
  const proc = Bun.spawn(args, {
    stdio: ["inherit", "inherit", "inherit"],
    env: process.env,
    ...options,
  });
  console.log(`$ ${args.map((x) => `"${x}"`).join(" ")}`);
  await proc.exited;
  if (proc.exitCode !== 0) {
    process.exit(proc.exitCode ?? undefined);
  }
};

const packageJsonSchema = z
  .object({
    catalog: z.optional(z.record(z.string(), z.string())),
    dependencies: z.optional(z.record(z.string(), z.string())),
    devDependencies: z.optional(z.record(z.string(), z.string())),
    scripts: z.optional(
      z
        .object({
          prepack: z.optional(z.string()),
          postpack: z.optional(z.string()),
        })
        .passthrough(),
    ),
  })
  .passthrough();

describe("create-conformal template", () => {
  test(
    "passes CI",
    async () => {
      await withDir(
        async ({ path: tmpDir }) => {
          const workspacePath = path.join(
            import.meta.path,
            "..",
            "..",
            "..",
            "..",
          );

          type RewireDepsOptions = {
            fixCatalog: boolean;
          };
          const rewireDeps = async (
            dest: string,
            { fixCatalog }: RewireDepsOptions = { fixCatalog: true },
          ) => {
            const rootPackageJson = packageJsonSchema.parse(
              await Bun.file(`package.json`).json(),
            );

            const packageJson = packageJsonSchema.parse(
              await Bun.file(`${dest}/package.json`).json(),
            );

            packageJson.version = "0.0.0";

            const cleanupDict = (dict: Record<string, string>) => {
              for (const [key, version] of Object.entries(dict)) {
                if (key.startsWith("@conformal/")) {
                  dict[key] =
                    `file://${tmpDir}/conformal-${key.split("/")[1]}-0.0.0.tgz`;
                }
                if (version === "catalog:" && fixCatalog) {
                  const catalogVersion = rootPackageJson.catalog?.[key];
                  if (!catalogVersion) {
                    throw new Error(
                      `Catalog dependency ${key} not found in root package.json`,
                    );
                  }
                  dict[key] = catalogVersion;
                }
              }
            };
            for (const dict of [
              packageJson.dependencies,
              packageJson.catalog,
              packageJson.devDependencies,
            ]) {
              if (dict) {
                cleanupDict(dict);
              }
            }

            await Bun.write(
              `${dest}/package.json`,
              JSON.stringify(packageJson, null, 2),
            );
          };

          // Note that bun skips dependencies when installing packages from local paths :'(,
          // so instead, we use `bun pm pack` to create tarballs.

          const localDependencies = [
            "scripts",
            "plugin",
            "stamp",
            "create-plugin",
          ];
          for (const dep of localDependencies) {
            await $`bun pm pack --destination=${tmpDir}`.cwd(
              path.join(workspacePath, "web", dep),
            );
            const tgzGlob = new Bun.Glob(`conformal-${dep}-*.tgz`);
            let tgzPath: string | undefined;
            for await (const tgzPathCandidate of tgzGlob.scan(tmpDir)) {
              if (tgzPath !== undefined) {
                throw new Error(`Found multiple tarballs for ${dep}`);
              }
              tgzPath = path.join(tmpDir, tgzPathCandidate);
            }
            if (tgzPath === undefined) {
              throw new Error(`No tarball found for ${dep}`);
            }
            expect(Bun.file(tgzPath).exists()).resolves.toBe(true);

            // Extract the tarball to a sub-directory of tmpDir
            const extractDir = path.join(tmpDir, `package`);
            await runShell(["tar", "-xzf", tgzPath], { cwd: tmpDir });
            // Fix up all dependencies
            await rewireDeps(extractDir);

            // Remove any "prepack" and "postpack" scripts as these have already been run
            const packageJson = packageJsonSchema.parse(
              await Bun.file(`${extractDir}/package.json`).json(),
            );
            if (packageJson.scripts?.prepack) {
              delete packageJson.scripts.prepack;
            }
            if (packageJson.scripts?.postpack) {
              delete packageJson.scripts.postpack;
            }
            await Bun.write(
              `${extractDir}/package.json`,
              JSON.stringify(packageJson, null, 2),
            );
            // Remove the unfixed tarball
            await unlink(tgzPath);
            // Re-pack the tarball
            await $`bun pm pack --destination=${tmpDir}`.cwd(extractDir);

            const rewiredTgzPath = path.join(
              tmpDir,
              `conformal-${dep}-0.0.0.tgz`,
            );
            expect(Bun.file(rewiredTgzPath).exists()).resolves.toBe(true);

            await rm(extractDir, { recursive: true });
          }

          // stamp the template
          const dest = path.join(tmpDir, TEST_CONFIG.proj_slug);
          const env = await toEnv(TEST_CONFIG, { rustVersionMode: "mock" });
          await stampTemplate(
            dest,
            path.join(workspacePath, "web", "create", "template"),
            env,
          );
          await postBuild(TEST_CONFIG, env, tmpDir);

          // We want to use the catalog from the template
          await rewireDeps(dest, { fixCatalog: false });

          await $`bun install`.cwd(dest);

          // Add a synth target
          await $`bun run create-plugin --plug_type synth --plug_slug test_synth --vendor_name "Test Vendor" --plug_name "Test Synth"`.cwd(
            dest,
          );

          // Note that the generate script leaves some intentional task markers - replace all these with "DONE"
          const replaceInFiles = async (
            dirs: string[],
            pattern: RegExp,
            replacement: string,
          ) => {
            for (const dir of dirs) {
              const entries = await readdir(path.join(dest, dir), {
                recursive: true,
                withFileTypes: true,
              });
              for (const entry of entries) {
                if (!entry.isFile()) continue;
                const filePath = path.join(entry.parentPath, entry.name);
                const content = await Bun.file(filePath).text();
                const updated = content.replace(pattern, replacement);
                if (updated !== content) {
                  await Bun.write(filePath, updated);
                }
              }
            }
          };

          await replaceInFiles(["web", "rust"], /TODO/g, "DONE");

          // We have to re-install after adding new packages, now that dependencies are isolated.
          await $`bun install`.cwd(dest);

          // In CI, we will have a 0.0.0 version for the conformal crates.
          // Replace these with a link to the local crate
          const createDependencies = ["component", "vst_wrapper", "poly"];
          for (const dep of createDependencies) {
            const crateVersion = `{ path = "${path.join(workspacePath, "rust", dep.replace("_", "-")).replaceAll("\\", "\\\\")}" }`;
            await replaceInFiles(
              ["rust"],
              new RegExp(`conformal_${dep} = "[^"]+"`, "g"),
              `conformal_${dep} = ${crateVersion}`,
            );
          }

          // Make sure CI would pass.
          await $`bun run ci`.cwd(dest);
        },
        { unsafeCleanup: true, tmpdir: process.env.RUNNER_TEMP },
      );
    },
    process.platform === "win32" ? 10 * MINUTE : 5 * MINUTE,
  );
});
