import { $ } from "bun";
import { failUnless } from "./util";
import * as path from "node:path";
import { Command } from "@commander-js/extra-typings";

// Note this is totally cromulent in bun
declare global {
  interface Set<T> {
    difference(other: Set<T>): Set<T>;
  }
}

const lfsFiles = async function* () {
  const root = (await $`git rev-parse --show-toplevel`).stdout
    .toString()
    .trim();
  const relPath = path.relative(root, process.cwd());
  for await (const line of $`git lfs ls-files`.lines()) {
    if (line === "") continue;
    const fullRepoPath = line.split(" ").slice(2).join(" ");
    if (!fullRepoPath.startsWith(relPath)) continue;
    yield fullRepoPath.slice(relPath.length + 1);
  }
};

const shouldLfsFiles = async function* () {
  for await (const line of $`git ls-files`.lines()) {
    if (line === "") continue;
    const filter_raw = await $`git check-attr filter ${line}`.text();
    const filter = filter_raw.split("filter: ").slice(-1)[0].trim();
    if (filter === "lfs") {
      yield line;
    }
  }
};

export const checkLfs = async (): Promise<boolean> => {
  // Gather all files that are in fact lfs tracked.
  const lfsFilesSet = new Set(await Array.fromAsync(lfsFiles()));

  // Gather all files that _should_ be lfs tracked.
  const lfsFilesShouldSet = new Set(await Array.fromAsync(shouldLfsFiles()));

  const unexpectedLfsFiles = lfsFilesSet.difference(lfsFilesShouldSet);
  const missingLfsFiles = lfsFilesShouldSet.difference(lfsFilesSet);

  if (unexpectedLfsFiles.size > 0) {
    console.error(
      `Unexpected lfs files: ${Array.from(unexpectedLfsFiles.values()).join(", ")}`,
    );
  }

  if (missingLfsFiles.size > 0) {
    console.error(
      `Missing lfs files: ${new Array(missingLfsFiles.values()).join(", ")}`,
    );
  }
  return !(unexpectedLfsFiles.size > 0 || missingLfsFiles.size > 0);
};

export const execute = async () => {
  failUnless(await checkLfs());
};

export const addCheckLfsCommand = (command: Command) =>
  command
    .command("check-lfs")
    .description(
      "Checks that no files are checked in that should be lfs tracked",
    )
    .action(async () => {
      await execute();
    });
