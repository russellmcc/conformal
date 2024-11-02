import { describe, test, expect } from "bun:test";
import { withDir } from "tmp-promise";
import path from "node:path";
import { $ } from "bun";
import { Config, postBuild } from "./config";
import { stampTemplate } from "@conformal/stamp";
import { toEnv } from "@conformal/create-plug";

const TEST_CONFIG: Config = {
  proj_slug: "test",
  plug_slug: "test_plug",
  plug_name: "Test Plug",
  vendor_name: "Test Project",
  plug_type: "synth",
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

          // Note that bun skips dependencies when installing packages from local paths :'(,
          // so instead, we use `npm pack` to create tarballs.

          const localDependencies = ["scripts", "plugin"];
          for (const dep of localDependencies) {
            await $`npm pack --pack-destination=${tmpDir}`.cwd(
              path.join(workspacePath, "web", dep),
            );
            expect(
              Bun.file(
                path.join(tmpDir, `conformal-${dep}-0.0.0.tgz`),
              ).exists(),
            ).resolves.toBe(true);
          }

          // stamp the template
          const dest = path.join(tmpDir, TEST_CONFIG.proj_slug);
          const envArgs = {
            skipTodo: true,
            component_crate_version: `{ path = "${path.join(workspacePath, "rust", "component")}" }`,
            vst_crate_version: `{ path = "${path.join(workspacePath, "rust", "vst-wrapper")}" }`,
          };
          await stampTemplate(
            dest,
            path.join(workspacePath, "web", "create", "template"),
            toEnv(TEST_CONFIG, envArgs),
          );
          await postBuild(TEST_CONFIG, envArgs, tmpDir);

          // Note we use perl as a sed replacement because of https://github.com/oven-sh/bun/issues/13197,
          // which makes sed unusable on macOS.
          const perl_command = `s/"\\@conformal\\/([^"]+)": "workspace:\\*"/"\\@conformal\\/$1": "file:..\\/conformal-$1-0.0.0.tgz"/`;
          await $`perl -pi -e ${perl_command} package.json`.cwd(dest);

          // Make sure CI would pass.
          await $`bun install`.cwd(dest);
          await $`bun run ci`.cwd(dest);
        },
        { unsafeCleanup: true },
      );
    },
    3 * MINUTE,
  );
});
