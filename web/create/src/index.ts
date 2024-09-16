#!/usr/bin/env bun

import { getConfig, toEnv } from "./config";
import { stampTemplate } from "./stamp";
import path from "node:path";
import { $ } from "bun";

const config = await getConfig();
const dest = config.proj_slug;

// Update the rust versions by grabbing our own package.json,
// This is valid because we version all crates and packages together.
// eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
const version: string =
  // eslint-disable-next-line @typescript-eslint/no-unsafe-member-access
  (await Bun.file(path.join(__dirname, "..", "package.json")).json()).version;

await stampTemplate(
  dest,
  path.join(path.dirname(import.meta.path), "..", "template"),
  toEnv(config, {
    component_crate_version: `"${version}"`,
    vst_crate_version: `"${version}"`,
  }),
);

await $`git init`.cwd(dest);
