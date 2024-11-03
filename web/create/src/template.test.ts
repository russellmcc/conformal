import { describe, test, expect } from "bun:test";
import { withDir } from "tmp-promise";
import path from "node:path";
import { unlink, rm } from "node:fs/promises";
import { $ } from "bun";
import { Config, postBuild } from "./config";
import { stampTemplate } from "@conformal/stamp";
import { toEnv } from "@conformal/create-plugin";

const TEST_CONFIG: Config = {
  proj_slug: "test",
  plug_slug: "test_plug",
  plug_name: "Test Plug",
  vendor_name: "Test Project",
  plug_type: "effect",
};

const MINUTE = 60_000;

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

          const rewireDeps = async (dest: string) => {
            // Note we use perl as a sed replacement because of https://github.com/oven-sh/bun/issues/13197,
            // which makes sed unusable on macOS.
            const perl_command = `s!"\\@conformal/([^"]+)": "workspace:\\*"!"\\@conformal/$1": "file://${tmpDir}/conformal-$1-0.0.0.tgz"!`;
            await $`perl -pi -e ${perl_command} package.json`.cwd(dest);
          };

          // Note that bun skips dependencies when installing packages from local paths :'(,
          // so instead, we use `npm pack` to create tarballs.

          const localDependencies = [
            "scripts",
            "plugin",
            "stamp",
            "create-plugin",
          ];
          for (const dep of localDependencies) {
            await $`npm pack --pack-destination=${tmpDir}`.cwd(
              path.join(workspacePath, "web", dep),
            );
            const tgzPath = path.join(tmpDir, `conformal-${dep}-0.0.0.tgz`);
            expect(Bun.file(tgzPath).exists()).resolves.toBe(true);

            // Extract the tarball to a sub-directory of tmpDir
            const extractDir = path.join(tmpDir, `package`);
            await $`tar -xzf ${tgzPath}`.cwd(tmpDir);
            // Fix up all dependencies
            await rewireDeps(extractDir);

            // Remove any "prepack" and "postpack" scripts as these have already been run
            // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
            const packageJson = await Bun.file(
              `${extractDir}/package.json`,
            ).json();
            // eslint-disable-next-line @typescript-eslint/no-unsafe-member-access
            if (packageJson.scripts?.prepack) {
              // eslint-disable-next-line @typescript-eslint/no-unsafe-member-access
              delete packageJson.scripts.prepack;
            }
            // eslint-disable-next-line @typescript-eslint/no-unsafe-member-access
            if (packageJson.scripts?.postpack) {
              // eslint-disable-next-line @typescript-eslint/no-unsafe-member-access
              delete packageJson.scripts.postpack;
            }
            await Bun.write(
              `${extractDir}/package.json`,
              JSON.stringify(packageJson, null, 2),
            );
            // Remove the unfixed tarball
            await unlink(tgzPath);
            // Re-pack the tarball
            await $`npm pack --pack-destination=${tmpDir}`.cwd(extractDir);
            expect(Bun.file(tgzPath).exists()).resolves.toBe(true);

            await rm(extractDir, { recursive: true });
          }

          // stamp the template
          const dest = path.join(tmpDir, TEST_CONFIG.proj_slug);
          await stampTemplate(
            dest,
            path.join(workspacePath, "web", "create", "template"),
            await toEnv(TEST_CONFIG),
          );
          await postBuild(TEST_CONFIG, tmpDir);

          await rewireDeps(dest);

          await $`bun install`.cwd(dest);

          // Add a synth target
          await $`bun x conformal-scripts create-plugin --plug_type synth --plug_slug test_synth --vendor_name "Test Vendor" --plug_name "Test Synth"`.cwd(
            dest,
          );

          // Note that the generate script leaves some intentional task markers - replace all these with "DONE"
          await $`find web rust -type f -exec perl -pi -e 's/TOD[O]/DONE/g' {} +`.cwd(
            dest,
          );

          // In CI, we will have a 0.0.0 version for the conformal crates.
          // Replace these with a link to the local crate
          const createDependencies = ["component", "vst_wrapper", "poly"];
          for (const dep of createDependencies) {
            const crateVersion = `{ path = "${path.join(workspacePath, "rust", dep.replace("_", "-"))}" }`;
            await $`find rust -type f -exec perl -pi -e 's!conformal_${dep} = "0.0.0"!conformal_${dep} = ${crateVersion}!' {} +`.cwd(
              dest,
            );
          }

          // Make sure CI would pass.
          await $`bun run ci`.cwd(dest);
        },
        { unsafeCleanup: true },
      );
    },
    3 * MINUTE,
  );
});
