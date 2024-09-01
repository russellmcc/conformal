#!/usr/bin/env bun

import { getConfig, toEnv } from "./config";
import { stampTemplate } from "./stamp";
import path from "node:path";
import { $ } from "bun";

const config = await getConfig();
const dest = config.proj_slug;
await stampTemplate(
  dest,
  path.join(path.dirname(import.meta.path), "..", "template"),
  toEnv(config),
);

await $`git init`.cwd(dest);
