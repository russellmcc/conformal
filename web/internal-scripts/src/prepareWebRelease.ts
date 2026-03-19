import { $ } from "bun";
import { Command } from "@commander-js/extra-typings";
import path from "node:path";
import { z } from "zod";
import { syncTemplateVersions } from "./syncTemplateVersions";

const workspacePath = path.join(import.meta.path, "..", "..", "..", "..");
const createPackageJsonPath = path.join(workspacePath, "web", "create", "package.json");
const createTemplatePackageJsonPath = path.join(
  workspacePath,
  "web",
  "create",
  "template",
  "package.json",
);

const packageJsonSchema = z.object({
  version: z.string(),
});

const readCreateVersion = async (): Promise<string> =>
  packageJsonSchema.parse(await Bun.file(createPackageJsonPath).json()).version;

const writeCreateConformalChangeset = async () => {
  const changesetPath = path.join(
    workspacePath,
    ".changeset",
    `ensure-create-conformal-bump-${Date.now()}.md`,
  );
  await Bun.write(
    changesetPath,
    `---
"create-conformal": patch
---

Bump create-conformal when synced template dependency versions change.
`,
  );
};

export const prepareWebRelease = async () => {
  const originalCreateVersion = await readCreateVersion();
  const originalTemplatePackageJson =
    await Bun.file(createTemplatePackageJsonPath).text();

  await $`changeset version`.cwd(workspacePath);
  await syncTemplateVersions();

  const updatedCreateVersion = await readCreateVersion();
  const updatedTemplatePackageJson =
    await Bun.file(createTemplatePackageJsonPath).text();

  if (
    originalCreateVersion === updatedCreateVersion &&
    originalTemplatePackageJson !== updatedTemplatePackageJson
  ) {
    await writeCreateConformalChangeset();
    await $`changeset version`.cwd(workspacePath);
    await syncTemplateVersions();
  }

  await $`bun install`.cwd(workspacePath);
};

export const addPrepareWebReleaseCommand = (command: Command) => {
  command
    .command("prepare-web-release")
    .description("prepare web release and sync template bumps")
    .action(async () => {
      await prepareWebRelease();
    });
};
