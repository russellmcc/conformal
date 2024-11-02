#!/usr/bin/env bun

import { Config, metadatas, postBuild } from "./config";
import { toEnv } from "@conformal/create-plug";
import { stampCommand } from "@conformal/stamp";
import path from "node:path";

const command = stampCommand<keyof Config>({
  metadatas,
  toEnv: (config) => toEnv(config, {}),
  toDest: (config) => config.proj_slug,
  toTemplate: () => path.join(path.dirname(import.meta.path), "..", "template"),
  postBuild,
});

await command.parseAsync();
