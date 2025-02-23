import * as path from "node:path";
import { access } from "node:fs/promises";

const directoryExists = async (dir: string) => {
  try {
    await access(dir);
    return true;
  } catch {
    return false;
  }
};

export const findWorkspaceRoot = async (dir: string) => {
  // Since we don't have an api to actually get the workspace root, we
  // rather crawl up trying to find a file named `bun.lockb`.
  let last = null;
  while (dir !== last) {
    last = dir;
    if (
      (await Bun.file(path.join(dir, "bun.lockb")).exists()) ||
      (await Bun.file(path.join(dir, "bun.lock")).exists())
    ) {
      return dir;
    }
    dir = path.dirname(dir);
  }

  // If that didn't work, assume _this file_ is installed in the workspace.
  last = null;
  dir = import.meta.dir;
  while (dir !== last) {
    last = dir;
    if (await directoryExists(path.join(dir, "node_modules"))) {
      return dir;
    }
    dir = path.dirname(dir);
  }

  throw new Error("Could not find workspace root!");
};
